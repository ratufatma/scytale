# Developer Guide

**Status:** Active

## Build and Test

```bash
cargo check --workspace
cargo test --workspace
cargo test -p scytale-sdk --features std
cargo test -p scytale-contract-vault
```

The default Cargo workspace members are listed in the root `Cargo.toml`.
Source code is organized into foundation, consensus, runtime, application, and
contract layers.

## Code Map

| Concern | Location |
|---|---|
| Primitive hashes and values | `crates/scytale-primitives` |
| Blocks, transactions, UTXO, genesis | `crates/scytale-core` |
| Consensus, difficulty, chain selection | `crates/scytale-consensus` |
| Mempool | `crates/scytale-mempool` |
| Mining | `crates/scytale-mining` |
| Persistent storage | `crates/scytale-storage` |
| Node orchestration and HTTP | `apps/scytale-node` |
| CLI and wallet | `apps/scytale-cli` |
| Wasm VM and SDK | `crates/scytale-vm`, `crates/scytale-sdk` |
| Contracts | `contracts/scy20`, `contracts/vault` |

## Change Workflow

1. Read the relevant canonical document in `docs/`.
2. Identify the owning source module and neighboring tests.
3. Update implementation and focused tests together.
4. Update the canonical document only when behavior is verified.
5. Add unresolved behavior to `IMPLEMENTATION-STATUS.md`.
6. Run the release checks in `AUDIT-MATRIX.md`.

Do not use archived working documents as protocol authority. When a protocol
value changes, update `PROTOCOL-REFERENCE.md` and its source-level test in the
same change.
