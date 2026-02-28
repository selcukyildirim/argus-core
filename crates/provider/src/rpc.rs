//! JSON-RPC provider backed by alloy-rs.

use crate::DataProvider;
use alloy_provider::{DynProvider, Provider, ProviderBuilder};
use argus_core::error::{ArgusError, ArgusResult};
use argus_core::Transaction;
use async_trait::async_trait;

/// Fetches blockchain data from an Ethereum JSON-RPC endpoint.
///
/// ```ignore
/// let provider = RpcProvider::connect("https://mainnet.infura.io/v3/KEY").await?;
/// ```
pub struct RpcProvider {
    provider: DynProvider,
    rpc_url: String,
}

impl RpcProvider {
    pub async fn connect(rpc_url: &str) -> ArgusResult<Self> {
        if rpc_url.is_empty() {
            return Err(ArgusError::InvalidInput("RPC URL must not be empty".into()));
        }

        let provider = ProviderBuilder::new()
            .connect(rpc_url)
            .await
            .map_err(|e| ArgusError::Provider(format!("Failed to connect to {rpc_url}: {e}")))?;

        tracing::info!(rpc_url, "connected");

        Ok(Self {
            provider: provider.erased(),
            rpc_url: rpc_url.to_string(),
        })
    }

    /// Returns the underlying `DynProvider` for use with `AlloyDB`.
    pub fn into_provider(self) -> DynProvider {
        self.provider
    }
}

#[async_trait]
impl DataProvider for RpcProvider {
    async fn get_block_transactions(&self, block_number: u64) -> ArgusResult<Vec<Transaction>> {
        use alloy_consensus::transaction::Transaction as TxTrait;

        tracing::debug!(block_number, rpc_url = %self.rpc_url, "fetching block");

        let block = self
            .provider
            .get_block_by_number(block_number.into())
            .full()
            .await
            .map_err(|e| {
                ArgusError::Provider(format!("Failed to fetch block {block_number}: {e}"))
            })?
            .ok_or_else(|| ArgusError::Provider(format!("Block {block_number} not found")))?;

        let transactions: Vec<Transaction> = block
            .transactions
            .into_transactions()
            .map(|tx| Transaction {
                hash: *tx.inner.tx_hash(),
                from: tx.inner.signer(),
                to: tx.to(),
                input: tx.input().clone(),
                value: tx.value(),
                gas: tx.gas_limit(),
            })
            .collect();

        tracing::info!(block_number, txs = transactions.len(), "fetched block");
        Ok(transactions)
    }

    async fn get_pending_transactions(&self) -> ArgusResult<Vec<Transaction>> {
        tracing::warn!("get_pending_transactions not implemented");
        Ok(Vec::new())
    }
}
