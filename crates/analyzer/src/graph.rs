//! Conflict graph construction from per-transaction access lists.

use alloy_primitives::B256;
use argus_core::{AccessList, AccessMode, Conflict, ConflictGraph, ConflictKind, StorageLocation};
use std::collections::HashMap;

/// Builds a [`ConflictGraph`] from a slice of access lists.
///
/// Uses a two-phase approach:
///   1. Reverse-index every `(location -> [(tx, mode)])` using borrowed keys.
///   2. For each location with 2+ accessors, emit conflict edges where at
///      least one side is a write.
///
/// Location clones only happen for actual conflicts (cold path).
pub fn build_conflict_graph(access_lists: &[AccessList]) -> ConflictGraph {
    let mut graph = ConflictGraph::new();

    // Reverse index: &StorageLocation -> [(tx_hash, mode)].
    let mut location_index: HashMap<&StorageLocation, Vec<(B256, AccessMode)>> = HashMap::new();

    for al in access_lists {
        for entry in &al.entries {
            location_index
                .entry(&entry.location)
                .or_default()
                .push((al.tx_hash, entry.mode));
        }
    }

    // Pair-wise conflict detection at each shared location.
    for (location, accessors) in &location_index {
        if accessors.len() < 2 {
            continue;
        }

        for i in 0..accessors.len() {
            for j in (i + 1)..accessors.len() {
                let (tx_a, mode_a) = &accessors[i];
                let (tx_b, mode_b) = &accessors[j];

                let kind = match (mode_a, mode_b) {
                    (AccessMode::Write, AccessMode::Write) => ConflictKind::WriteWrite,
                    (AccessMode::Write, AccessMode::Read)
                    | (AccessMode::Read, AccessMode::Write) => ConflictKind::ReadWrite,
                    (AccessMode::Read, AccessMode::Read) => continue,
                };

                graph.add_conflict(Conflict {
                    tx_a: *tx_a,
                    tx_b: *tx_b,
                    location: (*location).clone(),
                    kind,
                });
            }
        }
    }

    graph
}
