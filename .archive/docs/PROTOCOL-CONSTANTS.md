# Scytale Protocol Constants & Parameter Registry

This document serves as the **central canonical registry** for all protocol parameters, architectural constants, and economic variables in Scytale. It provides a single, unambiguous reference for implementation engineers and auditors.

---

## 1. Official Asset Identity

The official nomenclature for the native asset of the Scytale network is defined as follows:

```text
Project / Protocol : Scytale
Native Coin        : Scytale Coin
Ticker / Symbol    : SCY
Smallest Unit      : quanta
Conversion         : 1 SCY = 100,000,000 quanta (10^8 quanta)
```

---

## 2. Locked Protocol Constants (`FINAL`)

The following parameters have been formally locked by protocol design decisions and constitute immutable baseline constants:

| Parameter Identifier | Locked Value | Unit | Status | Canonical Source Specification |
| :--- | :--- | :--- | :---: | :--- |
| **`PROJECT_NAME`** | `Scytale` | String | **FINAL** | [`README.md`](../README.md) |
| **`ASSET_NAME`** | `Scytale Coin` | String | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`ASSET_SYMBOL`** | `SCY` | String | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`SMALLEST_UNIT`** | `quanta` | String | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`QUANTA_PER_SCY`** | `100,000,000` ($10^8$) | Integer Quanta | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`MAXIMUM_SUPPLY_SCY`** | `94,980,000` | SCY | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`MAXIMUM_SUPPLY_QUANTA`** | `9,498,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md) |
| **`GENESIS_ALLOCATION_PERCENT`**| `69.49` | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`GENESIS_ALLOCATION_SCY`** | `66,000,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`GENESIS_TOTAL_QUANTA`** | `6,600,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`GENESIS_OUTPUT_COUNT`** | `3` | Integer Outputs | **FINAL** | [`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md) |
| **`FOUNDER_ALLOCATION_PERCENT`**| `30.0` of genesis | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`FOUNDER_AMOUNT_SCY`** | `19,800,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`FOUNDER_AMOUNT_QUANTA`** | `1,980,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`GENESIS_FOUNDER_QUANTA`** | `1,980,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`TREASURY_ALLOCATION_PERCENT`**| `20.0` of genesis | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`TREASURY_AMOUNT_SCY`** | `13,200,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`TREASURY_AMOUNT_QUANTA`** | `1,320,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`GENESIS_TREASURY_QUANTA`** | `1,320,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`ECOSYSTEM_ALLOCATION_PERCENT`**| `50.0` of genesis | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`ECOSYSTEM_AMOUNT_SCY`** | `33,000,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`ECOSYSTEM_AMOUNT_QUANTA`** | `3,300,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`GENESIS_ECOSYSTEM_QUANTA`** | `3,300,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`MINING_ALLOCATION_PERCENT`** | `30.51` | Percent (%) | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`MINING_ALLOCATION_SCY`** | `28,980,000` | SCY | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
| **`MINING_ALLOCATION_QUANTA`** | `2,898,000,000,000,000` | Integer Quanta | **FINAL** | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |
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

## 3. Pending Technical Specifications (`TBD`)

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
| **`FOUNDER_VESTING_SCHEDULE`** | Cliff and tranche lock rules for the 30% of-genesis founder allocation. | `TBD` | [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) |

---

## 4. Critical Parameters Requiring Resolution

> [!WARNING]
> ### Mathematical Reconciliation of Mining Emission Series
> 
> The current locked parameters contain a mathematical discrepancy that must be explicitly resolved prior to implementation:
> 
> 1. **Locked Allocation Model:**
>    - $\text{Total Maximum Supply} = 94,980,000\text{ SCY}$
>    - $\text{Genesis Allocation (69.49\%)} = 66,000,000\text{ SCY}$ (Founder 30\%, Treasury 20\%, Ecosystem 50\% of genesis)
>    - $\text{Mining Emission Allocation (30.51\%)} = \mathbf{28,980,000\text{ SCY}}$
> 
> 3. **The Conflict:**
>    - The unadjusted $10\text{ SCY}$ halving series sums to $42\text{M}$ SCY, exceeding the implemented mining reserve of $28.98\text{M}$ SCY.
>    - **Status:** **`Requires Resolution`** (The consensus engine must cap mining subsidies, adjust the initial reward/halving interval, or define an explicit terminal epoch). This must be decided by formal protocol decision rather than unauthorized implementation assumptions.

	- The unadjusted $10\text{ SCY}$ halving series sums to $42\text{M}$ SCY, exceeding the implemented mining reserve of $28.98\text{M}$ SCY.
	- **Status:** **`Requires Resolution`** (The consensus engine must cap mining subsidies, adjust the initial reward/halving interval, or define an explicit terminal epoch). This must be decided by formal protocol decision rather than unauthorized implementation assumptions.
	- The unadjusted $10\text{ SCY}$ halving series sums to $42\text{M}$ SCY, exceeding the implemented mining reserve of $28.98\text{M}$ SCY.
	- **Status:** **`Requires Resolution`** (The consensus engine must cap mining subsidies, adjust the initial reward/halving interval, or define an explicit terminal epoch). This must be decided by formal protocol decision rather than unauthorized implementation assumptions.
## 5. Cross-Specification References

- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: Monetary policy and emission specifications.
- **[`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md)**: Macro allocation distribution breakdowns.
- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Genesis block specifications.
- **[`docs/CONSENSUS-SPEC.md`](CONSENSUS-SPEC.md)**: Master consensus rules.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work threshold and target mechanics.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: Retargeting formulas.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: redb table architecture.
- **[`docs/P2P-NETWORK-SPEC.md`](P2P-NETWORK-SPEC.md)**: Go networking layer.
