//! EVM transaction simulator using `revm`.
//!
//! Replays transactions against an EVM database and captures every
//! `SLOAD`/`SSTORE` to produce an [`AccessList`] per transaction.

use alloy_primitives::{Address, B256};
use argus_core::error::{ArgusError, ArgusResult};
use argus_core::types::{AccessEntry, AccessMode, StorageLocation};
use argus_core::{AccessList, Transaction};
use revm::context::Context;
use revm::database::EmptyDB;
use revm::inspector::Inspector;
use revm::interpreter::interpreter::EthInterpreter;
use revm::interpreter::{interpreter_types::*, Interpreter};
use smallvec::SmallVec;

/// Re-export from provider for backward compatibility.
pub use argus_provider::WarmCacheDB;

const OPCODE_SLOAD: u8 = 0x54;
const OPCODE_SSTORE: u8 = 0x55;

// ---------------------------------------------------------------------------
// Inspector
// ---------------------------------------------------------------------------

/// Records `SLOAD`/`SSTORE` accesses during EVM execution.
///
/// Tracks the current contract address via `call()`/`call_end()` hooks
/// so storage accesses are attributed to the correct account.
pub struct AccessListInspector {
    pub entries: SmallVec<[AccessEntry; 32]>,
    address_stack: SmallVec<[Address; 8]>,
}

impl AccessListInspector {
    pub fn new(initial_address: Option<Address>) -> Self {
        let mut address_stack = SmallVec::new();
        if let Some(addr) = initial_address {
            address_stack.push(addr);
        }
        Self {
            entries: SmallVec::new(),
            address_stack,
        }
    }

    pub fn into_entries(self) -> SmallVec<[AccessEntry; 32]> {
        self.entries
    }

    #[inline]
    fn current_address(&self) -> Option<&Address> {
        self.address_stack.last()
    }
}

impl<CTX> Inspector<CTX, EthInterpreter> for AccessListInspector {
    #[inline]
    fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, _context: &mut CTX) {
        let opcode = interp.bytecode.opcode();
        if opcode != OPCODE_SLOAD && opcode != OPCODE_SSTORE {
            return;
        }

        let mode = if opcode == OPCODE_SLOAD {
            AccessMode::Read
        } else {
            AccessMode::Write
        };

        let stack_data = interp.stack.data();
        if stack_data.is_empty() {
            return;
        }

        let slot = B256::from(stack_data[stack_data.len() - 1].to_be_bytes());

        let address = match self.current_address() {
            Some(addr) => *addr,
            None => return,
        };

        self.entries.push(AccessEntry {
            location: StorageLocation { address, slot },
            mode,
        });
    }

    fn call(
        &mut self,
        _context: &mut CTX,
        inputs: &mut revm::interpreter::CallInputs,
    ) -> Option<revm::interpreter::CallOutcome> {
        self.address_stack.push(inputs.target_address);
        None
    }

    fn call_end(
        &mut self,
        _context: &mut CTX,
        _inputs: &revm::interpreter::CallInputs,
        _outcome: &mut revm::interpreter::CallOutcome,
    ) {
        self.address_stack.pop();
    }

    fn create(
        &mut self,
        _context: &mut CTX,
        _inputs: &mut revm::interpreter::CreateInputs,
    ) -> Option<revm::interpreter::CreateOutcome> {
        None
    }
}

// ---------------------------------------------------------------------------
// Batch simulation (legacy EmptyDB path)
// ---------------------------------------------------------------------------

/// Simulates a batch against `EmptyDB`. Offloaded to `spawn_blocking`.
pub async fn simulate_batch(transactions: Vec<Transaction>) -> ArgusResult<Vec<AccessList>> {
    tokio::task::spawn_blocking(move || simulate_batch_sync(&transactions))
        .await
        .map_err(|e| ArgusError::Internal(format!("spawn_blocking panicked: {e}")))?
}

fn simulate_batch_sync(transactions: &[Transaction]) -> ArgusResult<Vec<AccessList>> {
    let mut access_lists = Vec::with_capacity(transactions.len());
    for tx in transactions {
        access_lists.push(simulate_one_tx(
            tx,
            revm::database::CacheDB::new(EmptyDB::new()),
        )?);
    }
    Ok(access_lists)
}

// ---------------------------------------------------------------------------
// Per-tx simulation (generic over DB backend)
// ---------------------------------------------------------------------------

/// Simulates a single transaction and returns its deduplicated access list.
///
/// Entries are sorted `(location asc, mode desc)` and deduped by location,
/// keeping the worst-case mode (Write over Read).
fn simulate_one_tx<DB>(tx: &Transaction, db: DB) -> ArgusResult<AccessList>
where
    DB: revm::database_interface::DatabaseRef,
    DB::Error: core::fmt::Debug,
{
    use revm::context::TxEnv;
    use revm::handler::{MainBuilder, MainContext};
    use revm::inspector::InspectEvm;

    let tx_env = TxEnv::builder()
        .caller(tx.from)
        .kind(match tx.to {
            Some(addr) => revm::primitives::TxKind::Call(addr),
            None => revm::primitives::TxKind::Create,
        })
        .data(tx.input.clone())
        .value(tx.value)
        .gas_limit(tx.gas)
        .build()
        .map_err(|e| ArgusError::Simulation(format!("Failed to build TxEnv: {e:?}")))?;

    let inspector = AccessListInspector::new(tx.to);

    // Disable all validation so txs execute through to SLOAD/SSTORE
    // even without exact balances, nonces, or gas pricing.
    let mut ctx = Context::mainnet()
        .with_db(revm::database::CacheDB::new(db))
        .with_tx(TxEnv::default()); // placeholder, overwritten by inspect_one_tx

    ctx.cfg.disable_nonce_check = true;
    ctx.cfg.disable_balance_check = true;
    ctx.cfg.disable_block_gas_limit = true;
    ctx.cfg.disable_base_fee = true;
    ctx.cfg.disable_eip3607 = true;

    let mut evm = ctx.build_mainnet_with_inspector(inspector);

    // Pass actual tx_env — inspect_one_tx calls set_tx() internally.
    let result = evm.inspect_one_tx(tx_env);

    match &result {
        Ok(res) => {
            tracing::debug!(
                tx_hash = %tx.hash,
                gas_used = res.gas_used(),
                "evm execution ok"
            );
        }
        Err(e) => {
            tracing::debug!(tx_hash = %tx.hash, error = ?e, "evm execution error");
        }
    }

    let mut entries = std::mem::take(&mut evm.inspector.entries);

    entries.sort_unstable_by(|a, b| {
        a.location
            .cmp(&b.location)
            .then(a.mode.cmp(&b.mode).reverse())
    });
    entries.dedup_by(|a, b| a.location == b.location);

    tracing::debug!(tx_hash = %tx.hash, entries = entries.len(), "simulated");

    Ok(AccessList {
        tx_hash: tx.hash,
        entries,
    })
}

// ---------------------------------------------------------------------------
// Parallel simulation with pre-fetched state
// ---------------------------------------------------------------------------

/// Simulates all transactions in parallel against pre-fetched state.
///
/// Uses a reference overlay: `CacheDB::new(&warm_db)` creates a per-tx
/// write layer that falls through to the shared base on reads.
pub fn simulate_batch_with_state(
    warm_db: &WarmCacheDB,
    transactions: &[Transaction],
) -> ArgusResult<Vec<AccessList>> {
    use rayon::prelude::*;

    tracing::info!(txs = transactions.len(), "parallel simulation");

    let results: Vec<ArgusResult<AccessList>> = transactions
        .par_iter()
        .map(|tx| simulate_one_tx(tx, warm_db))
        .collect();

    let mut access_lists = Vec::with_capacity(results.len());
    for r in results {
        access_lists.push(r?);
    }

    tracing::info!(lists = access_lists.len(), "simulation complete");
    Ok(access_lists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Bytes, U256};

    #[test]
    fn inspector_records_nothing_without_execution() {
        let inspector = AccessListInspector::new(Some(Address::ZERO));
        assert!(inspector.entries.is_empty());
        assert!(inspector.into_entries().is_empty());
    }

    #[test]
    fn inspector_tracks_initial_address() {
        let addr = Address::ZERO;
        let inspector = AccessListInspector::new(Some(addr));
        assert_eq!(inspector.current_address(), Some(&addr));
    }

    #[test]
    fn inspector_none_address() {
        let inspector = AccessListInspector::new(None);
        assert_eq!(inspector.current_address(), None);
    }

    #[tokio::test]
    async fn empty_batch_returns_empty() {
        assert!(simulate_batch(vec![]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn single_tx_does_not_panic() {
        let tx = Transaction {
            hash: B256::ZERO,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            input: Bytes::new(),
            value: U256::ZERO,
            gas: 21000,
        };
        let result = simulate_batch(vec![tx]).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tx_hash, B256::ZERO);
    }
}
