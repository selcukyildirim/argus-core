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

---

## 🏗️ Architecture

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

---

## 📊 ARGUS Analysis Report

**Block:** 21,000,000  
**Transactions:** 181  
**Total Conflicts:** 70  

| Rank | Contract Type                     | Conflicts | Hazard Type | Slots | Txs |
|------|-----------------------------------|-----------|-------------|-------|-----|
| 1    | ERC-20 / Meme Token               | 66        | WAW         | 1     | 12  |
| 2    | MEV Bot / Multi-Token Aggregator  | 3         | WAW         | 1     | 3   |
| 3    | MetaMask / Swap Router            | 1         | WAW         | 1     | 2   |


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

## 🚀 Getting Started

### Build

```bash
cargo build --release
