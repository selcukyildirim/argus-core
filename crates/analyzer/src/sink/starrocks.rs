//! StarRocks Stream Load sink via HTTP PUT.
//!
//! Uses StarRocks' native Stream Load API to ingest NDJSON rows at high
//! throughput. Requires the `starrocks` feature flag.
//!
//! ```ignore
//! let sink = StarRocksSink::new("http://fe:8030", "argus", "root", "");
//! sink.load_summary(&summary).await?;
//! sink.load_conflicts(&conflicts).await?;
//! ```
//!
//! # StarRocks DDL
//!
//! ```sql
//! CREATE DATABASE IF NOT EXISTS argus;
//!
//! CREATE TABLE argus.block_summary (
//!     block_number  BIGINT        NOT NULL,
//!     total_txs     INT           NOT NULL,
//!     txs_with_storage INT        NOT NULL,
//!     total_entries INT           NOT NULL,
//!     total_conflicts INT         NOT NULL,
//!     hotspot_count INT           NOT NULL,
//!     fetch_time_ms BIGINT        NOT NULL,
//!     total_time_ms BIGINT        NOT NULL,
//!     created_at    VARCHAR(32)   NOT NULL
//! ) ENGINE = OLAP
//! PRIMARY KEY (block_number)
//! DISTRIBUTED BY HASH(block_number) BUCKETS 4
//! PROPERTIES ("replication_num" = "1");
//!
//! CREATE TABLE argus.conflicts (
//!     block_number       BIGINT       NOT NULL,
//!     tx_a               VARCHAR(66)  NOT NULL,
//!     tx_b               VARCHAR(66)  NOT NULL,
//!     contract_address   VARCHAR(42)  NOT NULL,
//!     contract_protocol  VARCHAR(64)  NOT NULL,
//!     contract_name      VARCHAR(128) NOT NULL,
//!     slot               VARCHAR(66)  NOT NULL,
//!     conflict_kind      VARCHAR(4)   NOT NULL,
//!     created_at         VARCHAR(32)  NOT NULL
//! ) ENGINE = OLAP
//! DUPLICATE KEY (block_number, tx_a)
//! DISTRIBUTED BY HASH(block_number) BUCKETS 4
//! PROPERTIES ("replication_num" = "1");
//!
//! CREATE TABLE argus.contention_events (
//!     block_number       BIGINT       NOT NULL,
//!     contract_address   VARCHAR(42)  NOT NULL,
//!     contract_protocol  VARCHAR(64)  NOT NULL,
//!     contract_name      VARCHAR(128) NOT NULL,
//!     slot_id            VARCHAR(66)  NOT NULL,
//!     hazard_type        VARCHAR(4)   NOT NULL COMMENT 'WAW, RAW, WAR',
//!     affected_tx_count  INT          NOT NULL,
//!     conflict_count     INT          NOT NULL,
//!     conflict_density   FLOAT        NOT NULL COMMENT 'conflicts / txs — enemy score',
//!     severity           VARCHAR(10)  NOT NULL COMMENT 'LOW / MEDIUM / HIGH / CRITICAL',
//!     created_at         VARCHAR(32)  NOT NULL
//! ) ENGINE = OLAP
//! DUPLICATE KEY (block_number, contract_address)
//! DISTRIBUTED BY HASH(contract_address) BUCKETS 4
//! PROPERTIES ("replication_num" = "1");
//! ```

use super::{BlockSummaryRow, ConflictRow};

/// StarRocks Stream Load sink.
pub struct StarRocksSink {
    fe_url: String,
    database: String,
    username: String,
    password: String,
    client: reqwest::Client,
}

impl StarRocksSink {
    /// Create a new StarRocks sink.
    ///
    /// - `fe_url`: StarRocks FE HTTP address, e.g. `http://localhost:8030`
    /// - `database`: target database, e.g. `argus`
    /// - `username`/`password`: auth credentials (default: `root`/`""`)
    pub fn new(
        fe_url: impl Into<String>,
        database: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            fe_url: fe_url.into(),
            database: database.into(),
            username: username.into(),
            password: password.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Stream Load a block summary row.
    pub async fn load_summary(
        &self,
        row: &BlockSummaryRow,
    ) -> Result<StreamLoadResult, StreamLoadError> {
        let body = serde_json::to_string(row)?;
        self.stream_load("block_summary", &body).await
    }

    /// Stream Load conflict rows (batched in one HTTP request).
    pub async fn load_conflicts(
        &self,
        rows: &[ConflictRow],
    ) -> Result<StreamLoadResult, StreamLoadError> {
        if rows.is_empty() {
            return Ok(StreamLoadResult {
                status: "Success".into(),
                rows_loaded: 0,
                message: "no rows".into(),
            });
        }

        // NDJSON body.
        let mut body = String::with_capacity(rows.len() * 256);
        for row in rows {
            serde_json::to_writer(unsafe { body.as_mut_vec() }, row)?;
            body.push('\n');
        }

        self.stream_load("conflicts", &body).await
    }

    /// Execute a Stream Load request.
    async fn stream_load(
        &self,
        table: &str,
        body: &str,
    ) -> Result<StreamLoadResult, StreamLoadError> {
        let url = format!(
            "{}/api/{}/{}/_stream_load",
            self.fe_url, self.database, table
        );

        let label = format!(
            "argus_{}_{}_{}",
            table,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            rand_u32()
        );

        tracing::info!(table, label, bytes = body.len(), "stream load");

        let resp = self
            .client
            .put(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("label", &label)
            .header("format", "json")
            .header("strip_outer_array", "false")
            .header("Expect", "100-continue")
            .body(body.to_owned())
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() && !status.is_redirection() {
            return Err(StreamLoadError::Http(format!("HTTP {} — {}", status, text)));
        }

        // Parse StarRocks JSON response.
        let sr: serde_json::Value = serde_json::from_str(&text)?;
        let sr_status = sr["Status"].as_str().unwrap_or("Unknown").to_string();
        let loaded = sr["NumberLoadedRows"].as_u64().unwrap_or(0);
        let msg = sr["Message"].as_str().unwrap_or("").to_string();

        if sr_status != "Success" && sr_status != "Publish Timeout" {
            tracing::warn!(table, sr_status, msg, "stream load non-success");
        }

        Ok(StreamLoadResult {
            status: sr_status,
            rows_loaded: loaded,
            message: msg,
        })
    }
}

/// Result of a Stream Load operation.
#[derive(Debug)]
pub struct StreamLoadResult {
    pub status: String,
    pub rows_loaded: u64,
    pub message: String,
}

/// Errors from Stream Load.
#[derive(Debug)]
pub enum StreamLoadError {
    Json(serde_json::Error),
    Http(String),
    Reqwest(reqwest::Error),
}

impl From<serde_json::Error> for StreamLoadError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<reqwest::Error> for StreamLoadError {
    fn from(e: reqwest::Error) -> Self {
        Self::Reqwest(e)
    }
}

impl std::fmt::Display for StreamLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON: {e}"),
            Self::Http(s) => write!(f, "HTTP: {s}"),
            Self::Reqwest(e) => write!(f, "reqwest: {e}"),
        }
    }
}

impl std::error::Error for StreamLoadError {}

/// Quick pseudo-random u32 for unique labels (no rand dep).
fn rand_u32() -> u32 {
    use std::time::SystemTime;
    let t = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    (t.subsec_nanos() ^ (t.as_secs() as u32)) & 0xFFFF_FFFF
}
