//! Concurrent state prefetcher with rate-limited concurrency and retry.
//!
//! Fetches account state + known DeFi storage slots in parallel from an
//! RPC node, producing a warm `CacheDB<EmptyDB>` for revm simulation.

use alloy_eips::BlockId;
use alloy_primitives::Address;
use alloy_provider::{DynProvider, Provider};
use argus_core::error::ArgusResult;
use argus_core::Transaction;
use revm::database::{CacheDB, EmptyDB};
use revm::state::{AccountInfo, Bytecode};
use std::sync::Arc;

/// Default max concurrent RPC tasks (each makes 3 HTTP calls).
/// Set low for free-tier RPC compatibility; increase with paid RPCs.
const DEFAULT_CONCURRENCY: usize = 1;

/// Max retry attempts for 429 errors.
const MAX_RETRIES: u32 = 3;

/// Warm cache ready for simulation. Clone-able, network-free.
pub type WarmCacheDB = CacheDB<EmptyDB>;

/// Concurrent state prefetcher.
///
/// Owns a `DynProvider` and fetches account state + known storage slots
/// in parallel via `JoinSet`, throttled by a semaphore.
///
/// ```ignore
/// let prefetcher = Prefetcher::new(provider.into_provider());
/// let warm_db = prefetcher.prefetch(block_number, &transactions).await?;
/// let results = simulate_batch_with_state(&warm_db, &transactions)?;
/// ```
pub struct Prefetcher {
    provider: DynProvider,
    max_concurrent: usize,
}

impl Prefetcher {
    pub fn new(provider: DynProvider) -> Self {
        Self {
            provider,
            max_concurrent: DEFAULT_CONCURRENCY,
        }
    }

    /// Override max concurrent RPC tasks (default: 10).
    pub fn with_concurrency(mut self, n: usize) -> Self {
        self.max_concurrent = n;
        self
    }

    /// Concurrently fetches account state and known storage slots.
    pub async fn prefetch(
        &self,
        block_number: u64,
        transactions: &[Transaction],
    ) -> ArgusResult<WarmCacheDB> {
        let mut addresses = std::collections::HashSet::new();
        for tx in transactions {
            addresses.insert(tx.from);
            if let Some(to) = tx.to {
                addresses.insert(to);
            }
        }

        let block_id = BlockId::from(block_number);
        let addr_count = addresses.len();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_concurrent));

        tracing::info!(
            block_number,
            addrs = addr_count,
            concurrency = self.max_concurrent,
            "prefetching state"
        );

        let mut tasks = tokio::task::JoinSet::new();

        // Account info: one task per address.
        for &addr in &addresses {
            let p = self.provider.clone();
            let sem = semaphore.clone();
            tasks.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                fetch_account_with_retry(&p, addr, block_id).await
            });
        }

        // Storage slots for known DeFi contracts.
        let mut slot_count = 0usize;
        for &addr in &addresses {
            if let Some(slots) = crate::slots::known_slots(&addr) {
                for &slot in slots {
                    let p = self.provider.clone();
                    let sem = semaphore.clone();
                    slot_count += 1;
                    tasks.spawn(async move {
                        let _permit = sem.acquire().await.unwrap();
                        fetch_storage_with_retry(&p, addr, slot, block_id).await
                    });
                }
            }
        }

        if slot_count > 0 {
            tracing::info!(slot_count, "prefetching known DeFi slots");
        }

        // Drain into CacheDB.
        let mut warm_db = CacheDB::new(EmptyDB::new());
        let mut fetched = 0usize;
        let mut failed = 0usize;

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(FetchResult::Account(addr, info))) => {
                    warm_db.insert_account_info(addr, info);
                    fetched += 1;
                }
                Ok(Ok(FetchResult::Storage(addr, slot, value))) => {
                    warm_db.insert_account_storage(addr, slot, value).ok();
                    fetched += 1;
                }
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "prefetch failed");
                    failed += 1;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "prefetch task panicked");
                    failed += 1;
                }
            }
        }

        tracing::info!(block_number, fetched, failed, "prefetch done");
        Ok(warm_db)
    }
}

/// Fetch account info with exponential backoff retry on 429.
async fn fetch_account_with_retry(
    p: &DynProvider,
    addr: Address,
    block_id: BlockId,
) -> Result<FetchResult, String> {
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = std::time::Duration::from_millis(200 * 2u64.pow(attempt - 1));
            tokio::time::sleep(delay).await;
        }

        let balance = p.get_balance(addr).block_id(block_id);
        let nonce = p.get_transaction_count(addr).block_id(block_id);
        let code = p.get_code_at(addr).block_id(block_id);

        let (balance, nonce, code) = tokio::join!(balance, nonce, code);

        // Check for 429 and retry.
        let is_rate_limited = balance
            .as_ref()
            .err()
            .map_or(false, |e| format!("{e}").contains("429"))
            || nonce
                .as_ref()
                .err()
                .map_or(false, |e| format!("{e}").contains("429"))
            || code
                .as_ref()
                .err()
                .map_or(false, |e| format!("{e}").contains("429"));

        if is_rate_limited && attempt < MAX_RETRIES {
            continue;
        }

        let balance = balance.map_err(|e| format!("{e}"))?;
        let nonce = nonce.map_err(|e| format!("{e}"))?;
        let code_bytes = code.map_err(|e| format!("{e}"))?;

        let bytecode = Bytecode::new_raw(code_bytes.0.into());
        let code_hash = bytecode.hash_slow();
        let info = AccountInfo::new(balance, nonce, code_hash, bytecode);

        return Ok(FetchResult::Account(addr, info));
    }
    Err(format!("max retries exceeded for {addr}"))
}

/// Fetch storage slot with exponential backoff retry on 429.
async fn fetch_storage_with_retry(
    p: &DynProvider,
    addr: Address,
    slot: alloy_primitives::U256,
    block_id: BlockId,
) -> Result<FetchResult, String> {
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = std::time::Duration::from_millis(200 * 2u64.pow(attempt - 1));
            tokio::time::sleep(delay).await;
        }

        match p.get_storage_at(addr, slot).block_id(block_id).await {
            Ok(val) => return Ok(FetchResult::Storage(addr, slot, val)),
            Err(e) => {
                let err_str = format!("{e}");
                if err_str.contains("429") && attempt < MAX_RETRIES {
                    continue;
                }
                return Err(err_str);
            }
        }
    }
    Err(format!("max retries exceeded for {addr} slot {slot}"))
}

/// Internal result type for the JoinSet drain loop.
enum FetchResult {
    Account(Address, AccountInfo),
    Storage(Address, alloy_primitives::U256, alloy_primitives::U256),
}
