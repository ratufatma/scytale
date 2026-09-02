# Scytale Authorization Specification

This document defines the architectural specification for transaction authorization in Scytale. It establishes the conceptual framework by which a transaction proves its right to spend a referenced Unspent Transaction Output (UTXO).

---

## 1. Purpose & Conceptual Model

Authorization in Scytale is the deterministic validation process that verifies whether a transaction input is permitted to consume a specific UTXO encumbered by a `locking_condition`.

```text
               UTXO
                ↓
        Locking Condition
    (Encumbrance on Output)
                ↓
        Transaction Input
                ↓
       Authorization Data
   (Cryptographic Proof/Witness)
                ↓
           Validation
     (Deterministic Evaluator)
                ↓
     Spend Accepted / Rejected
```

### Core Question Answered by Authorization:
> **"Is this transaction legitimately authorized to consume the specific UTXO referenced by its `OutPoint`?"**

---

## 2. Specification Status & Decoupled Architecture

To maintain a clean and modular architecture, Scytale separates the structural transaction layout from concrete cryptographic signature schemes. The specific signature scheme and key serialization are deliberately designated as **TBD** at this baseline phase:

| Parameter | Baseline Status | Description |
| :--- | :--- | :--- |
| **Authorization Algorithm** | `TBD` | Concrete cryptographic algorithm (e.g., Ed25519, Schnorr, or Post-Quantum scheme). |
| **Public Key Format** | `TBD` | Canonical byte encoding for verification keys. |
| **Signature Encoding** | `TBD` | Fixed-size or variable-length byte representation for proofs. |
| **Scripting / Logic Model** | `None (Baseline)` | Pure encumbrance verification without complex, turing-complete VM overhead. |

> [!NOTE]
> Detailed cryptographic primitives, public key formats, and signature algorithms will be finalized in dedicated cryptographic milestones without requiring changes to the structural UTXO/Transaction engine.

---

## 3. Verification Pipeline & Semantics

When validating a transaction input against a referenced UTXO:

1. **Input Reference:** The input's `previous_output` identifies the target UTXO in the active ledger state.
2. **Context Binding:** The authorization proof must be cryptographically bound to the spending transaction's immutable identity (`TxID`) or canonical signing preimage to prevent signature replay attacks across different transactions.
3. **Execution:** The consensus evaluator passes the `TxIn.authorization` data into the verification routine corresponding to `TxOut.locking_condition`.
4. **Deterministic Decision:**
   - If the cryptographic proof satisfies the condition $\implies$ The input is **authorized**.
   - If verification fails or data is malformed $\implies$ The entire transaction is **rejected**.

---

## 4. Architectural Invariants

Scytale authorization enforces the following strict invariants:

1. **Stateless Verification:** Authorization validation requires only the transaction preimage, the input's authorization payload, and the referenced `TxOut.locking_condition`. It requires no external state, ambient environment variables, or non-deterministic inputs.
2. **Deterministic Outcome:** Given the same transaction and the same UTXO condition, every validating node across the network will yield the exact same acceptance or rejection result.
3. **No Malleability:** Modification of authorization data cannot alter the transaction's spending intent or compromise transaction integrity.
4. **Replay Protection:** An authorization proof generated for one transaction cannot be reused to authorize a different transaction referencing the same UTXO.
