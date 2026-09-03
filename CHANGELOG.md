# Changelog

All notable changes to the **Scytale** Layer-1 Blockchain Protocol are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [v0.3.0-devnet] - 2026-09-03

### Milestone: Phase 3 Completion — Programmable Consensus, Network Autonomy & State Authenticity

#### Added
- **Compact UTXO Commitment (`utxo_root`) in `BlockHeader` (Task 32)**:
  - Canonical 120-byte serialized block header incorporating 32-byte post-state commitment `utxo_root`.
  - Canonical lexicographical binary Merkle tree engine over active UTXOs with domain-separated BLAKE3 preimages (`SCYTALE_UTXO_LEAF_V1`).
  - Strict fail-closed consensus validation rule: rejects any block whose header `utxo_root` deviates from staging execution with `BlockError::InvalidUtxoRoot`.
  - Atomic snapshot export/import APIs (`export_utxo_snapshot`, `apply_utxo_snapshot`) on `StorageEngine`.
- **Fast Sync Wire Protocol (`getsnapshot` / `snapshot`) (Task 33)**:
  - Binary wire protocol commands `CmdGetSnapshot = "getsnap"` and `CmdSnapshot = "snapshot"`.
  - Chunked pagination streaming ($\le 2,000$ entries / chunk) to eliminate memory spikes.
  - Progressive `SnapshotAssembler` in Go P2P daemon to buffer and reconstruct out-of-order chunk streams safely.
  - Anti-DoS rate limiting: enforces minimum 30s interval between new snapshot export requests from a single peer.
  - Fail-closed snapshot application in Rust node daemon: verifies calculated Merkle root before committing state to `redb`.
- **Live Multi-Node Docker Cluster Chaos & Stress Suite (Task 34)**:
  - 4-node isolated topology on subnet `172.28.0.0/16` (`scytale-net`) via `docker-compose.yml`.
  - Automated test harness [scripts/chaos_stress_test.sh](scripts/chaos_stress_test.sh) covering:
    - **Scenario A**: Autonomous Mesh Discovery via `getaddr`/`addr`.
    - **Scenario B**: Dynamic Fee Market & Priority Mempool Telemetry.
    - **Scenario C**: Network Partition Split-Brain & Atomic Chain Reorganization.
    - **Scenario D**: End-to-End Fast Sync State Download & Merkle Verification.
- **Dynamic P2P Peer Discovery & Auto-Dialer (Task 29)**:
  - Binary wire messages `getaddr` and `addr` for neighborhood gossip.
  - Persistent `AddrBook` with scoring, attempt backoff, and local/private address filtering.
  - Active autonomous auto-dialer thread maintaining mesh connectivity.
- **Human-Readable Bech32 Addresses (`scy1...`) (Task 30)**:
  - BIP-173 Bech32 encoding/decoding with human-readable prefix `scy` (mainnet/devnet) and `tscy` (testnet).
  - Native integration across `scytale-cli` wallet, HTTP Gateway, and Web Explorer.
- **Dynamic Fee Market & Priority Mempool Eviction (Task 28)**:
  - Replace-By-Fee (RBF) and fee-density sorted mempool prioritization.
  - Automatic eviction of lowest fee density transactions when quota limits are reached.
  - Mempool inspector telemetry endpoint `GET /api/v1/mempool`.
- **Embedded Web Explorer Upgrade (Task 31)**:
  - Real-time mempool queue inspector with fee distribution breakdown.
  - Direct Bech32 address rendering and transaction search.

#### Changed
- Base container runtime updated to `ubuntu:24.04` ensuring GLIBC 2.39 binary compatibility with Ubuntu 24.04 hosts.
- `apps/scytale-cli` expanded to support both flag (`--start`, `--stop`) and positional syntax (`start`, `stop`) for mining commands.
- `apps/scytale-node` HTTP gateway extended to expose `utxo_root` and dynamic `peer_count` in `/api/v1/status`, plus `POST /api/v1/tx` endpoint.

#### Fixed
- **Lock Order Inversion**: Fixed cyclic mutex dependency in `apps/scytale-node/src/node.rs` (`submit_transaction` now invokes `canonical_height()` before acquiring `utxo_set` lock, preventing deadlock with mining thread).
- **P2P Listening Address Advertisement**: `scytale-p2p` now advertises listening port to remote peers upon handshake completion, allowing third-party mesh discovery without manual static seeding.
- **Sequential Chunk Rate Limiting**: `CanServeSnapshot` now rate-limits initial chunk requests (`chunkIndex == 0`) while permitting sequential streaming chunks without 30s delays.

---

## [v0.2.0-devnet] - 2026-09-02

### Milestone: Phase 2 Completion — Programmable Transactions, P2P Daemon & Wallet Ergonomics

#### Added
- **Stack-Based Script Execution Engine (`scytale-script`) (Task 25)**:
  - Forth-like stack machine supporting arithmetic, cryptographic hashing (BLAKE3), and Ed25519 signature checks.
  - Strict step and stack depth limits with bounded gas/execution budgets.
- **Consensus Script Verification & Sighash (Task 26)**:
  - SIGHASH_ALL canonical preimage computation for transaction authorization.
  - Support for P2PKH (Pay-to-Public-Key-Hash) and OP_RETURN data carrier outputs.
- **Non-Custodial CLI Wallet (`scytale-cli`) (Task 27)**:
  - Ed25519 key generation, encrypted profile keystores, and account management.
  - Subcommands: `wallet new`, `wallet list`, `transfer-p2pkh`, `embed-data`.
- **Live Go P2P Wire Integration (Tasks 19, 21)**:
  - Framing layer with magic bytes, commands, payload length, and checksum validation.
  - Two-step inventory gossip (`inv`, `getdata`, `tx`, `block`).
  - Initial Block Download (IBD) with block locator negotiation.
  - Unix domain socket IPC bridge linking Rust node with Go P2P daemon.
- **HTTP Gateway & Embedded Web Explorer (Task 23)**:
  - Axum-based HTTP REST API (`/api/v1/status`, `/api/v1/blocks`, `/api/v1/tx/:txid`).
  - Embedded responsive HTML5/Vanilla CSS Web Explorer.
- **Multi-Node Cluster & Live Fork Reorganization Harness (Tasks 21, 22, 24)**:
  - Deterministic reorganization test harness (`scripts/testnet_fork_reorg.sh`).
  - Dual-node sync testnet harness (`scripts/testnet_2node.sh`).

---

## [v0.1.0-devnet] - 2026-09-01

### Milestone: Phase 1 Completion — Core Monetary Policy & Blockchain Engine Baseline

#### Added
- **Core Primitives (`scytale-core`)**:
  - Exact integer arithmetic (Zero-Float invariant across all balances and rewards).
  - Canonical binary serialization and deserialization (`Codec`).
  - Cryptographic hashing via BLAKE3 (`scytale-primitives`).
- **UTXO Ledger & redb Storage (`scytale-storage`)**:
  - Embedded ACID key-value storage engine using Redb.
  - Tables: `BLOCKS`, `BLOCK_INDEX`, `TRANSACTIONS`, `UTXOS`, `META`.
- **Proof-of-Work Consensus (`scytale-consensus`, `scytale-mining`)**:
  - Target difficulty calculation and periodic adjustment.
  - Autonomous multithreaded CPU mining worker.
  - Canonical chain selection: heaviest cumulative proof-of-work rule.
- **Accounting & Auditability**:
  - Passbook accounting ledger (`passbook.rs`) tracking confirmed vs pending balances.
  - Value provenance tracing (`provenance.rs`) from coinbase origin to unspent tips.
