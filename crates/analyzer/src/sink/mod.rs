//! High-performance data sink for enriched conflict reports.
//!
//! Three row schemas:
//! - [`BlockSummaryRow`] — one per analyzed block
//! - [`ConflictRow`] — one per conflict edge (denormalized)
//! - [`ContentionEvent`] — one per contract×slot×hazard (aggregated, with density)
//!
//! Two backends:
//! - **NDJSON stream** — write newline-delimited JSON rows to any `Write` impl
//! - **StarRocks Stream Load** — HTTP PUT directly to StarRocks FE (feature-gated)

pub mod json_stream;
#[cfg(feature = "starrocks")]
pub mod starrocks;

use serde::Serialize;

// ---------------------------------------------------------------------------
// Serializable row types
// ---------------------------------------------------------------------------

/// One row per conflict edge — append-only, fully denormalized.
#[derive(Debug, Clone, Serialize)]
pub struct ConflictRow {
    pub block_number: u64,
    pub tx_a: String,
    pub tx_b: String,
    pub contract_address: String,
    pub contract_protocol: String,
    pub contract_name: String,
    pub slot: String,
    pub conflict_kind: String,
    pub created_at: String,
}

/// One row per analyzed block — summary statistics.
#[derive(Debug, Clone, Serialize)]
pub struct BlockSummaryRow {
    pub block_number: u64,
    pub total_txs: u32,
    pub txs_with_storage: u32,
    pub total_entries: u32,
    pub total_conflicts: u32,
    pub hotspot_count: u32,
    pub fetch_time_ms: u64,
    pub total_time_ms: u64,
    pub created_at: String,
}

/// Aggregated contention event: one row per (contract, slot, hazard_type) per block.
///
/// `conflict_density` = conflicts / affected_tx_count.
/// A density > 1.0 means combinatorial explosion — the contract is a bottleneck.
/// Example: 12 txs, 66 conflicts → density 5.5 — this contract serializes the block.
#[derive(Debug, Clone, Serialize)]
pub struct ContentionEvent {
    pub block_number: u64,
    pub contract_address: String,
    pub contract_protocol: String,
    pub contract_name: String,
    pub slot_id: String,
    /// WAW (Write-After-Write), RAW (Read-After-Write), WAR (Write-After-Read)
    pub hazard_type: String,
    /// Number of unique transactions touching this (contract, slot).
    pub affected_tx_count: u32,
    /// Number of pairwise conflict edges.
    pub conflict_count: u32,
    /// conflict_count / affected_tx_count — the "enemy score".
    pub conflict_density: f64,
    /// Severity: LOW (<1.0), MEDIUM (1.0–3.0), HIGH (3.0–5.0), CRITICAL (>5.0)
    pub severity: String,
    pub created_at: String,
}

impl ContentionEvent {
    fn severity_label(density: f64) -> &'static str {
        match density {
            d if d >= 5.0 => "CRITICAL",
            d if d >= 3.0 => "HIGH",
            d if d >= 1.0 => "MEDIUM",
            _ => "LOW",
        }
    }
}

// ---------------------------------------------------------------------------
// Builder: Report → Rows
// ---------------------------------------------------------------------------

use crate::reporter::Report;
use std::collections::{HashMap, HashSet};

impl Report {
    /// Flatten the report into sink-ready rows.
    pub fn to_rows(&self) -> (BlockSummaryRow, Vec<ConflictRow>) {
        let now = chrono_now();

        let summary = BlockSummaryRow {
            block_number: self.block_number,
            total_txs: self.total_txs as u32,
            txs_with_storage: self.txs_with_storage as u32,
            total_entries: self.total_entries as u32,
            total_conflicts: self.total_conflicts as u32,
            hotspot_count: self.groups.len() as u32,
            fetch_time_ms: self.fetch_time.as_millis() as u64,
            total_time_ms: self.total_time.as_millis() as u64,
            created_at: now.clone(),
        };

        let conflicts = Vec::new();

        (summary, conflicts)
    }

    /// Flatten the report + raw graph into per-edge conflict rows.
    pub fn to_rows_from_graph(
        &self,
        graph: &argus_core::ConflictGraph,
    ) -> (BlockSummaryRow, Vec<ConflictRow>) {
        let now = chrono_now();

        let summary = BlockSummaryRow {
            block_number: self.block_number,
            total_txs: self.total_txs as u32,
            txs_with_storage: self.txs_with_storage as u32,
            total_entries: self.total_entries as u32,
            total_conflicts: self.total_conflicts as u32,
            hotspot_count: self.groups.len() as u32,
            fetch_time_ms: self.fetch_time.as_millis() as u64,
            total_time_ms: self.total_time.as_millis() as u64,
            created_at: now.clone(),
        };

        let conflicts: Vec<ConflictRow> = graph
            .conflicts
            .iter()
            .map(|c| {
                let (protocol, name) = match argus_provider::labels::lookup(&c.location.address) {
                    Some(l) => (l.protocol.to_string(), l.name.to_string()),
                    None => ("Unknown".into(), format!("{}", c.location.address)),
                };

                ConflictRow {
                    block_number: self.block_number,
                    tx_a: format!("{}", c.tx_a),
                    tx_b: format!("{}", c.tx_b),
                    contract_address: format!("{}", c.location.address),
                    contract_protocol: protocol,
                    contract_name: name,
                    slot: format!("{}", c.location.slot),
                    conflict_kind: match c.kind {
                        argus_core::ConflictKind::WriteWrite => "W-W".into(),
                        argus_core::ConflictKind::ReadWrite => "R-W".into(),
                    },
                    created_at: now.clone(),
                }
            })
            .collect();

        (summary, conflicts)
    }

    /// Build aggregated contention events — one per (contract, slot, hazard_type).
    ///
    /// Key metric: `conflict_density` = conflicts / affected_txs.
    /// Sorted by density descending — worst offenders first.
    pub fn to_contention_events(&self, graph: &argus_core::ConflictGraph) -> Vec<ContentionEvent> {
        let now = chrono_now();

        // Group: (address, slot, kind) → { tx_hashes, conflict_count }
        #[derive(Default)]
        struct Bucket {
            tx_hashes: HashSet<alloy_primitives::B256>,
            count: u32,
        }

        type Key = (alloy_primitives::Address, alloy_primitives::B256, String);
        let mut buckets: HashMap<Key, Bucket> = HashMap::new();

        for c in &graph.conflicts {
            let hazard = match c.kind {
                argus_core::ConflictKind::WriteWrite => "WAW",
                argus_core::ConflictKind::ReadWrite => "RAW",
            };

            let key = (c.location.address, c.location.slot, hazard.to_string());
            let bucket = buckets.entry(key).or_default();
            bucket.tx_hashes.insert(c.tx_a);
            bucket.tx_hashes.insert(c.tx_b);
            bucket.count += 1;
        }

        let mut events: Vec<ContentionEvent> = buckets
            .into_iter()
            .map(|((addr, slot, hazard), bucket)| {
                let affected = bucket.tx_hashes.len() as u32;
                let density = bucket.count as f64 / affected as f64;

                let (protocol, name) = match argus_provider::labels::lookup(&addr) {
                    Some(l) => (l.protocol.to_string(), l.name.to_string()),
                    None => ("Unknown".into(), format!("{}", addr)),
                };

                ContentionEvent {
                    block_number: self.block_number,
                    contract_address: format!("{}", addr),
                    contract_protocol: protocol,
                    contract_name: name,
                    slot_id: format!("{}", slot),
                    hazard_type: hazard,
                    affected_tx_count: affected,
                    conflict_count: bucket.count,
                    conflict_density: (density * 100.0).round() / 100.0, // 2 decimal
                    severity: ContentionEvent::severity_label(density).into(),
                    created_at: now.clone(),
                }
            })
            .collect();

        // Sort by density descending — worst offenders first.
        events.sort_by(|a, b| b.conflict_density.partial_cmp(&a.conflict_density).unwrap());

        events
    }
}

/// ISO-8601 timestamp without chrono dependency.
fn chrono_now() -> String {
    use std::time::SystemTime;
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let secs = d.as_secs();
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        1970 + secs / 31_536_000,
        (secs % 31_536_000) / 2_592_000 + 1,
        (secs % 2_592_000) / 86_400 + 1,
        (secs % 86_400) / 3600,
        (secs % 3600) / 60,
        secs % 60,
    )
}
