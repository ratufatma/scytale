# Implementation Status

**Status:** Active
**Last reviewed:** 2026-09-09

This document separates current behavior from design claims. `Implemented`
means the behavior exists in the repository and has a test or direct source
reference. `Gap` means documentation or implementation work remains.

## Implemented

| Area | Current state | Evidence |
|---|---|---|
| Genesis and tokenomics | 66M genesis, 94.98M maximum, 28.98M mining reserve | `crates/scytale-core/src/genesis.rs` |
| PoW | BLAKE3, target comparison, nonce search | `crates/scytale-consensus`, `crates/scytale-mining` |
| Difficulty | Retargeting, 1440-block epoch, 4x clamp | `crates/scytale-consensus/src/difficulty.rs` |
| UTXO commitment | Merkle root and validation | `crates/scytale-core/src/utxo.rs`, node validation |
| Storage | redb persistence and atomic commits | `crates/scytale-storage` |
| Mempool | fee ordering, limits, dependency tracking | `crates/scytale-mempool` |
| Wallet | Ed25519, mnemonic restore, encrypted key material | `apps/scytale-cli` |
| Smart contracts | Wasm validator runtime and CLI tooling | `crates/scytale-vm`, `apps/scytale-cli/src/contract.rs` |
| Vault contract | timelock and emergency penalty validation | `contracts/vault/src/lib.rs` and tests |
| SDK | no_std payload helpers and host crypto wrappers | `crates/scytale-sdk/src/lib.rs` and tests |
| HTTP gateway | status, blocks, transactions, mempool, passbook | `apps/scytale-node/src/http_gateway.rs` |

## Partial or Not Implemented

| Area | Current state | Action needed |
|---|---|---|
| Networking | NATS transport is the active network path | Extend NATS coverage as protocol features are added |
| DNS seeder | No seeder implementation is present | Mark deployment guide historical or implement the daemon |
| IBD | Locator helpers exist, full download/apply flow is incomplete | Add integration tests and protocol documentation |
| Mining template refresh | No complete event-driven refresh policy | Document polling/event behavior or implement it |
| Provenance DAG | Current query follows a single input path | Define merge semantics or narrow the specification |
| TxContext block height | SDK context has time but no block height | Add field only through a protocol decision |
| Emergency vault authorization | `emergency_key` is not used by `EmergencyRescue` | Decide whether penalty-only rescue is intended |
| Emission terminal rule | Reserve and end height exist; full schedule proof needs a dedicated test | Add cumulative emission test and formalize rounding |

## Historical or Superseded Material

- `docs/archive/work-history/` contains task runbooks and design history, not active protocol rules.
- `AUDIT_CHECKLIST_v0.3.0.md` contains historical claims that require a fresh
  verification before being used as release evidence.
- `TASKS_32_TO_34.md` is superseded by the later consolidated task document.
- The old Go P2P and DNS seeder documents are not evidence that those binaries
  exist in the current checkout.
