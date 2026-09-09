# Task 08 — Proof-of-Work

This document is the permanent **Task Execution Runbook** for Task 08: Proof-of-Work (PoW). It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's BLAKE3-based computational proof evaluation, header target verification, and stateless PoW verification primitives.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 08
Task Name   : Proof-of-Work
Phase       : Consensus
Level       : HEAVY
Status      : COMPLETED / PRODUCTION-READY
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Emission interval target ($60\text{ seconds}$).
- **Task 02 — Genesis Allocation:** Genesis PoW boundary.
- **Task 03 — Transaction:** Transaction structural boundaries.
- **Task 04 — UTXO:** Solvency verification boundaries.
- **Task 05 — Authorization:** Proof separation.
- **Task 06 — Hashing / Serialization:** 32-byte BLAKE3 primitives and canonical codecs.
- **Task 07 — Block:** `BlockHeader` struct and `difficulty_target` / `nonce` fields.

### Core Reference Specifications:
- [`docs/POW-SPEC.md`](../POW-SPEC.md)
- [`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)
- [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)
- [`docs/DIFFICULTY-SPEC.md`](../DIFFICULTY-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)

---

## 2. Objective

> **Task Goal:** *Implement the pure, deterministic Proof-of-Work verification engine in `scytale-consensus`, establishing the mathematical validation rule where a block header is valid if and only if its canonical BLAKE3 32-byte hash digest is numerically less than or equal to the active difficulty target.*

### Core Proof Invariant:
$$\text{Numeric}(\text{BLAKE3}(\text{Serialize}_{\text{canonical}}(\text{BlockHeader}))) \le \text{difficulty\_target}$$

```text
Received Block Header
         ↓
[ Canonical Binary Serialization ]
         ↓
[ BLAKE3 Cryptographic Hash (32 bytes) ]
         ↓
[ Big-Endian 256-bit Numeric Interpretation ]
         ↓
Assert: Numeric(Hash) <= Expected Target
 ├── True  ──> Proof-of-Work VALID
 └── False ──> Proof-of-Work INVALID (Immediate Drop)
```

---

## 3. Locked Decisions & Architectural Boundaries

```text
Primary Hashing Primitive  : BLAKE3
Digest Length              : 32 bytes (256 bits)
Target Block Interval      : 60 seconds
```

### Verification vs. Mining Separation:
- **Consensus Verification (In Scope):** Stateless, fast, deterministic verification performed by 100% of validating nodes upon block receipt.
- **Mining Search (Out of Scope):** Operational loop exploring candidate nonces, parallel worker threads, and hardware optimizations (deferred to Task 12 / Mining). Consensus only evaluates the submitted solution header.

---

## 4. Scope & Non-Goals

### In Scope:
- Hashing canonical `BlockHeader` representations using BLAKE3.
- Numerical evaluation of 32-byte hash digests against target thresholds.
- Stateless PoW verification API (`verify_pow(header, target)`).
- Minimal single-threaded nonce search helper for test fixtures.
- Unit, boundary, determinism, and negative test suites.

### Out of Scope / Non-Goals:
- Implementing dynamic difficulty recalculation or retarget epochs (deferred to Task 09 / Difficulty).
- Implementing cumulative chain work accumulation or fork choice rules (deferred to Task 10 / Chain Selection).
- Implementing background mining daemon loops or multi-threaded worker pools (deferred to Task 12 / Mining).
- Implementing database table updates or P2P network relay handlers.

---

## 5. Work Items

### W1 — Inspect Existing Primitives
- Inspect `BlockHeader` from Task 07 and `Hash` from Task 06.
- Re-use types directly without creating redundant structs.

### W2 — Target Representation & Numerical Conversion
- Standardize the numeric interpretation of the 32-byte BLAKE3 hash digest as an unsigned 256-bit big-endian integer.
- *Note on Status:* `Target Compact Encoding Format: TBD`. If target encoding requires an unvetted representation, mark status as `BLOCKED`.

### W3 — Implement Hash-to-Target Comparison
- Implement deterministic comparison:
  ```rust
  pub fn check_pow_satisfaction(header_hash: &Hash, target: &Target) -> bool {
      // Numerical comparison: header_hash <= target
  }
  ```

### W4 — Implement Canonical Header Hash Computation
- Implement the pure hashing function:
  $$\text{compute\_pow\_hash}(\text{header}: \&\text{BlockHeader}) \rightarrow \text{Hash}$$
- Ensure hashing operates strictly over the canonical binary bytes of `BlockHeader` (never JSON, debug strings, or P2P envelopes).

### W5 — Nonce Integration
- Verify that iterating `header.nonce: u64` mutates the resulting `Hash` across the entire 256-bit output space.

### W6 — Stateless PoW Verification API
- Expose the core consensus validation function:
  ```rust
  pub fn verify_pow(header: &BlockHeader, expected_target: &Target) -> Result<(), PowError> {
      let hash = header.compute_pow_hash();
      if !check_pow_satisfaction(&hash, expected_target) {
          return Err(PowError::InsufficientWork { hash, target: *expected_target });
      }
      Ok(())
  }
  ```

### W7 — Minimal Candidate Nonce Testing Helper
- Provide a minimal utility function for test suites to iterate candidate nonces on ultra-low testing difficulty.

---

## 6. Zero Trust & Verification Invariants

Every validating node must independently execute local PoW recalculation:

$$\text{Peer Claim} \ne \text{Consensus Truth}$$

```text
Incoming Block from Peer ──> Extract BlockHeader ──> Local Canonical Serialize ──> Local BLAKE3 ──> Assert <= Target
```

- A node must **never** accept peer assertions or pre-computed validation flags without locally re-hashing the header.

---

## 7. Mathematical Relationship: Target vs. Computational Work

$$\text{Estimated Hashes to Solve} \approx \frac{2^{256}}{\text{Target}}$$

```text
Lower Numerical Target  ──> Smaller Valid Space ──> Higher Difficulty (More Work)
Higher Numerical Target ──> Larger Valid Space  ──> Lower Difficulty (Less Work)
```

---

## 8. Error Model

PoW evaluation returns strongly-typed domain errors:

```rust
pub enum PowError {
    InsufficientWork { hash: Hash, target: Target },
    InvalidTarget(String),
    SerializationFailure(String),
    MalformedHeader,
}
```

---

## 9. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 08 must fulfill the following test suites:

### Unit Tests:
- `test_pow_hash_reproducibility`: Assert identical header fields generate identical PoW hash digests.
- `test_nonce_mutation_changes_hash`: Assert changing `nonce` modifies the output hash.
- `test_valid_proof_acceptance`: Assert headers with $\text{Hash} \le \text{Target}$ return `Ok(())`.
- `test_invalid_proof_rejection`: Assert headers with $\text{Hash} > \text{Target}$ return `InsufficientWork`.

### Boundary Tests:
- `test_target_boundary_exact_match`: Assert $\text{Hash} == \text{Target}$ is accepted ($\le$).
- `test_target_boundary_off_by_one_above`: Assert $\text{Hash} == \text{Target} + 1$ is rejected.
- `test_target_boundary_off_by_one_below`: Assert $\text{Hash} == \text{Target} - 1$ is accepted.

### Determinism & Isolation Tests:
- Prove that PoW verification produces identical boolean outcomes across repeated executions with zero memory or clock dependencies.
- Use an ultra-low testing target ($2^{255}-1$) for instant test execution with zero CI CPU overhead.

---

## 10. Acceptance Criteria Checklist

Task 08 can only be marked as **VERIFIED** when:

- [x] BLAKE3 is integrated for canonical header hashing.
- [x] 32-byte numerical target comparison is implemented deterministically.
- [x] `BlockHeader.nonce` variation is supported and tested.
- [x] Stateless `verify_pow(header, target)` API is exposed.
- [x] Exact target boundary conditions ($==, > Target, < Target$) are mathematically proven.
- [x] Invalid and insufficient PoW headers are rejected immediately.
- [x] Zero storage, P2P network, or mining worker dependencies exist in the verifier.
- [x] 100% of unit, boundary, and negative tests pass.
- [x] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 11. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Stateless verifier implementation underway in `scytale-consensus`.
     │
     ├── If target encoding or integer schema is TBD ──> [ BLOCKED ]
     │                                                         │
     │ <───────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit, boundary, and invariant tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 09 (Difficulty).
```

- **Current Status:** **`COMPLETED / PRODUCTION-READY`**

---

## 12. Dependency for Downstream Tasks

- **Task 09 (Difficulty Adjustment):** Dynamically adjusts the `difficulty_target` based on historical block timestamps to maintain the 60-second target interval.
- **Task 10 (Chain Selection):** Accumulates block work ($\approx 2^{256}/\text{Target}$) to compute cumulative chain work.

---

## 13. Agent Operating Rules

1. Treat `docs/work/08-pow.md` as the authoritative work runbook.
2. Re-use primitives from Tasks 06–07; do not duplicate hash or header structs.
3. Keep PoW verification pure, stateless, and distinct from mining search loops.
4. If compact target encoding or Genesis target is unspecified, set status to `BLOCKED`.
5. Adhere strictly to the definition of done and quality gates.

---

## 14. Cross-Specification References

- **[`docs/POW-SPEC.md`](../POW-SPEC.md)**: Master Proof-of-Work specification.
- **[`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)**: Block and BlockHeader specification.
- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)**: BLAKE3 digests.
- **[`docs/DIFFICULTY-SPEC.md`](../DIFFICULTY-SPEC.md)**: Retargeting formulas and work calculation.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Consensus validation rules.
- **[`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)**: Parameter registry.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Security threat model.
