//! NDJSON (newline-delimited JSON) stream sink.
//!
//! Zero-copy, zero-alloc hot path: each row is serialized directly to the
//! writer without intermediate `String` allocation.
//!
//! ```ignore
//! let mut sink = JsonStreamSink::stdout();
//! sink.write_summary(&summary)?;
//! sink.write_conflicts(&conflicts)?;
//! ```

use super::{BlockSummaryRow, ConflictRow};
use std::io::{self, BufWriter, Write};

/// High-performance NDJSON writer.
///
/// Wraps any `Write` in a `BufWriter` for batch I/O. Each row is
/// serialized directly via `serde_json::to_writer` (no intermediate String).
pub struct JsonStreamSink<W: Write> {
    writer: BufWriter<W>,
    rows_written: usize,
}

impl JsonStreamSink<io::Stdout> {
    /// Write NDJSON to stdout.
    pub fn stdout() -> Self {
        Self {
            writer: BufWriter::with_capacity(64 * 1024, io::stdout()),
            rows_written: 0,
        }
    }
}

impl<W: Write> JsonStreamSink<W> {
    /// Create a sink wrapping any writer (file, Vec<u8>, etc.).
    pub fn new(writer: W) -> Self {
        Self {
            writer: BufWriter::with_capacity(64 * 1024, writer),
            rows_written: 0,
        }
    }

    /// Write one block summary row.
    pub fn write_summary(&mut self, row: &BlockSummaryRow) -> io::Result<()> {
        serde_json::to_writer(&mut self.writer, row)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        self.writer.write_all(b"\n")?;
        self.rows_written += 1;
        Ok(())
    }

    /// Write all conflict rows.
    pub fn write_conflicts(&mut self, rows: &[ConflictRow]) -> io::Result<()> {
        for row in rows {
            serde_json::to_writer(&mut self.writer, row)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            self.writer.write_all(b"\n")?;
            self.rows_written += 1;
        }
        Ok(())
    }

    /// Write aggregated contention events.
    pub fn write_contention_events(&mut self, rows: &[super::ContentionEvent]) -> io::Result<()> {
        for row in rows {
            serde_json::to_writer(&mut self.writer, row)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            self.writer.write_all(b"\n")?;
            self.rows_written += 1;
        }
        Ok(())
    }

    /// Flush and return how many rows were written.
    pub fn finish(mut self) -> io::Result<usize> {
        self.writer.flush()?;
        Ok(self.rows_written)
    }

    /// Number of rows written so far.
    pub fn rows_written(&self) -> usize {
        self.rows_written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_roundtrip() {
        let mut buf = Vec::new();
        let mut sink = JsonStreamSink::new(&mut buf);

        let summary = BlockSummaryRow {
            block_number: 21_000_000,
            total_txs: 181,
            txs_with_storage: 133,
            total_entries: 304,
            total_conflicts: 70,
            hotspot_count: 3,
            fetch_time_ms: 340,
            total_time_ms: 42000,
            created_at: "2026-02-28T00:00:00Z".into(),
        };

        let conflicts = vec![ConflictRow {
            block_number: 21_000_000,
            tx_a: "0xabc".into(),
            tx_b: "0xdef".into(),
            contract_address: "0x502E".into(),
            contract_protocol: "ERC-20".into(),
            contract_name: "Meme Token".into(),
            slot: "0x02".into(),
            conflict_kind: "W-W".into(),
            created_at: "2026-02-28T00:00:00Z".into(),
        }];

        sink.write_summary(&summary).unwrap();
        sink.write_conflicts(&conflicts).unwrap();
        let n = sink.finish().unwrap();

        assert_eq!(n, 2);

        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);

        // Verify JSON is valid.
        let _: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let _: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    }
}
