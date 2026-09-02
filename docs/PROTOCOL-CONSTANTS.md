# Scytale Protocol Constants & Parameter Registry

This document serves as the **central canonical registry** for all protocol parameters, architectural constants, and economic variables in Scytale. It provides a single, unambiguous reference for implementation engineers and auditors.

---

## 1. Locked Protocol Constants (`FINAL`)

The following parameters have been formally locked by protocol design decisions and constitute immutable baseline constants:

| Parameter Identifier | Locked Value | Unit | Status | Canonical Source Specification |
| :--- | :--- | :--- | :---: | :--- |
| **`ASSET_SYMBOL`** | `SCY` | String | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`SMALLEST_UNIT`** | `quanta` | String | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`QUANTA_PER_SCY`** | `100,000,000` ($10^8$) | Integer Quanta | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`MAXIMUM_SUPPLY_SCY`** | `42,000,000` | SCY | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`MAXIMUM_SUPPLY_QUANTA`** | `4,200,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`GENESIS_ALLOCATION_PERCENT`**| `25.0` | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`GENESIS_ALLOCATION_SCY`** | `10,500,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`FOUNDER_ALLOCATION_PERCENT`**| `15.0` | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`FOUNDER_AMOUNT_SCY`** | `6,300,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`FOUNDER_AMOUNT_QUANTA`** | `630,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`TREASURY_ALLOCATION_PERCENT`**| `5.0` | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`TREASURY_AMOUNT_SCY`** | `2,100,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`TREASURY_AMOUNT_QUANTA`** | `210,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`ECOSYSTEM_ALLOCATION_PERCENT`**| `5.0` | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`ECOSYSTEM_AMOUNT_SCY`** | `2,100,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`ECOSYSTEM_AMOUNT_QUANTA`** | `210,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`MINING_ALLOCATION_PERCENT`** | `75.0` | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`MINING_ALLOCATION_SCY`** | `31,500,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`MINING_ALLOCATION_QUANTA`** | `3,150,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`NEW_USER_INITIAL_BALANCE`** | `0` | Quanta / SCY | **FINAL** | [`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md) |
| **`INITIAL_BLOCK_REWARD_SCY`** | `10` | SCY / block | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`INITIAL_BLOCK_REWARD_QUANTA`**| `1,000,000,000` | Quanta / block | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`BLOCK_INTERVAL_SECONDS`** | `60` | Seconds | **FINAL** | [`docs/POW-SPEC.md`](POW-SPEC.md) |
| **`HALVING_INTERVAL_BLOCKS`** | `2,100,000` | Blocks | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`REWARD_REDUCTION_FACTOR`** | `50.0` (Div by 2) | Percent (%) | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`HASH_FUNCTION`** | `BLAKE3` | Primitive | **FINAL** | [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md) |
| **`HASH_SIZE_BYTES`** | `32` | Bytes | **FINAL** | [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md) |
| **`LEDGER_MODEL`** | `UTXO` | Architecture | **FINAL** | [`docs/UTXO-SPEC.md`](UTXO-SPEC.md) |
| **`STORAGE_ENGINE`** | `redb` | Embedded DB | **FINAL** | [`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md) |
| **`CORE_PROTOCOL_RUNTIME`** | `Rust` (2021 edition) | Language | **FINAL** | [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) |
| **`P2P_NETWORK_RUNTIME`** | `Go` | Language | **FINAL** | [`docs/P2P-NETWORK-SPEC.md`](P2P-NETWORK-SPEC.md) |

---

## 2. Pending Technical Specifications (`TBD`)

The following parameters represent architectural components whose conceptual boundaries are specified but whose concrete binary formats or numerical constants are awaiting finalization:

| Parameter Identifier | Scope | Status | Source Specification |
| :--- | :--- | :---: | :--- |
| **`BLOCK_ID_DERIVATION`** | Domain-separated BLAKE3 header digest schema. | `TBD` | [`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md) |
| **`TRANSACTION_COMMITMENT`** | Merkle tree vs. BLAKE3 tree commitment over transaction vector. | `TBD` | [`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md) |
| **`CANONICAL_SERIALIZATION_FORMAT`** | Byte-level canonical encoding format for structs. | `TBD` | [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md) |
| **`AUTHORIZATION_ALGORITHM`** | Digital signature algorithm suite (Ed25519 vs. Secp256k1). | `TBD` | [`docs/AUTHORIZATION-SPEC.md`](AUTHORIZATION-SPEC.md) |
| **`GENESIS_DIFFICULTY_TARGET`** | Initial Proof-of-Work threshold for Block 0. | `TBD` | [`docs/POW-SPEC.md`](POW-SPEC.md) |
| **`DIFFICULTY_RETARGET_WINDOW`** | Exact block epoch interval for difficulty recalculation. | `TBD` | [`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md) |
| **`TARGET_COMPACT_ENCODING`** | Scientific exponent-mantissa compact encoding format. | `TBD` | [`docs/POW-SPEC.md`](POW-SPEC.md) |
| **`COINBASE_MATURITY_DEPTH`** | Confirmation blocks required before coinbase UTXOs become spendable. | `TBD` | [`docs/UTXO-SPEC.md`](UTXO-SPEC.md) |
| **`EQUAL_WORK_TIE_BREAK_RULE`**| Deterministic tie-breaking criteria for equal-work branches. | `TBD` | [`docs/CHAIN-SELECTION-SPEC.md`](CHAIN-SELECTION-SPEC.md) |
| **`SETTLEMENT_FINALITY_DEPTH`** | Recommended confirmation count for high-value transactions. | `TBD` | [`docs/CHAIN-SELECTION-SPEC.md`](CHAIN-SELECTION-SPEC.md) |
| **`FOUNDER_VESTING_SCHEDULE`** | Cliff and tranche lock rules for the 15% founder allocation. | `TBD` | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |

---

## 3. Critical Parameters Requiring Resolution

> [!WARNING]
> ### Mathematical Reconciliation of Mining Emission Series
> 
> The current locked parameters contain a mathematical discrepancy that must be explicitly resolved prior to implementation:
> 
> 1. **Locked Allocation Model:**
>    - $\text{Total Maximum Supply} = 42,000,000\text{ SCY}$
>    - $\text{Genesis Allocation (25%)} = 10,500,000\text{ SCY}$ (Founder 15%, Treasury 5%, Ecosystem 5%)
>    - $\text{Mining Emission Allocation (75%)} = \mathbf{31,500,000\text{ SCY}}$
> 
> 2. **Theoretical Infinite Halving Series from Baseline Formula:**
>    $$\text{Theoretical Mining Emission} = 10\text{ SCY} \times 2,100,000\text{ blocks} \times \sum_{k=0}^{\infty} \left( \frac{1}{2} \right)^k = 21,000,000 \times 2 = \mathbf{42,000,000\text{ SCY}}$$
> 
> 3. **The Conflict:**
>    - If the unadjusted $10\text{ SCY}$ halving series runs to infinity ($42\text{M}$ SCY) on top of the Genesis allocation ($10.5\text{M}$ SCY), total minted supply would reach **$52,500,000\text{ SCY}$**, violating the immutable $42,000,000\text{ SCY}$ ceiling.
>    - **Status:** **`Requires Resolution`** (The consensus engine must either cap mining subsidies when total mined reaches $31.5\text{M}\text{ SCY}$, adjust the initial reward/halving interval to mathematically sum to $31.5\text{M}\text{ SCY}$, or define an explicit terminal epoch). This must be decided by formal protocol decision rather than unauthorized implementation assumptions.

---

## 4. Cross-Specification References

- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: Monetary policy and emission specifications.
- **[`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md)**: Macro allocation distribution breakdowns.
- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Genesis block specifications.
- **[`docs/CONSENSUS-SPEC.md`](CONSENSUS-SPEC.md)**: Master consensus rules.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work threshold and target mechanics.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: Retargeting formulas.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: redb table architecture.
- **[`docs/P2P-NETWORK-SPEC.md`](P2P-NETWORK-SPEC.md)**: Go networking layer.
