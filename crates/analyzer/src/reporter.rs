//! Enriched conflict report generator.
//!
//! Takes a `ConflictGraph` and produces a human-readable report with
//! protocol labels, conflict grouping, and summary statistics.

use alloy_primitives::Address;
use argus_core::{AccessList, ConflictGraph, ConflictKind};
use std::collections::HashMap;

/// Enriched report produced from a ConflictGraph.
#[derive(Debug)]
pub struct Report {
    pub block_number: u64,
    pub total_txs: usize,
    pub txs_with_storage: usize,
    pub total_entries: usize,
    pub total_conflicts: usize,
    pub groups: Vec<ConflictGroup>,
    pub fetch_time: std::time::Duration,
    pub total_time: std::time::Duration,
}

/// A group of conflicts on the same contract.
#[derive(Debug)]
pub struct ConflictGroup {
    pub address: Address,
    pub protocol: String,
    pub label: String,
    pub slot_count: usize,
    pub tx_count: usize,
    pub conflict_count: usize,
    pub kind_summary: String,
}

impl Report {
    /// Build a report from conflict graph + access lists.
    pub fn build(
        block_number: u64,
        access_lists: &[AccessList],
        graph: &ConflictGraph,
        fetch_time: std::time::Duration,
        total_time: std::time::Duration,
    ) -> Self {
        let total_txs = access_lists.len();
        let txs_with_storage = access_lists
            .iter()
            .filter(|al| !al.entries.is_empty())
            .count();
        let total_entries: usize = access_lists.iter().map(|al| al.entries.len()).sum();

        // Group conflicts by contract address.
        let mut by_address: HashMap<Address, ContractConflicts> = HashMap::new();

        for c in &graph.conflicts {
            let entry = by_address.entry(c.location.address).or_default();
            entry.slots.insert(c.location.slot);
            entry.tx_hashes.insert(c.tx_a);
            entry.tx_hashes.insert(c.tx_b);
            entry.conflict_count += 1;
            match c.kind {
                ConflictKind::WriteWrite => entry.ww_count += 1,
                ConflictKind::ReadWrite => entry.rw_count += 1,
            }
        }

        let mut groups: Vec<ConflictGroup> = by_address
            .into_iter()
            .map(|(addr, cc)| {
                let (protocol, label) = match argus_provider::labels::lookup(&addr) {
                    Some(l) => (l.protocol.to_string(), l.name.to_string()),
                    None => ("Unknown".to_string(), format!("{}", addr)),
                };

                let kind_summary = if cc.rw_count > 0 && cc.ww_count > 0 {
                    format!("{} W-W, {} R-W", cc.ww_count, cc.rw_count)
                } else if cc.ww_count > 0 {
                    format!("{} W-W", cc.ww_count)
                } else {
                    format!("{} R-W", cc.rw_count)
                };

                ConflictGroup {
                    address: addr,
                    protocol,
                    label,
                    slot_count: cc.slots.len(),
                    tx_count: cc.tx_hashes.len(),
                    conflict_count: cc.conflict_count,
                    kind_summary,
                }
            })
            .collect();

        // Sort by conflict count descending.
        groups.sort_by(|a, b| b.conflict_count.cmp(&a.conflict_count));

        Report {
            block_number,
            total_txs,
            txs_with_storage,
            total_entries,
            total_conflicts: graph.len(),
            groups,
            fetch_time,
            total_time,
        }
    }

    /// Render the report as a formatted string with contention density.
    pub fn render(&self, graph: &ConflictGraph) -> String {
        let mut out = String::new();

        // Compute contention events for density display.
        let contention = self.to_contention_events(graph);

        out.push_str("\n");
        out.push_str("╔══════════════════════════════════════════════════════════════╗\n");
        out.push_str("║                    ARGUS ANALYSIS REPORT                    ║\n");
        out.push_str("╠══════════════════════════════════════════════════════════════╣\n");
        out.push_str(&format!(
            "║  Block:              {:>38} ║\n",
            self.block_number
        ));
        out.push_str(&format!(
            "║  Transactions:       {:>38} ║\n",
            self.total_txs
        ));
        out.push_str(&format!(
            "║  With storage ops:   {:>38} ║\n",
            self.txs_with_storage
        ));
        out.push_str(&format!(
            "║  Storage entries:    {:>38} ║\n",
            self.total_entries
        ));
        out.push_str(&format!(
            "║  Conflicts:          {:>38} ║\n",
            self.total_conflicts
        ));
        out.push_str(&format!(
            "║  Fetch time:         {:>35?} ║\n",
            self.fetch_time
        ));
        out.push_str(&format!(
            "║  Total time:         {:>35?} ║\n",
            self.total_time
        ));
        out.push_str("╠══════════════════════════════════════════════════════════════╣\n");

        if contention.is_empty() {
            out.push_str("║  No conflicts — all txs can run in parallel.               ║\n");
        } else {
            out.push_str("║  CONTENTION HOTSPOTS                                       ║\n");
            out.push_str("╠══════════════════════════════════════════════════════════════╣\n");

            for (i, ev) in contention.iter().enumerate() {
                out.push_str("║                                                              ║\n");
                out.push_str(&format!(
                    "║  {}. [{}] {} / {}\n",
                    i + 1,
                    ev.severity,
                    ev.contract_protocol,
                    ev.contract_name
                ));
                out.push_str(&format!(
                    "║     {} | Slot: {}…\n",
                    ev.contract_address,
                    &ev.slot_id[..10]
                ));
                out.push_str(&format!(
                    "║     Hazard: {}  |  Txs: {}  |  Conflicts: {}  |  Density: {:.2}\n",
                    ev.hazard_type, ev.affected_tx_count, ev.conflict_count, ev.conflict_density
                ));
            }
        }

        out.push_str("╚══════════════════════════════════════════════════════════════╝\n");
        out
    }
}

#[derive(Default)]
struct ContractConflicts {
    slots: std::collections::HashSet<alloy_primitives::B256>,
    tx_hashes: std::collections::HashSet<alloy_primitives::B256>,
    conflict_count: usize,
    ww_count: usize,
    rw_count: usize,
}
