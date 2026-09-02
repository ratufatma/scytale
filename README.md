# Scytale

**Scytale** is a modular, pure-Rust, headless blockchain engine designed with:
- **Redb** for fast, embedded, ACID-compliant key-value storage.
- **UTXO Model** for transparent and concurrent transaction verification.
- **Proof-of-Work (PoW)** consensus with dynamic difficulty adjustment and smooth emission schedule.
- **Mempool** with prioritized fee-market validation.
- **Modular Architecture** partitioned across dedicated crates.

---

## 📁 Repository Structure

```text
scytale/
├── Cargo.toml                        # Workspace Manifest
├── README.md                         # Project Overview
├── crates/
│   ├── scytale-core/                 # Primitives & Cryptography
│   ├── scytale-storage/              # Redb Storage Engine
│   ├── scytale-consensus/            # PoW, Emission, & Validation
│   ├── scytale-mempool/              # Transaction Pool & Fee Market
│   └── scytale-network/              # P2P Layer (Stub)
└── apps/
    └── scytale-node/                 # CLI & Node Daemon
```

---

## 🚀 Getting Started

### Prerequisites
- Rust 1.75+ (2021 edition)

### Build & Check
```bash
cargo check --workspace
cargo build --workspace
cargo test --workspace
```

### Run Node
```bash
cargo run -p scytale-node -- --help
```

---

## 📖 Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Ledger Specification](docs/LEDGER-SPEC.md)
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







