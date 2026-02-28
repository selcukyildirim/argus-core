//! Data provider abstraction and state prefetching for Argus.

pub mod labels;
pub mod prefetcher;
pub mod rpc;
pub mod slots;

use argus_core::error::ArgusResult;
use argus_core::Transaction;
use async_trait::async_trait;

pub use prefetcher::{Prefetcher, WarmCacheDB};

/// Abstraction for fetching transaction data from any source.
#[async_trait]
pub trait DataProvider: Send + Sync {
    async fn get_block_transactions(&self, block_number: u64) -> ArgusResult<Vec<Transaction>>;
    async fn get_pending_transactions(&self) -> ArgusResult<Vec<Transaction>>;
}
