//! Domain types for the Argus conflict analyzer.

use alloy_primitives::{Address, Bytes, B256, U256};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

/// A unique `(contract, slot)` pair in EVM state.
///
/// `#[repr(C)]` for stable layout: `Address(20) + B256(32)` = 52 bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(C)]
pub struct StorageLocation {
    pub address: Address,
    pub slot: B256,
}

/// Read (`SLOAD`) or Write (`SSTORE`).
///
/// Ordered `Read(0) < Write(1)` so reverse-sort puts writes first
/// during dedup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessMode {
    Read = 0,
    Write = 1,
}

impl PartialOrd for AccessMode {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AccessMode {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

/// A single storage access: location + read/write mode.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(C)]
pub struct AccessEntry {
    pub location: StorageLocation,
    pub mode: AccessMode,
}

/// All storage accesses recorded for one transaction.
///
/// `SmallVec<[AccessEntry; 32]>` avoids heap allocation for most txs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessList {
    pub tx_hash: B256,
    pub entries: SmallVec<[AccessEntry; 32]>,
}

// ---------------------------------------------------------------------------
// Transaction
// ---------------------------------------------------------------------------

/// Lightweight EVM transaction -- only the fields the analyzer needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: B256,
    pub from: Address,
    pub to: Option<Address>,
    /// `Bytes` (ref-counted) for zero-copy sharing through the pipeline.
    pub input: Bytes,
    pub value: U256,
    pub gas: u64,
}

// ---------------------------------------------------------------------------
// Conflict graph
// ---------------------------------------------------------------------------

/// W-W conflicts force serialization. R-W may be resolvable via speculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictKind {
    WriteWrite,
    ReadWrite,
}

/// An edge connecting two transactions through a shared storage slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub tx_a: B256,
    pub tx_b: B256,
    pub location: StorageLocation,
    pub kind: ConflictKind,
}

/// All detected conflicts for a batch of transactions.
///
/// `conflicts` is the flat edge list; `adjacency` enables O(1) neighbor lookup.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConflictGraph {
    pub conflicts: Vec<Conflict>,
    pub adjacency: HashMap<B256, Vec<B256>>,
}

impl ConflictGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_conflict(&mut self, conflict: Conflict) {
        self.adjacency
            .entry(conflict.tx_a)
            .or_default()
            .push(conflict.tx_b);
        self.adjacency
            .entry(conflict.tx_b)
            .or_default()
            .push(conflict.tx_a);
        self.conflicts.push(conflict);
    }

    pub fn has_conflict(&self, tx_a: &B256, tx_b: &B256) -> bool {
        self.adjacency
            .get(tx_a)
            .map_or(false, |neighbors| neighbors.contains(tx_b))
    }

    pub fn len(&self) -> usize {
        self.conflicts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.conflicts.is_empty()
    }
}

// Compile-time layout assertions.
const _: () = assert!(std::mem::size_of::<StorageLocation>() == 52);
const _: () = assert!(std::mem::align_of::<StorageLocation>() == 1);
