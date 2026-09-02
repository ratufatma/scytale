# Scytale Consensus Specification

This document defines the formal **Consensus Specification** for Scytale. It establishes the single conceptual boundary that governs all deterministic rules required for transactions, blocks, state transitions, and blockchain branches to be validated and accepted into canonical state.

---

## 1. Consensus Definition & Architectural Role

> **Definition:** *Consensus in Scytale is the strict, deterministic ruleset enforced uniformly across all nodes to verify transactions, validate blocks, execute state transitions, and converge on the canonical blockchain branch.*

Consensus serves as the supreme validator of truth:
- **Universal Uniformity:** Every compliant node executing the same inputs against identical rules produces the exact same validation output.
- **Fail-Closed Security:** Any transaction, block, or branch that violates a consensus invariant is rejected immediately with zero mutation to canonical state.
- **Independence from Transport:** Consensus validity is independent of how data was routed, received, or presented.

```text
Incoming Protocol Object (Transaction / Block / Branch)
                         │
                         ▼
             [ Consensus Engine Evaluation ]
            ├── INVALID ──> Rejected (Zero Canonical State Change)
            └── VALID   ──> State Transition Executed Atomically
```

---

## 2. Transaction Validity Invariants

A transaction is valid if and only if it satisfies all of the following rules:

1. **Structural Conformance:** The payload conforms strictly to the canonical binary encoding specified in [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md) and [`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md).
2. **Deterministic Identifier:** Possesses an unambiguous identifier:
   $$\text{TxID} = \text{BLAKE3}(\text{Serialize}_{\text{canonical}}(\text{Transaction}))$$
3. **Cryptographic Authorization:** Every input includes a valid cryptographic proof satisfying the `locking_condition` of the referenced UTXO as defined in [`docs/AUTHORIZATION-SPEC.md`](AUTHORIZATION-SPEC.md).
4. **UTXO Availability:** Every referenced input `OutPoint` exists in the active `UTXO_SET` and is unspent.
5. **Double-Spend Prevention:** No input `OutPoint` is consumed more than once within the transaction or within the same block.
6. **Solvency & Value Conservation:** Total input value must be greater than or equal to total output value:
   $$\sum \text{Input Values} \ge \sum \text{Output Values}$$
   $$\text{Transaction Fee} = \sum \text{Input Values} - \sum \text{Output Values} \ge 0$$
7. **No Arbitrary Minting:** Non-coinbase transactions cannot create new quanta out of thin air.

---

## 3. Block Validity Invariants

A block is valid if and only if it satisfies all of the following 13 core consensus validation checks (as specified in [`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)):

```text
Candidate Block Evaluation
├── 1. Structural Schema       : Valid header fields and binary payload format.
├── 2. Parent Linkage          : previous_block_hash references a known, valid ancestor block.
├── 3. Difficulty Target       : difficulty_target matches the calculated network target for this epoch.
├── 4. Proof-of-Work           : Numeric(BLAKE3(Header)) <= difficulty_target.
├── 5. Transaction Commitment  : transaction_commitment matches the root of the transaction vector.
├── 6. Transaction Count       : Block contains >= 1 transaction.
├── 7. Coinbase Isolation      : Exactly one coinbase transaction exists, located strictly at index 0.
├── 8. Coinbase Value Limit    : Coinbase output <= Block Subsidy R(height) + Sum of Transaction Fees.
├── 9. Transaction Validity    : Every included transaction [1..N] passes complete transaction validation.
├── 10. Intra-Block Solvency   : No double-spending of UTXOs within the block.
├── 11. State Transition Check : All input UTXOs are available in the pre-block UTXO_SET.
├── 12. Timestamp Validity     : Block timestamp satisfies monotonicity and drift limits.
└── 13. Monetary Conservation  : No violation of global supply caps or emission curves.
```

- Cross-References: [`docs/POW-SPEC.md`](POW-SPEC.md) and [`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md).

---

## 4. Monetary Invariants & Supply Constraints

Consensus enforces strict macro-economic limits across the entire blockchain lifecycle:

1. **Maximum Supply Ceiling:** Total circulating value can never exceed the immutable cap:
   $$\text{Total Issued Supply} \le 42,000,000\text{ SCY} \quad (4,200,000,000,000,000\text{ quanta})$$
2. **Denomination Standard:** All internal consensus accounting operates strictly in unsigned 64-bit integer quanta (`u64`):
   $$1\text{ SCY} = 100,000,000\text{ quanta}$$
3. **Genesis Allocation Limit:** Block 0 issuance is strictly locked to **25%** ($10,500,000\text{ SCY}$):
   - Founder Allocation: `15%` ($6,300,000\text{ SCY}$ / $630\text{T quanta}$) — one-time issuance, zero future mint authority.
   - Development / Treasury: `5%` ($2,100,000\text{ SCY}$ / $210\text{T quanta}$).
   - Ecosystem / Community: `5%` ($2,100,000\text{ SCY}$ / $210\text{T quanta}$).
4. **Mining Allocation Limit:** Proof-of-Work subsidies are locked to a maximum of **75%** ($31,500,000\text{ SCY}$ / $3,150\text{T quanta}$).
5. **Emission Schedule:** Base block subsidy begins at $10\text{ SCY}$ and reduces by $50\%$ every $2,100,000\text{ blocks}$.
- Cross-References: [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md), [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md), and [`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md).

---

## 5. State Transition Determinism

Every state transition in Scytale is pure and deterministic:

$$\text{State}_{H+1} = \text{ApplyBlock}(\text{State}_H, \text{Block}_{H+1})$$

```text
                  Current Canonical State (H)
                               │
                               ▼
               Validate Block (H+1) Consensus
                               │
                ├── INVALID ──> Drop Block (State Remains at Height H)
                └── VALID   ──> Apply Atomic State Mutation:
                                ├── Delete Consumed Input OutPoints
                                ├── Insert Newly Created Output OutPoints
                                └── Advance Tip to Height (H+1)
                               │
                               ▼
                    New Canonical State (H+1)
```

- **Atomicity Guarantee:** State transitions are committed all-or-nothing in `redb`. No partial state (e.g. block written without UTXO mutation) is ever permitted.

---

## 6. Canonical Chain Selection

When competing valid branches exist, the canonical chain is selected deterministically:

$$\text{Canonical Chain} = \arg\max_{C \in \mathcal{V}} \left( \sum_{B \in C} \text{Work}(B) \right)$$

- **Validity Precedes Work:** A branch must be 100% valid under all consensus rules before its cumulative Proof-of-Work is evaluated.
- **Cumulative Work Metric:** Selection is governed by total accumulated computational work, **not simple block count**.
- Cross-Reference: [`docs/CHAIN-SELECTION-SPEC.md`](CHAIN-SELECTION-SPEC.md).

---

## 7. Consensus Invariants vs. Local Node Policy

Scytale strictly separates universal consensus invariants from node-local operational policies:

| Dimension | Universal Consensus Invariants (Mandatory) | Local Node Policy (Node Discretion) |
| :--- | :--- | :--- |
| **Enforcement** | Uniformly enforced by 100% of network nodes. | Configured locally by individual node operators. |
| **Divergence Impact** | Disagreement causes a permanent consensus fork. | Disagreement has zero impact on consensus validity. |
| **Key Domains** | - Transaction structural and cryptographic validity.<br>- Proof-of-Work threshold checks ($\le \text{Target}$).<br>- Coinbase positioning and issuance value ceilings.<br>- UTXO solvency and double-spend rejection.<br>- Supply cap limits ($42,000,000\text{ SCY}$). | - Mempool fee ranking and sorting strategy.<br>- Transaction relay and flood rate limits.<br>- Mining block template selection criteria.<br>- Maximum mempool RAM allocation.<br>- Peer connection counts and ban thresholds. |

---

## 8. Consensus Dependency Hierarchy

The execution dependency layers of the consensus engine are structured as follows:

```text
                    Canonical Serialization & BLAKE3
                                   ↓
                   Transaction Cryptographic Authorization
                                   ↓
                       UTXO Solvency & Lineage
                                   ↓
                     Proof-of-Work & Block Structure
                                   ↓
                       Difficulty Retargeting
                                   ↓
                   Cumulative Chain Work Selection
                                   ↓
                    Atomic State Transition (redb)
                                   ↓
                        Canonical Ledger State
```

---

## 9. Open Consensus Questions & Parameter Status

The following consensus parameters and mechanisms remain designated as **TBD** and must be resolved prior to production freezing:

| Consensus Domain | Status | Specification Scope |
| :--- | :--- | :--- |
| **`BlockID Derivation`** | `TBD` | Finalization of domain-separated BLAKE3 header digest schema. |
| **`Transaction Commitment`** | `TBD` | Cryptographic tree algorithm (Merkle tree vs. BLAKE3 tree) for header commitment. |
| **`Canonical Serialization`** | `TBD` | Exact byte-level packing schemas for wire and storage serialization. |
| **`Authorization Algorithm`** | `TBD` | Specific signature suite (e.g. Ed25519 vs. Secp256k1) and locking script semantics. |
| **`Target Encoding & Retarget Window`** | `TBD` | Compact target representation and exact epoch interval length. |
| **`Timestamp Monotonicity Rules`** | `TBD` | Median-time-past calculation and maximum future drift window. |
| **`Coinbase Spending Maturity`** | `TBD` | Number of confirmation blocks required before mined UTXOs can be spent. |
| **`Equal-Work Tie-Break Rule`** | `TBD` | Formal deterministic tie-break criteria for equal cumulative work branches. |
| **`Settlement Finality Depth`** | `TBD` | Recommended confirmation depth for commercial transaction finality. |
| **`Mining Emission Halving Tail`** | `Requires Resolution` | Mathematical reconciliation between $31.5\text{M}$ mining cap and $10\text{ SCY} \rightarrow 5\text{ SCY} \dots$ infinite halving series. |

---

## 10. Cross-Specification References

- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Transaction format, inputs, outputs, and authorization rules.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: UTXO lifecycle, OutPoint primary keys, and state transitions.
- **[`docs/AUTHORIZATION-SPEC.md`](AUTHORIZATION-SPEC.md)**: Locking conditions and signature proofs.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block headers, coinbase placement, and 13 consensus checks.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: BLAKE3 Proof-of-Work evaluation.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: Dynamic difficulty adjustment.
- **[`docs/CHAIN-SELECTION-SPEC.md`](CHAIN-SELECTION-SPEC.md)**: Heaviest chain selection and atomic reorgs.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: 42M supply cap and quanta denomination.
- **[`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md)**: 15/5/5/75 macro distribution model.
- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Genesis block root anchor and zero-balance onboarding.
- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md)**: BLAKE3 hashing and determinism.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: Atomic commit and persistence in `redb`.
