# Scytale Block Specification

This document defines the formal specification for the **Block Structure**, block header fields, validation rules, and state transition mechanics within the Scytale blockchain engine.

---

## 1. Block Conceptual Model

In Scytale, a **Block** is a cryptographically secured batch of transactions that executes an atomic state transition over the global UTXO set:

```text
Block
├── Header
│   ├── version
│   ├── previous_block_hash
│   ├── transaction_commitment
│   ├── timestamp
│   ├── difficulty_target
│   └── nonce
│
└── Transactions[]
    ├── Transactions[0]  : Coinbase Transaction (Protocol Issuance & Fees)
    └── Transactions[1..]: Ordinary Transactions (Value Transfers)
```

### Architectural Principles:
1. **Header Metadata:** The block header encapsulates all fields necessary to establish block identity, chain linkage, and Proof-of-Work validation.
2. **Transaction Payload:** Contains the ordered sequence of transactions included in the block.
3. **Coinbase Isolation:** The first transaction (`index 0`) is strictly reserved for the protocol issuance and fee settlement.
4. **Deterministic Evaluation:** Validating a block against a given ledger state must produce identical results across all nodes.

---

## 2. Block Header Specification

The block header consists of six core fields evaluated by consensus:

| Field | Conceptual Type | Description | Specification Status |
| :--- | :--- | :--- | :--- |
| **`version`** | `u32` | Protocol version signaling consensus rule set applicability. | `TBD` |
| **`previous_block_hash`** | `[u8; 32]` | 32-byte cryptographic hash reference binding the block to its parent. | `32-byte Blake3 Digest` |
| **`transaction_commitment`** | `[u8; 32]` | Cryptographic commitment root over the block's ordered transaction set. | `Algorithm: TBD` |
| **`timestamp`** | `u64` | Block generation time recorded as Unix epoch seconds. | `Validation Window: TBD` |
| **`difficulty_target`** | `u32` / `[u8; 32]` | Target threshold required for Proof-of-Work satisfaction. | `Encoding: TBD` |
| **`nonce`** | `u64` / `u32` | Miner-adjusted counter variable used in Proof-of-Work evaluation. | `Size / Range: TBD` |

### Field Details:
- **`version`:** Distinguishes consensus rule eras and protocol upgrade milestones.
- **`previous_block_hash`:** Anchors the block to the existing blockchain history. For any non-genesis block, this must match the valid hash of the tip block in the active chain.
- **`transaction_commitment`:** A root digest committing to the exact membership and ordering of all transactions in the block. Prevents tampering with or reordering the transaction payload.
- **`timestamp`:** Records the block creation time and enforces forward-progress constraints.
- **`difficulty_target`:** Encodes the maximum numerical hash value permitted for a valid Proof-of-Work solution.
- **`nonce`:** An arbitrary field iterated by miners to discover a header hash satisfying the `difficulty_target`.

---

## 3. Block Identifier (`BlockID`)

The identity of a block is derived deterministically from its canonical header representation using Scytale's primary hashing primitive:

$$\text{BlockID} = \text{BLAKE3}(\text{Serialize}_{\text{canonical}}(\text{BlockHeader}))$$

- **Digest Size:** `32 bytes (256 bits)`.
- **Derivation Status:** `BlockID Derivation: TBD` (Final binary byte layout and domain separation prefix to be formalized during serialization locking).
- **Separation from TxID:** While both use 32-byte BLAKE3 digests, transaction identity (`TxID`) is computed over transaction data, whereas `BlockID` is computed over the block header.

---

## 4. Transaction List & Capacity Rules

Every block carries an ordered vector of valid transactions:

```text
Transactions Array
├── [0]  : Coinbase Transaction
├── [1]  : Transaction A
├── [2]  : Transaction B
└── [...]
```

### Ordering & Verification:
- Transaction order is immutable and committed to within the block header.
- Every ordinary transaction (`Transactions[1..]`) must be valid both independently and in sequential execution order against the evolving UTXO view of the block.
- Intra-block dependencies (e.g., Transaction B spending an output created by Transaction A within the same block) are governed by strict execution sequence rules.

### Block Capacity Constraints:
- `Maximum Block Size: TBD`
- `Maximum Transaction Count: TBD`
- `Block Capacity Metric: TBD`
- Finite block space creates the economic scarcity necessary for the transaction fee market.

---

## 5. Coinbase Transaction & Monetary Integration

The **Coinbase Transaction** is the exclusive protocol vehicle for new coin issuance:

```text
Coinbase Transaction
├── Inputs: Exactly 1 non-UTXO input (carrying arbitrary miner metadata / height)
└── Outputs: Miner reward distribution outputs (denominated in integer quanta)
```

### Consensus Constraints:
1. **First Position:** Must occupy `index 0` of the block's transaction vector. Exactly one coinbase transaction is permitted per block.
2. **Subsidy & Fee Cap:** Total coinbase output value cannot exceed the block subsidy plus all valid transaction fees:
   $$\sum \text{Coinbase Outputs (quanta)} \le R_{\text{quanta}}(height) + \sum_{k=1}^{N} \text{Fee}_{\text{quanta}}(Tx_k)$$
3. **Monetary Parameters:**
   - Initial Subsidy: $R_0 = 1,000,000,000\text{ quanta}$ ($10\text{ SCY}$).
   - Halving Interval: Every $2,100,000\text{ blocks}$.
   - Supply Target Ceiling: $4,200,000,000,000,000\text{ quanta}$ ($42,000,000\text{ SCY}$).
4. **Coinbase Maturity:** Coinbase outputs create new UTXOs subject to spending maturity rules:
   - `Coinbase Maturity Rule: TBD` (Number of confirmation blocks required before coinbase outputs can be spent).

---

## 6. Genesis Block

The **Genesis Block** is the initial anchor block (height $0$) of the Scytale network:

```text
Genesis Block (Height 0)
├── Header
│   ├── previous_block_hash = [0u8; 32] (Null Parent)
│   ├── timestamp           = Protocol Start Time
│   ├── difficulty_target   = Initial Baseline Target
│   └── nonce               = Valid Initial Nonce
└── Transactions[0]          = Genesis Coinbase / Fair-Launch Issuance
```

- **Null Predecessor:** The genesis block has no preceding parent block; its `previous_block_hash` is a protocol-defined null identifier.
- **Genesis Parameters:** `Genesis Parameters: TBD` (Exact timestamp, initial target, nonce, and issuance allocation remain open until testnet/mainnet launch parameterization).

---

## 7. Block Linkage & Chain Continuity

Blocks form a linear, cryptographically verified chain extending forward from genesis:

```text
Genesis (Height 0)
       ↓
Block 1 (Parent: Genesis)
       ↓
Block 2 (Parent: Block 1)
       ↓
Block 3 (Parent: Block 2)
       ↓
      ...
```

- **Strict Linkage:** Every non-genesis block must reference the exact `BlockID` of its immediate parent.
- **Orphan / Broken References:** Any block whose `previous_block_hash` does not reference a known, valid tip in the active chain is rejected.

---

## 8. Block Validity Rules (13 Core Consensus Checks)

To be accepted into the canonical ledger, a candidate block must satisfy all 13 consensus validation criteria:

1. **Header Structure Validity:** All header fields are well-formed and conform to protocol schema.
2. **Parent Reference:** `previous_block_hash` references a recognized, valid parent block in the active tree (or matches null for Genesis).
3. **Timestamp Bounds:** `timestamp` satisfies consensus window constraints relative to parent block timestamps.
4. **Difficulty Target Compliance:** The `difficulty_target` matches the protocol-calculated target for the current height/epoch.
5. **Proof-of-Work Verification:** The computed header hash satisfies the numerical threshold: $\text{BlockID} \le \text{difficulty\_target}$.
6. **Commitment Integrity:** The computed `transaction_commitment` over the transaction vector matches the header commitment root.
7. **Transaction Validity:** Every transaction in the block satisfies all individual validity and authorization checks.
8. **Coinbase Uniqueness:** The block contains exactly one coinbase transaction.
9. **Coinbase Position:** The coinbase transaction is located at `index 0`.
10. **Coinbase Value Limit:** Total coinbase outputs do not exceed $R(height) + \sum \text{Fees}$.
11. **State Transition Validity:** All input UTXOs exist in the active ledger and are unspent prior to block execution.
12. **No Intra-Block Double Spends:** No UTXO is consumed multiple times across different transactions within the block.
13. **Value Conservation:** No transaction generates unauthorized or unbacked monetary supply.

---

## 9. Block State Transition

Block validation executes an all-or-nothing atomic state transition:

```text
               Previous Active State (UTXO Set at Height H - 1)
                                      ↓
                         Verify Block Header & PoW
                                      ↓
                       Verify Transaction Commitments
                                      ↓
                   Sequential Transaction Validation & State Diff
                                      ↓
             Delete Consumed UTXOs  |  Insert Newly Created UTXOs
                                      ↓
                  New Active State (UTXO Set at Height H)
```

- **Atomic Commit:** If all 13 checks pass, the database batch transaction commits the UTXO state updates atomically.
- **Zero Partial Mutation:** If any individual check fails at any stage, the entire block is rejected and the ledger state remains completely unmodified.

---

## 10. Consensus Rules vs. Miner Policy

| Dimension | Consensus Rules (Enforced by All Nodes) | Miner Policy (Local Node Autonomy) |
| :--- | :--- | :--- |
| **Authority** | Universal, mandatory, immutable. | Configured locally by mining nodes. |
| **Enforcement** | Any violation causes immediate block rejection. | Governs only local block template construction. |
| **Scope** | - Proof-of-Work threshold compliance.<br>- Strict coinbase value cap ($\le \text{Subsidy} + \text{Fees}$).<br>- Valid parent hash linkage.<br>- Valid UTXO state transitions.<br>- Transaction commitment match. | - Transaction inclusion / exclusion heuristics.<br>- Fee density prioritization ordering.<br>- Mempool eviction thresholds.<br>- Nonce search algorithms and parallelization. |

---

## 11. Network Determinism

Scytale consensus enforces the following determinism guarantee:

> **Given an identical preceding ledger state and identical raw block bytes, every compliant node across the network must reach the exact same validity verdict and generate the exact same resulting UTXO state.**

Determinism is achieved through:
- Canonical binary serialization for headers and transactions.
- Pure integer quanta arithmetic for all balance, reward, and fee evaluations.
- Stateless, side-effect-free cryptographic verification routines.

---

## 12. Open Questions & Pending Specifications

The following parameters and mechanisms are designated as **TBD** and will be finalized in subsequent dedicated milestones:

| Area | Status | Target Specification Milestone |
| :--- | :--- | :--- |
| **Block Version Encoding** | `TBD` | Protocol upgrade signaling specification. |
| **Transaction Commitment Algorithm** | `TBD` | Cryptographic tree commitment specification (e.g., Merkle tree or hash accumulator). |
| **BlockID Derivation Formula** | `TBD` | Header serialization format & domain separation specification. |
| **Genesis Block Parameters** | `TBD` | Launch configuration milestone. |
| **Timestamp Acceptance Rules** | `TBD` | Consensus time window specification. |
| **Maximum Block Size / Capacity** | `TBD` | Block scale & throughput specification. |
| **Coinbase Maturity Period** | `TBD` | Output spendability delay specification. |
| **Proof-of-Work & Retarget Algorithm** | `TBD` | Dedicated [`docs/POW-SPEC.md`](POW-SPEC.md) milestone. |

---

## 13. Cross-Specification References

This document integrates with the complete Scytale architectural suite:
- **[`docs/ARCHITECTURE.md`](ARCHITECTURE.md)**: Modular crate structure and execution pipeline.
- **[`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md)**: Top-level UTXO ledger specification.
- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Transaction model, TxID, and validity rules.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint definitions and Value Provenance.
- **[`docs/AUTHORIZATION-SPEC.md`](AUTHORIZATION-SPEC.md)**: Locking conditions and signature validation.
- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md)**: BLAKE3 primitive and canonical serialization.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: Emission curve, halving schedule, and quanta accounting.
- **[`docs/ECONOMIC-MODEL.md`](ECONOMIC-MODEL.md)**: Miner revenue dynamics and fee markets.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: User-facing financial presentation layer.
