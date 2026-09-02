# Task 05 — Authorization Model

This document is the permanent **Task Execution Runbook** for Task 05: Authorization Model. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's cryptographic ownership verification, input unlocking proofs, context binding, and anti-replay mechanics.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 05
Task Name   : Authorization
Phase       : Ledger
Level       : MEDIUM → HEAVY
Status      : VERIFIED
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Denomination and value invariants.
- **Task 02 — Genesis Allocation:** Genesis OutPoint encumbrance models.
- **Task 03 — Transaction:** `Transaction`, `TxIn` (`authorization`), and `TxOut` (`locking_condition`) structs.
- **Task 04 — UTXO:** UTXO resolution and input spending validation.

### Core Reference Specifications:
- [`docs/work/01-monetary-policy.md`](01-monetary-policy.md)
- [`docs/work/02-genesis-allocation.md`](02-genesis-allocation.md)
- [`docs/work/03-transaction.md`](03-transaction.md)
- [`docs/work/04-utxo.md`](04-utxo.md)
- [`docs/AUTHORIZATION-SPEC.md`](../AUTHORIZATION-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/LEDGER-SPEC.md`](../LEDGER-SPEC.md)
- [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)
- [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)

---

## 2. Objective

> **Task Goal:** *Implement the deterministic cryptographic authorization and ownership verification engine in `scytale-core`, establishing the consensus mechanism that proves whether the creator of a `TxIn` possesses the legitimate authority to spend the referenced `OutPoint` without introducing trusted third parties or off-chain state.*

### Core Evaluation Pipeline:
```text
Referenced UTXO (from Active Set)
                ↓
    TxOut.locking_condition
                ↓
    Spending TxIn.authorization
                ↓
[ Stateless Cryptographic Verifier ]
    ├── Valid Proof   ──> Accept Input & Continue State Transition
    └── Invalid Proof ──> Fail-Closed Rejection (Zero State Mutation)
```

---

## 3. Core Principles & Design Constraints

1. **Consensus-Critical Invariance:** Spend authorization is a foundational consensus rule; invalid authorization must halt block/transaction acceptance immediately.
2. **Deterministic & Stateless:** Verification depends strictly on explicit inputs (`locking_condition`, `authorization`, and transaction context) with zero environmental side-effects.
3. **UTXO Ledger Coupling:** Authorization bridges `TxOut.locking_condition` (encumbrance) with `TxIn.authorization` (witness/proof).
4. **Context-Bound Anti-Replay:** An authorization proof generated for Transaction A cannot be lifted and replayed in Transaction B.
5. **Separation from Key Management:** Task 05 implements the *verification engine*, not user-facing wallet storage, key generation, or passphrase managers.

---

## 4. Cryptographic Algorithm Policy & Status

> [!WARNING]
> ### Cryptographic Algorithm Status: `TBD`
> 
> The specific digital signature algorithm suite remains designated as **TBD** in [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md) and [`docs/AUTHORIZATION-SPEC.md`](../AUTHORIZATION-SPEC.md).
> 
> - **Operating Rule:** The executing agent must **NOT** unilaterally select or hardcode a specific cryptographic curve (e.g. Ed25519 vs. Secp256k1) without formal protocol consensus.
> - **Blocking Rule:** If implementation cannot proceed without a concrete cryptographic primitive, mark the task as:
>   `Status: BLOCKED` (Reason: *Authorization cryptographic algorithm not yet finalized*).

---

## 5. Scope & Non-Goals

### In Scope:
- Authorization domain models and proof structures in `scytale-core`.
- Encumbrance evaluation matching `TxOut.locking_condition` against `TxIn.authorization`.
- Transaction context digest generation for signature verification (signing preimage).
- Multi-input transaction authorization (verifying each input independently).
- Strongly-typed authorization error taxonomy.
- Unit, invariant, and negative authorization test suites.

### Out of Scope / Non-Goals:
- Implementing user wallet key storage, mnemonic seed phrases, or keystore encryption.
- Designing complex Turing-complete scripting languages (keep conditions minimal and ownership-focused).
- Implementing database lookups or `redb` storage bindings (provided externally via Task 04).
- Implementing Passbook presentation or signing UI components.
- Modifying monetary invariants or supply accounting rules.

---

## 6. Work Items

### W1 — Inspect Existing Primitives
- Inspect `scytale-core` structs from Task 03 and Task 04.
- Re-use `Transaction`, `TxIn`, `TxOut`, `OutPoint`, and `UtxoEntry` directly.

### W2 — Define Authorization Domain Boundary
- Establish clean verifier interfaces in `scytale-core`:
  ```rust
  pub trait AuthorizationVerifier {
      fn verify_input(
          &self,
          tx: &Transaction,
          input_index: usize,
          locking_condition: &[u8],
          authorization_proof: &[u8],
      ) -> Result<(), AuthorizationError>;
  }
  ```

### W3 — Define Locking Condition Container
- Treat `locking_condition` as a canonical byte sequence representing spending constraints (e.g. public key hash or ownership predicate).
- Keep condition semantics minimal, explicit, and extensible.

### W4 — Define Input Authorization Container
- Treat `TxIn.authorization` as the witness proof (e.g. signature + public key payload) that satisfies the locking condition.

### W5 — Implement Signing Context & Preimage Binding
- Construct the deterministic transaction digest over which authorization proofs are signed:
  $$\text{Signing Digest} = \text{BLAKE3}(\text{Serialize}_{\text{canonical}}(\text{Tx Preimage}))$$
- Ensure all inputs, outputs, version, and referenced OutPoints are committed into the digest.

### W6 — Verify Anti-Replay Isolation
- Prove that modifying any transaction field (e.g. output destination or value) invalidates the authorization proof.

### W7 — Implement Deterministic Verification
- Ensure verifier execution is pure and identical across all CPU architectures and platforms.

### W8 — Implement Error Taxonomy
- Define granular domain errors:
  ```rust
  pub enum AuthorizationError {
      EmptyAuthorization,
      MalformedProof,
      SignatureMismatch,
      KeyConditionMismatch,
      InvalidInputIndex(usize),
      UnsupportedVersion(u32),
      PreimageSerializationFailure,
  }
  ```

---

## 7. Multi-Input & Multi-Output Integration

- **Multi-Input Isolation:** In a transaction with $N$ inputs, each input references a distinct UTXO and must independently supply a valid `authorization` satisfying that specific output's `locking_condition`. One signature cannot implicitly authorize adjacent inputs unless explicitly designed by the signature scheme.
- **Multi-Output Encumbrance:** Transaction outputs establish new, unspent `locking_condition` constraints that govern future spending transactions.

```text
[ UTXO A (Owner 1) ] ──> Input 0 ──> Verified with Proof 1 ──┐
                                                             ├──> Transaction ──> New Output (Owner 3)
[ UTXO B (Owner 2) ] ──> Input 1 ──> Verified with Proof 2 ──┘
```

---

## 8. Architectural Boundaries & Decoupling

```text
┌─────────────────────────────────────────────────────────────────┐
│                    `scytale-core` (Task 05)                     │
│  - Pure, stateless authorization verification                   │
│  - Zero storage dependencies (No redb coupling)                 │
│  - Zero wallet / private key persistence                        │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                     `scytale-consensus`                         │
│  - Resolves UTXOs from storage via Task 04                      │
│  - Invokes Task 05 verifier during block & transaction admission│
└─────────────────────────────────────────────────────────────────┘
```

---

## 9. Security Requirements & Threat Mitigations

In accordance with [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md), the authorization layer must defend against:

| Threat Category | Attack Scenario | Defensive Invariant |
| :--- | :--- | :--- |
| **Unauthorized Spend** | Attacker crafts transaction spending someone else's UTXO. | Fails `SignatureMismatch` check; rejected immediately. |
| **Signature Forgery** | Attacker crafts synthetic proof without private key. | Cryptographic verification fails closed. |
| **Replay Attack** | Attacker copies valid proof from Tx A into Tx B. | Context digest mismatch invalidates signature. |
| **Malformed Proof DoS** | Attacker sends corrupted byte arrays to crash node. | Safe bounds-checked deserialization returns `MalformedProof`. |

---

## 10. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 05 must fulfill the following test suites:

### Unit Tests:
- `test_valid_single_input_authorization`: Prove valid proof unlocks corresponding UTXO.
- `test_valid_multi_input_authorization`: Prove multiple independent inputs verify correctly.
- `test_context_digest_determinism`: Prove identical transactions produce identical signing digests.

### Negative & Security Tests:
- `test_reject_missing_authorization`: Reject inputs with empty proof buffers.
- `test_reject_signature_tamper`: Modify 1 bit in signature buffer and assert rejection.
- `test_reject_transaction_mutation`: Modify output amount after signing and assert rejection.
- `test_reject_cross_transaction_replay`: Apply proof from Tx A to Tx B and assert rejection.
- `test_reject_wrong_public_key`: Use valid signature for wrong key condition and assert rejection.

---

## 11. Acceptance Criteria Checklist

Task 05 can only be marked as **VERIFIED** when:

- [x] Authorization verifier interfaces are implemented in `scytale-core`.
- [x] `TxOut.locking_condition` and `TxIn.authorization` evaluate deterministically.
- [x] Context-bound signing digest generation is implemented and tested.
- [x] Multi-input authorization verification is supported without cross-input leakage.
- [x] Anti-replay protection across differing transactions is verified.
- [x] Pure stateless verification is maintained (no `redb` or network coupling).
- [x] All negative and tamper-resistance tests pass with 100% success.
- [x] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 12. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Verification engine implementation underway.
     │
     ├── If signature primitive or serialization format is TBD ──> [ BLOCKED ]
     │                                                                   │
     │ <─────────────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit, negative, and invariant tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 06 (Hashing & Serialization).
```

- **Current Status:** **`VERIFIED`**

---

## 13. Dependency for Downstream Tasks

- **Task 06 (Hashing and Serialization):** Canonical wire and storage codecs will format transaction authorization vectors and signing preimages.

---

## 14. Agent Operating Rules

1. Treat `docs/work/05-authorization.md` as the authoritative work runbook.
2. Never choose or lock a cryptographic algorithm that is marked as `TBD` in specifications.
3. If an implementation blocker arises due to unspecified cryptographic choices, set status to `BLOCKED`.
4. Keep the verifier pure, stateless, and decoupled from storage and wallet infrastructure.
5. Adhere strictly to the definition of done and quality gates.

---

## 15. Cross-Specification References

- **[`docs/AUTHORIZATION-SPEC.md`](../AUTHORIZATION-SPEC.md)**: Master authorization specification.
- **[`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)**: Transaction model.
- **[`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)**: UTXO specification.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Threat model.
- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)**: Hashing digests.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Master consensus rules.
- **[`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)**: Parameter registry.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
