# Scytale Codebase Scan Report

**Date:** 2026-09-09
**Version:** 0.3.0-devnet
**Repository:** https://github.com/ratufatma/scytale.git
**Total Commits:** 110

---

## Executive Summary

Scytale is a modular Layer-1 blockchain engine written primarily in Rust with Go P2P networking, a Python CLI wrapper, and a React/Tauri desktop GUI. The codebase contains **107 Rust source files** (~27,608 LOC), **53 web files** (JS/HTML/CSS), **9 shell scripts**, **406 Python lines**, and **31+ specification documents**. It implements a full UTXO-based blockchain with PoW consensus, smart contracts (Wasm), and a complete developer/user toolchain.

---

## Codebase Statistics

| Metric | Count |
|--------|-------|
| Rust source files | 107 |
| Rust lines of code | ~27,608 |
| Web files (JS/MJS/HTML) | 53 |
| Shell scripts | 9 |
| Python files | 3 |
| Specification documents (.md) | 31+ (top-level) + 48 (work/) |
| Source files with `#[test]` | 49 |
| Total `#[test]` annotations | 233 |
| Workspace crates | 11 |
| Application binaries | 4 |
| Smart contracts | 2 |

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│                     USER INTERFACE LAYER                      │
│  scytale-studio (React/Tauri GUI)  |  scytale-cli (Rust CLI)│
│  scytale-py (Python wrapper)       |  explorer (Node.js web) │
└──────────────────────┬───────────────────────────────────────┘
                       │ IPC (Unix socket) / HTTP
┌──────────────────────▼───────────────────────────────────────┐
│                      NODE DAEMON                             │
│  scytale-node: HTTP Gateway │ IPC Server │ P2P Supervisor    │
│  Passbook │ Block Indexer │ NATS Networking                  │
└──────┬──────────┬──────────┬──────────┬─────────────────────┘
       │          │          │          │
┌──────▼───┐ ┌───▼────┐ ┌──▼───┐ ┌───▼──────┐
│Consensus │ │Mining  │ │Mem-  │ │ Storage  │
│  (PoW,   │ │Worker  │ │pool  │ │  (redb   │
│  Chain)  │ │        │ │      │ │  ACID)   │
└──────┬───┘ └───┬────┘ └──┬───┘ └───┬──────┘
       │         │         │         │
┌──────▼─────────▼─────────▼─────────▼───────────────────────┐
│                      CORE CRATES                            │
│  scytale-core (Tx, Block, UTXO, Address, Codec)            │
│  scytale-script (Stack Engine)  |  scytale-vm (Wasm VM)    │
│  scytale-sdk (no_std Wasm SDK)  |  scytale-primitives      │
└─────────────────────────────────────────────────────────────┘
```

---

## Crate-by-Crate Breakdown

### Foundation Layer

#### 1. `scytale-primitives`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-primitives/` |
| **Files** | 1 |
| **Purpose** | Fundamental types: `Hash` (BLAKE3 32-byte), `OutPoint`, `TxOut`, `Quanta` (10^8 per SCY), hex helpers |
| **Dependencies** | `blake3`, `serde`, `thiserror` |
| **Tests** | 5 inline |

#### 2. `scytale-sdk`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-sdk/` |
| **Files** | 1 |
| **Purpose** | `no_std` SDK for Wasm smart contracts: `TxContext`, bincode helpers, Ed25519 verification, BLAKE3 hashing |
| **Dependencies** | `serde` (no_std), `bincode`, `ed25519-dalek` (std), `blake3` (std) |
| **Tests** | 0 |

### Execution Layer

#### 3. `scytale-script`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-script/` |
| **Files** | 7 |
| **Purpose** | Non-Turing-complete stack-based script engine: arithmetic, BLAKE3, Ed25519 `OP_CHECKSIG`, `OP_CHECKLOCKTIMEVERIFY`, branching |
| **Modules** | `opcode`, `stack`, `engine`, `builder`, `context`, `error` |
| **Dependencies** | `blake3`, `thiserror`, `ed25519-dalek` |
| **Tests** | 2 external test files |

#### 4. `scytale-vm`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-vm/` |
| **Files** | 1 source + 2 test |
| **Purpose** | WebAssembly execution engine (wasmi-based) for eUTXO smart contract validation. Fuel-based gas metering, 4 MiB memory limit |
| **Dependencies** | `scytale-sdk`, `wasmi`, `blake3`, `ed25519-dalek` |
| **Tests** | 2 external + 1 inline |

### Core Types

#### 5. `scytale-core`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-core/` |
| **Files** | 10 source + 5 test |
| **Purpose** | Central type hub: `Transaction`, `Block`, `BlockHeader` (120 bytes), `UtxoSet`, `UtxoMerkleProof`, Bech32 `Address`, canonical codec, genesis block, VM adapter |
| **Modules** | `transaction`, `block`, `utxo`, `address`, `codec`, `genesis`, `authorization`, `vm_adapter`, `error` |
| **Dependencies** | `scytale-primitives`, `scytale-script`, `scytale-sdk`, `scytale-vm`, `bech32`, `bincode` |
| **Tests** | 5 external |

### Consensus Layer

#### 6. `scytale-consensus`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-consensus/` |
| **Files** | 7 source + 3 test |
| **Purpose** | PoW validation, BLAKE3 target evaluation, difficulty adjustment, chain tree with reorg support, emission curve (28.98M SCY mining reserve, halving, reward cessation at height 3,696,000) |
| **Modules** | `pow`, `difficulty`, `chain`, `target`, `work`, `error` |
| **Constants** | `INITIAL_REWARD`, `HALVING_INTERVAL`, `MINING_RESERVE_QUANTA`, `MINING_REWARD_END_HEIGHT`, `CLAMPING_FACTOR` |
| **Dependencies** | `scytale-primitives`, `scytale-core` |
| **Tests** | 3 external + 2 inline |

#### 7. `scytale-mempool`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-mempool/` |
| **Files** | 4 source + 2 test |
| **Purpose** | In-memory unconfirmed transaction pool with priority-based fee ordering |
| **Modules** | `pool`, `entry`, `error` |
| **Dependencies** | `scytale-primitives`, `scytale-core` |
| **Tests** | 2 external |

#### 8. `scytale-mining`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-mining/` |
| **Files** | 3 source + 1 test |
| **Purpose** | Block candidate template assembly and PoW mining loop |
| **Modules** | `worker`, `error` |
| **Dependencies** | `scytale-core`, `scytale-consensus`, `scytale-mempool` |
| **Tests** | 1 external |

### Storage & Networking

#### 9. `scytale-storage`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-storage/` |
| **Files** | 4 source + 2 test |
| **Purpose** | ACID storage engine (redb). 5 tables: `BLOCKS`, `BLOCK_INDEX`, `UTXOS`, `TRANSACTIONS`, `ADDRESS_TX_INDEX` + `CHAIN_STATE` meta. Atomic commits within single `redb::WriteTransaction` |
| **Modules** | `engine`, `tables`, `error` |
| **Dependencies** | `scytale-primitives`, `scytale-core`, `redb` |
| **Tests** | 2 external + 1 inline |

#### 10. `scytale-bridge`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-bridge/` |
| **Files** | 1 |
| **Purpose** | IPC and message framing bridge: CLI-to-Daemon IPC, Rust Node-to-Go P2P daemon RPC, async newline-delimited JSON framing |
| **Key Types** | `NodeRequest`/`NodeResponse`, `P2pBridgeRequest`/`P2pBridgeResponse`, `BridgeMessage` |
| **Dependencies** | `scytale-core`, `serde_json`, `tokio` |
| **Tests** | 3 inline |

### Account Management

#### 11. `scytale-account`
| Attribute | Detail |
|-----------|--------|
| **Path** | `crates/scytale-account/` |
| **Files** | 5 source + 1 test |
| **Purpose** | Optional local account-number alias wrapper for Scytale Passbook IDs. PIN protection (Argon2 + ChaCha20-Poly1305), candidate derivation, bind protocol |
| **Modules** | `candidate`, `pin_vault`, `protocol`, `store` |
| **Dependencies** | `argon2`, `chacha20poly1305`, `scytale-core`, `zeroize` |
| **Tests** | 1 external |

---

## Application Layer

### 1. `scytale-node` (Rust)
| Attribute | Detail |
|-----------|--------|
| **Files** | 14 source files, ~5,791 LOC |
| **Purpose** | Full blockchain node daemon |
| **Features** | HTTP REST gateway (axum), IPC server (Unix socket), P2P supervisor (Go bridge + NATS), block indexer, passbook, mining coordinator |
| **HTTP Endpoints** | `/block/<hash>`, `/blocks`, `/tx/<hash>`, `/balance/<address>`, `/status`, `/broadcast`, `/mempool`, `/passbook`, `/faucet` |
| **Key Deps** | `axum`, `tower-http`, `async-nats`, `crossbeam-channel`, `tokio` |

### 2. `scytale-cli` (Rust)
| Attribute | Detail |
|-----------|--------|
| **Files** | 10 source files, ~5,027 LOC |
| **Purpose** | Command-line wallet, contract, and node management |
| **Commands** | `wallet` (new/list/balance), `identity`, `node`, `contract` (inspect/build/deploy/call), `broadcast`, `passbook`, `faucet`, `genesis`, `mine`, `blocks`, `tx` |
| **Key Deps** | `clap`, `ed25519-dalek`, `bip39`, `ureq`, `rpassword` |

### 3. `scytale-py` (Python)
| Attribute | Detail |
|-----------|--------|
| **Files** | 1 source file (161 LOC) |
| **Purpose** | Bilingual (Indonesian/English) Python wrapper for `scytale-cli` |
| **Commands** | `buat` (create), `saldo` (balance), `kirim` (send), `riwayat` (history), `perbarui` (update) |
| **Deps** | Python stdlib only |

### 4. `scytale-studio` (React + Tauri 2)
| Attribute | Detail |
|-----------|--------|
| **Files** | 16 source/config files, ~1,462 LOC (frontend) |
| **Purpose** | Desktop GUI for wallet management, transaction building, mining monitoring, passbook, live terminal |
| **Features** | 4-panel workbench, xterm.js PTY terminal, hashrate charts, bilingual i18n, Gemini AI integration |
| **Key Deps** | React 19, Tauri 2, Vite, Tailwind CSS 4, xterm.js, recharts, i18next |

---

## Smart Contracts

### 1. `scy20` (SCY-20 Fungible Token Standard)
| Attribute | Detail |
|-----------|--------|
| **Files** | 6 source + 2 test |
| **Type** | `cdylib` + `rlib` (Wasm target) |
| **Operations** | `Transfer`, `Mint`, `Burn` |
| **Invariants** | Value conservation, token ID consistency, max supply cap, Ed25519 authorization |
| **Tests** | Unit + lifecycle + Wasm integration (comprehensive) |

### 2. `vault` (Timelocked Vault)
| Attribute | Detail |
|-----------|--------|
| **Files** | 1 source file (~131 LOC) |
| **Type** | `cdylib` + `rlib` (Wasm target) |
| **Operations** | `NormalWithdraw` (timelock + signature), `EmergencyRescue` (penalty fee) |
| **Tests** | None |

---

## Block Explorer (Node.js)

| Attribute | Detail |
|-----------|--------|
| **Tech Stack** | Express 5, better-sqlite3, Tailwind CSS, vanilla JS |
| **Features** | Block explorer, mempool inspector, passbook viewer, devnet faucet, block reconciler, node proxy, i18n (EN/ID) |
| **API** | REST endpoints for blocks, tx, status, faucet, ingest |
| **Deployment** | systemd service, rsync to VPS |

---

## Scripts & Infrastructure

| Script | Purpose |
|--------|---------|
| `test_docker_cluster.sh` | 3-node Docker cluster validation with height/hash verification |
| `deploy_explorer.sh` | Explorer deployment to VPS via rsync |
| `setup_vps_env.sh` | Production VPS provisioning (user, dirs, systemd, UFW) |
| `systemd/` | `scytale-node.service` + `scytale-explorer.service` with security hardening |

---

## Documentation

### Specification Documents (31 files)

| Category | Documents |
|----------|-----------|
| **Protocol** | BLOCK-SPEC, TRANSACTION-SPEC, UTXO-SPEC, HASHING-AND-SERIALIZATION-SPEC, PROTOCOL-CONSTANTS |
| **Consensus** | CONSENSUS-SPEC, POW-SPEC, DIFFICULTY-SPEC, CHAIN-SELECTION-SPEC, MINING-LIFECYCLE-SPEC |
| **Ledger** | LEDGER-SPEC, STORAGE-SPEC, MEMPOOL-SPEC, AUTHORIZATION-SPEC, VALUE-PROVENANCE-SPEC |
| **Economics** | ECONOMIC-MODEL, MONETARY-POLICY, GENESIS-SPEC, GENESIS-ALLOCATION |
| **Networking** | P2P-NETWORK-SPEC, DNS-SEEDER-DEPLOYMENT-GUIDE |
| **Architecture** | ARCHITECTURE, NODE-LIFECYCLE-SPEC, SMART_CONTRACTS, PASSBOOK-CONCEPT |
| **Quality** | TESTING-STRATEGY, SECURITY-THREAT-MODEL, AUDIT_CHECKLIST_v0.3.0 |
| **Milestones** | TASKS_28_TO_31, TASKS_32_TO_34, TASKS_32_TO_38 |

### Working Documents (48 files in `docs/work/`)
Numbered task designs (01 through 48) covering the full development timeline from monetary policy design through P2P hardening and passbook enhancement.

---

## Internal Dependency Graph

```
scytale-primitives ◄──────────────────────────────────────────────────┐
scytale-sdk (no_std) ◄────┐                                           │
                           ▼                                           │
                      scytale-vm                                       │
scytale-script ◄──────┐   │                                           │
                      ▼   ▼                                           │
                  scytale-core ◄──────────────────────────────────────┘
                      │
        ┌─────────────┼──────────────┬──────────────┐
        ▼             ▼              ▼              ▼
  scytale-account  scytale-bridge  scytale-storage  scytale-consensus
                                                  │
                                              ┌───┴───┐
                                              ▼       ▼
                                        mempool   (chained)
                                            │
                                            ▼
                                          mining
```

---

## Test Coverage Summary

| Crate | External Tests | Inline Tests | Test Files |
|-------|---------------|-------------|------------|
| scytale-primitives | 0 | 5 | 0 |
| scytale-sdk | 0 | 0 | 0 |
| scytale-vm | 2 | 1 | 2 |
| scytale-script | 2 | 0 | 2 |
| scytale-core | 5 | 0 | 5 |
| scytale-account | 1 | 0 | 1 |
| scytale-bridge | 0 | 3 | 0 |
| scytale-consensus | 3 | 2 | 3 |
| scytale-mempool | 2 | 0 | 2 |
| scytale-mining | 1 | 0 | 1 |
| scytale-storage | 2 | 1 | 2 |
| scy20 contract | 2 | 4 | 2 |
| vault contract | 0 | 0 | 0 |
| **TOTAL** | **20** | **16** | **20** |

**Total test annotations:** 233 across 49 source files.

---

## Key Technical Highlights

1. **120-byte BlockHeader** with `utxo_root` (Merkle commitment) for state authenticity
2. **BLAKE3 PoW** with CPU-friendly hashing and dynamic difficulty adjustment (60s target)
3. **Integer-only arithmetic** for deterministic fee market (zero-float policy)
4. **ACID storage** via redb with atomic block commits
5. **eUTXO smart contracts** running in wasmi Wasm VM with fuel-based gas metering
6. **Dual networking**: NATS async messaging + Go P2P daemon bridge
7. **Bech32 addresses** (`scy1...`) with human-readable encoding
8. **BIP-39 mnemonic** support for wallet recovery
9. **Bilingual UI** (Indonesian/English) across CLI, Studio, and Explorer
10. **Zero-float monetary policy**: 66M initial supply, 28.98M mining reserve, halving epochs, reward cessation at height 3,696,000

---

## Observations

- **vault contract** has no tests — should be added
- **scytale-sdk** has no tests — relies on downstream integration
- Working docs have duplicate naming (hyphen vs underscore) for tasks 34-41
- The project follows a spec-driven development approach with 31 formal specifications
- All crates use workspace-inherited versioning and edition settings
- `Cargo.toml` has `float_arithmetic = "deny"` enforced via clippy lint
