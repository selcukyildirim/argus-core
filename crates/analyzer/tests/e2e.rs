//! E2E integration tests — require a live RPC endpoint.
//!
//! Run: `RPC_URL=https://... cargo test -p argus-analyzer -- --ignored`

use alloy_primitives::B256;

#[tokio::test]
#[ignore]
async fn analyze_real_block() {
    let rpc_url = std::env::var("RPC_URL").expect("Set RPC_URL to run E2E tests");
    let block_number = 19_000_000u64;

    let provider = argus_provider::rpc::RpcProvider::connect(&rpc_url)
        .await
        .expect("Failed to connect");
    use argus_provider::DataProvider;
    let txs = provider
        .get_block_transactions(block_number)
        .await
        .expect("Failed to fetch block");

    assert!(!txs.is_empty(), "Block should have transactions");
    eprintln!(
        "[e2e] Fetched {} transactions from block {}",
        txs.len(),
        block_number
    );

    let prefetcher = argus_provider::Prefetcher::new(provider.into_provider());
    let warm_db = prefetcher
        .prefetch(block_number, &txs)
        .await
        .expect("Prefetch failed");

    eprintln!(
        "[e2e] Prefetched state for {} accounts",
        warm_db.cache.accounts.len()
    );

    let access_lists = argus_analyzer::simulator::simulate_batch_with_state(&warm_db, &txs)
        .expect("Simulation failed");

    assert_eq!(access_lists.len(), txs.len());
    eprintln!("[e2e] Generated {} access lists", access_lists.len());

    let graph = argus_analyzer::graph::build_conflict_graph(&access_lists);
    eprintln!(
        "[e2e] Found {} conflicts across {} tx pairs",
        graph.len(),
        graph.adjacency.len()
    );

    assert!(
        !access_lists.iter().all(|al| al.entries.is_empty()),
        "At least some transactions should have storage accesses"
    );
}

#[tokio::test]
#[ignore]
async fn fetch_block_smoke() {
    let rpc_url = std::env::var("RPC_URL").expect("Set RPC_URL to run E2E tests");

    let provider = argus_provider::rpc::RpcProvider::connect(&rpc_url)
        .await
        .expect("Failed to connect");
    use argus_provider::DataProvider;

    let txs = provider
        .get_block_transactions(18_000_000)
        .await
        .expect("Failed to fetch block");

    assert!(!txs.is_empty(), "Block 18M should have transactions");

    for tx in &txs {
        assert_ne!(tx.hash, B256::ZERO, "tx hash should not be zero");
        assert!(tx.gas > 0, "gas limit should be positive");
    }

    eprintln!("[e2e] Block 18M: {} transactions, all valid", txs.len());
}
