# Scytale Proof-of-Work Specification

This document defines the formal specification for the **Proof-of-Work (PoW)** consensus mechanism in Scytale. It establishes the mathematical and procedural rules by which computational work is evaluated, verified, and linked to block propagation.

---

## 1. Purpose & Role of Proof-of-Work

Proof-of-Work in Scytale provides an open, permissionless, and objective mechanism for decentralized block proposal and network consensus:

- **Permissionless Block Proposal:** Allows any participant to compete for the right to append valid blocks to the canonical ledger without central authorization.
- **Resource Expenditure Requirement:** Demands provable, unforgeable computational effort to propose a block, imposing tangible economic costs on block production.
- **Asymmetric Verification:** Finding a valid proof requires intensive computational iteration, while verifying a proof is instantaneous and requires only a single cryptographic hash operation by validating nodes.
- **Fair Competition:** Maintains a meritocratic, open competition across network participants.
- **Consensus Core:** Serves as a mandatory protocol validation invariant, not an optional mining feature.

---

## 2. Hashing Primitive: BLAKE3

Scytale standardizes on **BLAKE3** as its sole Proof-of-Work hashing primitive, as defined in [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md):

```text
Canonical Block Header Bytes
              ↓
            BLAKE3
              ↓
        32-Byte Hash
```

| Property | Specification | Description |
| :--- | :--- | :--- |
| **Hash Function** | `BLAKE3` | High-throughput, collision-resistant cryptographic hash. |
| **Output Width** | `32 bytes (256 bits)` | Fixed-size output digest evaluated against the difficulty target. |

---

## 3. Proof-of-Work Target & Numerical Condition

A candidate block header is considered to satisfy the Proof-of-Work requirement if and only if its computed BLAKE3 hash is numerically less than or equal to the active consensus target:

$$\text{PoW Valid} \iff \text{Numeric}(\text{BLAKE3}(\text{Serialize}_{\text{canonical}}(\text{BlockHeader}))) \le \text{Target}$$

```text
                  Computed Header Hash (32 bytes)
                                ↓
                 Interpreted as Large Integer
                                ↓
        [ Hash Value ]  <=  [ Active Consensus Target ]
             ├── TRUE   ==>  Valid Proof-of-Work
             └── FALSE  ==>  Invalid Proof-of-Work
```

### Specification Status:
- `Target Encoding: TBD` (Compact exponent-mantissa representation vs full 256-bit scalar encoding).
- `Hash Integer Interpretation: TBD` (Big-endian vs little-endian scalar mapping).
- `Comparison Boundary: TBD` (Exact numerical range constraints).

---

## 4. Mining Objective & Nonce Iteration

Miners iterate through candidate block headers by modifying permitted mining fields until discovering a hash that satisfies the target:

```text
               Block Header Template
                         ↓
         Modify Nonce / Permitted Mining Fields
                         ↓
               Canonical Serialization
                         ↓
                    BLAKE3 Hash
                         ↓
            Hash <= Difficulty Target?
              ├── NO  ──> Modify Nonce and Repeat
              └── YES ──> Valid Proof Found! Broadcast Block
```

- Each modification of the `nonce` alters the canonical binary input bytes, producing an independent, uniformly distributed BLAKE3 digest.
- **Mining Freedom:** Miners are free to utilize any search order, hardware architecture, or parallelization strategy to explore the candidate space.
- **Consensus Boundary:** The consensus engine only evaluates the validity of the final submitted header; the search methodology is outside consensus scope.

---

## 5. Nonce Field

The `nonce` field in the block header provides the primary variable search space for Proof-of-Work discovery:

```text
Block Header
├── version
├── previous_block_hash
├── transaction_commitment
├── timestamp
├── difficulty_target
└── nonce: [Field for miner state exploration]
```

- **Status:** `Nonce Width: TBD` (32-bit `u32` vs 64-bit `u64` width to be finalized in header encoding specification).
- **Secondary Search Spaces:** If the primary nonce space is exhausted within a single timestamp second, miners may adjust additional permitted header fields (e.g., coinbase extra-nonce data, which updates the `transaction_commitment`).

---

## 6. Block Header & PoW Evaluation Boundaries

Proof-of-Work verification operates strictly on the **canonical binary serialization** of the block header:

```text
Block Header Struct
        ↓
Canonical Binary Serialization (Strict Consensus Layout)
        ↓
BLAKE3 Digest (32 bytes)
        ↓
PoW Verification (Numeric Comparison against Target)
```

### PoW Validation vs. Block Identification:
- **PoW Hash:** The numerical value evaluated against `difficulty_target` during consensus validation.
- **Block Identifier (`BlockID`):** The canonical reference handle for the block within the chain DAG.
- Both use BLAKE3, but their domain separation and contextual usage are formally distinguished by the protocol.

---

## 7. Valid Proof Requirements

A block satisfies Proof-of-Work consensus if and only if all of the following conditions are met:

1. **Structural Integrity:** The block header conforms to the canonical schema defined in [`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md).
2. **Canonical Encoding:** The header serializes deterministically without ambiguity.
3. **Digest Generation:** The BLAKE3 hash produces an exact 32-byte digest.
4. **Target Threshold:** The numerical interpretation of the hash satisfies $\text{Hash} \le \text{Target}$.
5. **Consensus Target Alignment:** The `difficulty_target` recorded in the header matches the mathematically expected target calculated by the protocol for that block height.
6. **Parent Linkage:** The header correctly references a valid, active parent block.

---

## 8. Invalid Proof Conditions

A block is rejected immediately if:
- $\text{Hash} > \text{Target}$ (Proof-of-Work threshold not met).
- The `difficulty_target` in the header does not match the protocol's expected difficulty value.
- The header is malformed, contains non-canonical serialization, or references an unknown parent.
- The block violates any other ledger, transaction, or monetary consensus rule.

---

## 9. Difficulty & Target Relationship

The difficulty of the network is inversely proportional to the numerical target:

```text
Lower Numerical Target  ──>  Smaller Valid Solution Space  ──>  Higher Difficulty (More Work)
Higher Numerical Target ──>  Larger Valid Solution Space   ──>  Lower Difficulty (Less Work)
```

$$\text{Difficulty} \propto \frac{1}{\text{Target}}$$

---

## 10. Target Block Interval

Scytale targets an average block generation cadence of:

$$\text{Target Block Interval} = 60\text{ seconds}$$

### Stochastic Nature of Mining:
- The 60-second parameter represents the **statistical network average**, not a rigid guarantee for individual blocks.
- Due to the memoryless Poisson nature of Proof-of-Work, individual block intervals will naturally vary (some shorter, some longer).
- Long-term cadence is regulated through automated difficulty adjustments.

---

## 11. Difficulty Adjustment (Retargeting)

To maintain the 60-second average block interval across changing network hash rates, Scytale periodically adjusts the difficulty target.

> **Conceptual Goal:** Automatically adjust difficulty upward when blocks are discovered faster than 60 seconds on average, and downward when blocks are discovered slower than 60 seconds on average.

### Specification Status (To Be Detailed in Dedicated Retarget Milestone):
- `Difficulty Adjustment Algorithm: TBD`
- `Adjustment Interval (Epoch Length in Blocks): TBD`
- `Adjustment Damping / Smoothing Formula: TBD`
- `Minimum Target (Maximum Difficulty Ceiling): TBD`
- `Maximum Target (Minimum Difficulty Floor / Genesis Target): TBD`

---

## 12. Permissionless Miner Competition

Mining in Scytale is strictly permissionless and meritocratic:
- No special privileges, voting weights, or governance roles are granted based on stake, identity, reputation, or registration.
- Any participant with computing resources can generate valid candidate headers and broadcast winning blocks.
- The consensus engine evaluates purely objective computational proofs.

---

## 13. Mining Policy vs. Consensus Rules

| Domain | Consensus Rules (Universal) | Mining Policy (Local Miner Choice) |
| :--- | :--- | :--- |
| **Scope** | Enforced identically by every validating node. | Configured locally by individual miners. |
| **Enforcement** | Blocks violating rules are instantly rejected. | Affects only local search efficiency and block packing. |
| **Examples** | - $\text{Hash} \le \text{Target}$ verification.<br>- Expected target calculation.<br>- Valid block header linkage.<br>- Valid transactions and UTXO state diffs.<br>- Coinbase value limits. | - Nonce iteration strategy (sequential, random, SIMD).<br>- CPU / GPU / ASIC hardware utilization.<br>- Transaction selection and fee prioritization.<br>- Mempool admission filters.<br>- Extra-nonce manipulation schemes. |

---

## 14. Monetary & Ledger Integration

A block that satisfies the Proof-of-Work target is not valid unless it also satisfies all economic and ledger consensus invariants:

1. **Coinbase Value Limit:**
   $$\sum \text{Coinbase Outputs (quanta)} \le R_{\text{quanta}}(height) + \sum \text{Fees}_{\text{quanta}}$$
2. **Locked Monetary Parameters:**
   - Maximum Supply: `42,000,000 SCY` ($4,200,000,000,000,000\text{ quanta}$).
   - Initial Subsidy: `10 SCY` per block ($1,000,000,000\text{ quanta}$).
   - Halving Interval: Every `2,100,000 blocks` (~3.995 years).
   - Smallest Unit: `1 SCY = 100,000,000 quanta`.

---

## 15. Security Model & Chain Finality

- **Computational Work Anchor:** Proof-of-Work anchors the canonical history of the ledger in accumulated computational energy.
- **Reorganization Resistance:** Re-writing historical blocks requires re-computing the Proof-of-Work for the target block and all subsequent child blocks, imposing substantial computational and energetic costs.
- **Objective Fork Choice:** Nodes determine the canonical chain by evaluating the valid branch with the greatest accumulated Proof-of-Work (heaviest chain rule).

---

## 16. Open Questions & Pending Specifications

The following implementation parameters are designated as **TBD**:

| Area | Status | Description |
| :--- | :--- | :--- |
| **Target Encoding Format** | `TBD` | Compact representation (e.g., 32-bit floating exponent/mantissa) vs full 256-bit scalar. |
| **Hash Integer Interpretation** | `TBD` | Endianness convention for mapping 32-byte digests to 256-bit unsigned integers. |
| **Nonce Field Width** | `TBD` | Size of primary header nonce (`u32` vs `u64`). |
| **Difficulty Adjustment Algorithm** | `TBD` | Exact mathematical formula and window smoothing for periodic retargeting. |
| **Adjustment Epoch Interval** | `TBD` | Number of blocks between difficulty adjustments. |
| **Genesis Difficulty Target** | `TBD` | Initial difficulty target parameter for Block 0. |
| **Mining Field Expansion** | `TBD` | Rules for utilizing extra-nonce fields in coinbase or header. |

---

## 17. Cross-Specification References

- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block header structure, fields, and 13 core consensus checks.
- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md)**: BLAKE3 32-byte primitive and canonical encoding.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: Emission curve, 60s block target, and quanta accounting.
- **[`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md)**: UTXO state transitions and Value Provenance.
- **[`docs/ECONOMIC-MODEL.md`](ECONOMIC-MODEL.md)**: Miner revenue dynamics and fee markets.
