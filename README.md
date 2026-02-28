# 🛡️ Argus

Argus is a high-performance execution profiler designed to identify and analyze **storage-slot contention** in Parallel EVM environments.

It simulates transactions against real-world state and maps **data hazards (RAW, WAW, WAR)** that trigger transaction re-execution in optimistic parallel engines such as Monad.

---

## ⚡ Why Argus?

Parallel EVMs rely on optimistic execution.

When multiple transactions modify the same storage slot within a block:

- The engine must abort  
- Then re-execute sequentially  
- Resulting in reduced parallel throughput  

Without storage-slot-level visibility, developers cannot:

- Detect contention hotspots  
- Understand parallel bottlenecks  
- Optimize contract layouts for throughput  

Argus provides precise storage-slot-level conflict detection.

---

## 🔥 The Core Problem: State Contention

State contention occurs when multiple transactions in the same block access overlapping storage slots.

This leads to:

- WAW — Write-After-Write  
- RAW — Read-After-Write  
- WAR — Write-After-Read  

In optimistic parallel execution engines like Monad, these hazards force fallback execution.

Argus detects and classifies them automatically.

---

## 🧠 Engineering Philosophy

Argus is built with a strict performance-first systems mindset.

### 🚀 Zero-Allocation Hot Path

- Uses `SmallVec`  
- Stack-allocated access tracking  
- Eliminates heap churn during simulation  

### ⚡ Zero-Copy Deduplication

- Uses `sort_unstable`  
- Uses `dedup_by`  
- Avoids `HashSet` hashing overhead  
- Operates directly on references  

### 🌊 Concurrent State Prefetching

- Powered by `tokio::task::JoinSet`  
- Parallel RPC state ingestion  
- CacheDB warmed before execution  
- Optimized for rate-limited public RPCs  

### 🧩 Data Hazard Classification

Automatically classifies:

- WAW — Write After Write  
- RAW — Read After Write  
- WAR — Write After Read  

### 📐 Contention Density Scoring

Each hotspot is scored by **Conflict Density** = `conflicts / affected_txs`:

| Density | Severity | Meaning |
|---------|----------|------------------------------------------|
| < 1.0   | LOW      | Normal contention, parallelizable         |
| 1.0–3.0 | MEDIUM   | Moderate bottleneck                       |
| 3.0–5.0 | HIGH     | Significant serialization pressure        |
| > 5.0   | CRITICAL | Block serializer — "network enemy"        |

### �️ Protocol Label Registry

45+ well-known Ethereum contracts are labeled instantly (no API calls):

- Uniswap V2/V3 (Router, Factory, Pools)  
- Tokens (WETH, USDC, USDT, DAI, WBTC, LINK, UNI)  
- Aave V2/V3, Curve, 1inch, OpenSea, Blur  
- Lido (stETH/wstETH), EigenLayer, MetaMask Swap Router  
- Unknown contracts fall back to address display  

---

## �🏗️ Architecture

### Provider

- Asynchronously fetches block data  
- Pre-fetches account and storage state  
- Works with any Ethereum-compatible RPC  

### Analyzer

- Executes transactions via optimized `revm`  
- Uses a custom Argus Inspector  
- Tracks storage-level reads and writes  

### Conflict Engine

- Post-processes access lists  
- Identifies slot overlaps  
- Generates dependency graphs  
- Computes contention density per contract  

### Data Sink

- **NDJSON Stream** — zero-alloc serialization via 64KB `BufWriter`  
- **StarRocks Stream Load** — HTTP PUT for OLAP analytics (feature-gated)  
- Three row schemas: `BlockSummary`, `ConflictRow`, `ContentionEvent`  

---

## 📊 ARGUS Analysis Report

**Block:** 21,000,000  
**Transactions:** 181  
**Storage Touches:** 304 entries across 133 txs  
**Total Conflicts:** 70  

| # | Severity | Protocol / Contract | Hazard | Txs | Conflicts | Density |
|---|----------|---------------------|--------|-----|-----------|---------|
| 1 | 🔴 CRITICAL | ERC-20 / Meme Token | WAW | 12 | 66 | **5.50** |
| 2 | 🟡 MEDIUM | MEV Bot / Aggregator | WAW | 3 | 3 | 1.00 |
| 3 | 🟢 LOW | MetaMask / Swap Router | WAW | 2 | 1 | 0.50 |


### 🔍 Observation

12 transactions competing for a single storage slot produced 66 WAW conflicts.

Argus identified this bottleneck using only public RPC infrastructure.

---

## 🗺️ Roadmap

### Phase 1 — Core Engine & CLI ✅

- revm Inspector integration  
- Zero-allocation hot path  
- Hazard classification  
- CLI block analyzer  

### Phase 2 — High-Performance Data Ingestion ✅

- Concurrent state prefetching  
- Zero-copy deduplication  
- Validation on Ethereum Mainnet (free RPC)  

### Phase 3 — Monad Optimization & Tooling 🛠️

- MonadDB compatibility research  
- I/O optimization (io_uring support)  
- Deferred execution contention profiling  
- Developer Dashboard (visual hotspot detection)  

---

## 📦 Workspace Structure

```
argus-core/
├── crates/core/           # Domain types, error handling
├── crates/provider/       # RPC, Prefetcher, Labels, DeFi Slots
├── crates/analyzer/       # Inspector, Graph, Reporter, Sink
└── crates/cli/            # CLI entry point
```

---

## 🚀 Getting Started

### Build

```bash
cargo build --release
```

### Analyze a Block

```bash
# Standard analysis
argus analyze --rpc-url $RPC_URL --block 21000000

# Export to NDJSON file
argus analyze --rpc-url $RPC_URL --block 21000000 --sink ndjson:output.ndjson

# JSON conflict graph output
argus analyze --rpc-url $RPC_URL --block 21000000 --json

# Dry run (EmptyDB — no RPC prefetch)
argus analyze --rpc-url $RPC_URL --block 21000000 --dry-run
```

### Environment Variable

```bash
export ARGUS_RPC_URL="https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY"
argus analyze --block 21000000
```
