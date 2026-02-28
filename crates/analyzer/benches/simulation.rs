use alloy_primitives::{Address, Bytes, B256, U256};
use argus_core::types::{AccessEntry, AccessMode, StorageLocation};
use argus_core::{AccessList, Transaction};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use smallvec::SmallVec;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_tx(i: u64) -> Transaction {
    Transaction {
        hash: B256::from(U256::from(i)),
        from: Address::from_word(B256::from(U256::from(i * 1000))),
        to: Some(Address::from_word(B256::from(U256::from(i * 2000)))),
        input: Bytes::new(),
        value: U256::ZERO,
        gas: 100_000,
    }
}

fn make_access_list(tx_idx: u64, n_entries: usize, overlap_ratio: f64) -> AccessList {
    let mut entries = SmallVec::new();
    for j in 0..n_entries {
        // Some entries overlap across txs (shared slots), some are unique.
        let slot_base = if (j as f64 / n_entries as f64) < overlap_ratio {
            j as u64 // shared across all txs
        } else {
            tx_idx * 1000 + j as u64 // unique per tx
        };

        entries.push(AccessEntry {
            location: StorageLocation {
                address: Address::from_word(B256::from(U256::from(tx_idx % 10))),
                slot: B256::from(U256::from(slot_base)),
            },
            mode: if j % 3 == 0 {
                AccessMode::Write
            } else {
                AccessMode::Read
            },
        });
    }
    AccessList {
        tx_hash: B256::from(U256::from(tx_idx)),
        entries,
    }
}

// ---------------------------------------------------------------------------
// Benchmark: simulate_batch (EmptyDB)
// ---------------------------------------------------------------------------

fn bench_simulate_batch(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("simulate_batch");
    for count in [10, 50, 100] {
        let txs: Vec<Transaction> = (0..count).map(make_tx).collect();
        group.bench_with_input(BenchmarkId::from_parameter(count), &txs, |b, txs| {
            b.to_async(&rt).iter(|| {
                let txs = txs.clone();
                async move {
                    black_box(
                        argus_analyzer::simulator::simulate_batch(txs)
                            .await
                            .unwrap(),
                    )
                }
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: conflict graph construction
// ---------------------------------------------------------------------------

fn bench_conflict_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("conflict_graph");
    for (tx_count, entries_per_tx) in [(50, 10), (100, 10), (200, 20), (500, 10)] {
        let access_lists: Vec<AccessList> = (0..tx_count)
            .map(|i| make_access_list(i, entries_per_tx, 0.3))
            .collect();

        group.bench_with_input(
            BenchmarkId::new(
                format!("{}tx_{}entries", tx_count, entries_per_tx),
                tx_count,
            ),
            &access_lists,
            |b, lists| {
                b.iter(|| black_box(argus_analyzer::graph::build_conflict_graph(lists)));
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: overlay (ref) vs clone
// ---------------------------------------------------------------------------

fn bench_overlay_vs_clone(c: &mut Criterion) {
    use revm::database::{CacheDB, EmptyDB};
    use revm::state::{AccountInfo, Bytecode};

    // Build a warm CacheDB with N accounts.
    let mut warm_db = CacheDB::new(EmptyDB::new());
    for i in 0u64..500 {
        let addr = Address::from_word(B256::from(U256::from(i)));
        warm_db.insert_account_info(
            addr,
            AccountInfo::new(U256::from(1000), i, B256::ZERO, Bytecode::new()),
        );
    }

    let txs: Vec<Transaction> = (0..100).map(make_tx).collect();

    let mut group = c.benchmark_group("overlay_vs_clone");

    // Overlay (current implementation - reference based)
    group.bench_function("overlay_ref", |b| {
        b.iter(|| {
            black_box(argus_analyzer::simulator::simulate_batch_with_state(&warm_db, &txs).unwrap())
        });
    });

    // Clone-based (old implementation for comparison)
    group.bench_function("clone_per_tx", |b| {
        b.iter(|| {
            use rayon::prelude::*;
            let results: Vec<_> = txs
                .par_iter()
                .map(|_tx| {
                    let _snapshot = black_box(warm_db.clone());
                    // We can't call simulate_one_tx (private), so just
                    // measure the clone cost overhead.
                })
                .collect();
            black_box(results)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simulate_batch,
    bench_conflict_graph,
    bench_overlay_vs_clone,
);
criterion_main!(benches);
