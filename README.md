# Scytale

**Scytale** is a modular, high-performance Layer-1 blockchain engine designed with:
- **Redb** for fast, embedded, ACID-compliant key-value storage.
- **Deterministic UTXO Model** with authenticated state commitment (`utxo_root` in 120-Byte BlockHeader).
- **Proof-of-Work (PoW)** consensus powered by CPU-friendly BLAKE3 hashing.
- **Deterministic Zero-Float Fee Market** with integer-only arithmetic.
- **Go P2P Wire Network** with binary chunked Fast Sync streaming (`getsnap` / `snapshot`).
- **Autonomous DNS Seeder** daemon for dynamic cold-start peer bootstrapping.

---

## 📁 Repository Structure

```text
scytale/
├── Cargo.toml                        # Workspace Manifest
├── README.md                         # Project Overview
├── Dockerfile                        # Multi-stage production container build
├── docker-compose.yml                # Cluster topology (Mining, Relay, Seeder, Coldstart)
├── crates/
│   ├── scytale-primitives/           # Core cryptographic types, BlockHash, & Bech32 encoding
│   ├── scytale-core/                 # Blocks, 120B Header, Transactions, UTXOs & Merkle trees
│   ├── scytale-script/               # Forth-like stack execution engine & opcodes
│   ├── scytale-storage/              # ACID redb engine, UTXO table, snapshots & indexers
│   ├── scytale-consensus/            # PoW validation, BLAKE3 target, emissions & reorg rules
│   ├── scytale-mempool/              # Transaction mempool & priority fee market
│   ├── scytale-mining/               # CPU miner worker, template builder & coinbase generation
│   └── scytale-bridge/               # Zero-copy binary IPC bridge between Rust and Go
├── apps/
│   ├── scytale-node/                 # Full node daemon, HTTP RPC gateway & supervisor
│   └── scytale-cli/                  # Wallet management, keygen, balance & miner operator CLI
├── network/                          # High-performance Go P2P subsystem
│   ├── cmd/
│   │   ├── scytale-p2p/              # P2P wire daemon subprocess
│   │   └── scytale-seeder/           # Autonomous DNS seeder daemon
│   └── internal/
│       ├── bridge/                   # Unix domain socket IPC framing
│       ├── gossip/                   # Transaction and block gossip relay
│       ├── peer/                     # Peer state machine & dynamic DNS seed resolver
│       ├── seeder/                   # Authoritative DNS server (:53) & health crawler
│       ├── sync/                     # Block sync state machine & out-of-order snapshot assembler
│       └── wire/                     # Binary frame codec & checksum verification
├── web/
│   └── explorer/                     # Embedded live block explorer and RPC dashboard
├── scripts/                          # Automated cluster verification & chaos test suites
└── docs/                             # Consolidated architecture & technical specifications
```

---

## 🚀 Getting Started

### Prerequisites
- **Rust**: 1.75+ (2021 edition)
- **Go**: 1.22+ (for P2P networking & DNS seeder daemon)
- **Docker & Docker Compose**: (optional, for multi-node cluster verification)

### Build & Check
```bash
# Build & check all Rust workspace crates
cargo check --workspace
cargo build --workspace
cargo test --workspace

# Build & test Go P2P network subsystem
cd network && go test -v -race ./... && cd ..
```

### Run Node
```bash
cargo run -p scytale-node -- --help
```

---

## 📖 Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Protocol Constants Registry](docs/PROTOCOL-CONSTANTS.md)
- [Consensus Specification](docs/CONSENSUS-SPEC.md)
- [Node Lifecycle Specification](docs/NODE-LIFECYCLE-SPEC.md)
- [Ledger Specification](docs/LEDGER-SPEC.md)
- [Storage Architecture Specification](docs/STORAGE-SPEC.md)
- [Mempool Specification](docs/MEMPOOL-SPEC.md)
- [P2P Networking Specification](docs/P2P-NETWORK-SPEC.md)
- [Automatic Mining Lifecycle Specification](docs/MINING-LIFECYCLE-SPEC.md)
- [Chain Selection & Reorganization Specification](docs/CHAIN-SELECTION-SPEC.md)
- [Block Specification](docs/BLOCK-SPEC.md)
- [Proof-of-Work Specification](docs/POW-SPEC.md)
- [Difficulty Adjustment Specification](docs/DIFFICULTY-SPEC.md)
- [Transaction Specification](docs/TRANSACTION-SPEC.md)
- [UTXO Specification](docs/UTXO-SPEC.md)
- [Value Provenance Specification](docs/VALUE-PROVENANCE-SPEC.md)
- [Authorization Specification](docs/AUTHORIZATION-SPEC.md)
- [Hashing and Serialization Specification](docs/HASHING-AND-SERIALIZATION-SPEC.md)
- [Economic Model](docs/ECONOMIC-MODEL.md)
- [Monetary Policy](docs/MONETARY-POLICY.md)
- [Genesis Specification](docs/GENESIS-SPEC.md)
- [Genesis Allocation Specification](docs/GENESIS-ALLOCATION.md)
- [Passbook Concept](docs/PASSBOOK-CONCEPT.md)
- [Testing Strategy & QA Framework](docs/TESTING-STRATEGY.md)
- [Security & Threat Model](docs/SECURITY-THREAT-MODEL.md)
- [Autonomous DNS Seeder & Cloudflare NS Delegation](docs/DNS-SEEDER-DEPLOYMENT-GUIDE.md)
- [Consolidated Milestone Specification: Tasks 32–38](docs/TASKS_32_TO_38.md)














