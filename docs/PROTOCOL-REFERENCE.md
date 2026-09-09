# Protocol Reference

**Status:** Active
**Version:** 0.3.0-devnet
**Canonical source:** Rust constants and constructors listed below

This is the single reference for protocol values. Other documents should link
here instead of repeating these numbers.

## Monetary Parameters

| Parameter | Canonical value | Source |
|---|---:|---|
| Smallest unit | `quanta` | `crates/scytale-primitives/src/lib.rs` |
| Quanta per SCY | `100,000,000` | `crates/scytale-primitives/src/lib.rs` |
| Maximum supply | `94,980,000 SCY` | `crates/scytale-core/src/genesis.rs` |
| Maximum supply | `9,498,000,000,000,000 quanta` | `crates/scytale-core/src/genesis.rs` |
| Genesis allocation | `66,000,000 SCY` | `crates/scytale-core/src/genesis.rs` |
| Founder output | `19,800,000 SCY` | `GENESIS_FOUNDER_QUANTA` |
| Treasury output | `13,200,000 SCY` | `GENESIS_DEVELOPER_QUANTA` |
| Ecosystem output | `33,000,000 SCY` | `GENESIS_COMMUNITY_QUANTA` |
| Mining reserve | `28,980,000 SCY` | `MINING_RESERVE_QUANTA` |
| Initial reward | `10 SCY` | `crates/scytale-consensus/src` |
| Halving interval | `2,100,000 blocks` | `crates/scytale-consensus/src` |
| Target block interval | `60 seconds` | `crates/scytale-consensus/src` |
| Reward end height | `3,696,000` | `MINING_REWARD_END_HEIGHT` |

## Canonical Genesis Outputs

Block 0 contains one bootstrap transaction with exactly three outputs:

| Index | Role | Amount | Locking script source |
|---:|---|---:|---|
| 0 | Founder | `1,980,000,000,000,000` quanta | `GENESIS_FOUNDER_LOCK_HEX` |
| 1 | Development / Treasury | `1,320,000,000,000,000` quanta | `GENESIS_DEVELOPER_LOCK_HEX` |
| 2 | Ecosystem / Community | `3,300,000,000,000,000` quanta | `GENESIS_COMMUNITY_LOCK_HEX` |

The generated key registry is `.genesis_keys.json`. It is local secret
material, ignored by Git, and must never be committed or published.

## Block and Transaction Encoding

- `BlockHeader` includes `utxo_root` and serializes to 120 bytes.
- `difficulty_target` is represented as compact `u32` in the header.
- `nonce` is `u64`.
- Transaction encoding includes `lock_time`.
- UTXO commitments use the canonical Merkle implementation in
  `crates/scytale-core/src/utxo.rs`.
- Fees and values use integer quanta only.

## Runtime Defaults

| Component | Default | Source |
|---|---|---|
| HTTP gateway | `0.0.0.0:8332` | `apps/scytale-node/src/http_gateway.rs` |
| P2P bind | `0.0.0.0:8333` | `apps/scytale-node/src/main.rs` |
| IPC socket | `/tmp/scytale.sock` | node and CLI sources |
| Data directory | `.scytale` from CLI | `apps/scytale-node/src/main.rs` |
| Mempool count limit | `5,000` | `crates/scytale-mempool/src` |
| Mempool byte limit | `5,000,000` | `crates/scytale-mempool/src` |
| Maximum reorg depth | `100` | `scytale-consensus` |

## Important Boundary

The reward schedule has an initial 10 SCY reward and a 2,100,000 block halving
interval, while the canonical mining reserve is 28.98M SCY. The terminal reward
rule at height 3,696,000 must remain documented and tested as consensus logic.
The theoretical unbounded halving sum must not be treated as an additional
allocation.
