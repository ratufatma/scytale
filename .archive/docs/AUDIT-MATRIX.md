# Audit Matrix

**Status:** Active
**Purpose:** Give developers and auditors one traceable route from a claim to
code and tests.

## Core Traceability

| Requirement | Implementation | Verification | Documentation |
|---|---|---|---|
| Genesis output values | `crates/scytale-core/src/genesis.rs` | `cargo test -p scytale-core genesis` | `PROTOCOL-REFERENCE.md`, `GENESIS-SPEC.md` |
| Genesis key metadata | `.genesis_keys.json` | `jq` reconciliation and permission check | `PROTOCOL-REFERENCE.md` |
| Block header and `utxo_root` | `crates/scytale-core/src/block.rs`, `utxo.rs` | core block/UTXO tests | `BLOCK-SPEC.md`, `PROTOCOL-REFERENCE.md` |
| Reward and halving | `crates/scytale-consensus/src` | consensus tests | `MONETARY-POLICY.md` |
| Mempool limits | `crates/scytale-mempool/src` | mempool tests | `MEMPOOL-SPEC.md` |
| Node HTTP and IPC | `apps/scytale-node` | node and gateway tests | `NODE-LIFECYCLE-SPEC.md`, README |
| CLI wallet and mining | `apps/scytale-cli` | `cargo test -p scytale-cli` | README, `DEVELOPER-GUIDE.md` |
| Vault validator | `contracts/vault/src/lib.rs` | `cargo test -p scytale-contract-vault` | `IMPLEMENTATION-STATUS.md` |
| SDK codec and crypto | `crates/scytale-sdk/src/lib.rs` | `cargo test -p scytale-sdk --features std` | `IMPLEMENTATION-STATUS.md` |

## Release Checks

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo test -p scytale-sdk --features std
cargo test -p scytale-contract-vault
```

For repository documentation scans:

```bash
grep -RInE '42,000,000|10,500,000|31,500,000|15%|25%|75%' \
  README.md docs --include='*.md'
git diff --check
```

Any match in an active document must be reviewed. Matches in
`docs/archive/` may be historical, but should be labeled clearly.

## Audit Verdict Rules

- `Verified`: code and test evidence agree.
- `Partial`: code exists but coverage or specification is incomplete.
- `Not implemented`: documentation describes behavior absent from the checkout.
- `Historical`: retained for context and excluded from active protocol claims.

The consistency report and scan reports are inputs to this matrix, not
substitutes for current code/test verification.
