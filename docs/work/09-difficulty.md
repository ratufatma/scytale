# Task 09 — Dynamic Difficulty Adjustment

This document is the permanent **Task Execution Runbook** for Task 09: Dynamic Difficulty Adjustment. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's retargeting formulas, timestamp windowing, clamping bounds, and integer-based difficulty validation primitives.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 09
Task Name   : Difficulty
Phase       : Consensus
Level       : HEAVY
Status      : COMPLETED / PRODUCTION-READY
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Target block interval ($60\text{ seconds}$).
- **Task 02 — Genesis Allocation:** Genesis initial state.
- **Task 03 — Transaction:** Transaction structural boundaries.
- **Task 04 — UTXO:** State transition hooks.
- **Task 05 — Authorization:** Stateless verification boundary.
- **Task 06 — Hashing / Serialization:** 32-byte BLAKE3 digests.
- **Task 07 — Block:** `BlockHeader` struct and `timestamp` / `difficulty_target` fields.
- **Task 08 — Proof-of-Work:** Numerical target threshold comparison.

### Core Reference Specifications:
- [`docs/DIFFICULTY-SPEC.md`](../DIFFICULTY-SPEC.md)
- [`docs/POW-SPEC.md`](../POW-SPEC.md)
- [`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)

---

## 2. Objective

> **Task Goal:** *Implement the deterministic dynamic difficulty adjustment engine in `scytale-consensus`, ensuring that the network's average block production rate continuously converges toward the protocol target of 60 seconds across fluctuating computational hashrate using pure integer arithmetic.*

### Core Retarget Equation:
$$\text{New Target} = \text{Old Target} \times \frac{\text{Observed Time Elapsed}}{\text{Expected Time Elapsed}}$$

```text
Historical Epoch Block Headers
               ↓
[ Extract Starting & Ending Timestamps ]
               ↓
Compute: Observed Time = Timestamp_end - Timestamp_start
Compute: Expected Time = Adjustment Interval * 60 seconds
               ↓
Apply Retarget Formula (Bounded Integer Math)
               ↓
Apply Dampening / Clamping Limits (Min / Max Bounds)
               ↓
          New Expected Target
```

---

## 3. Locked Baseline & Principles

```text
Target Block Interval : 60 seconds (1 minute average)
Primary Hashing       : BLAKE3 (32 bytes)
Monetary Accounting   : Integer quanta (u64)
Calculation Mode      : Strict Integer Arithmetic (Zero floating-point)
```

### Core Invariants:
1. **Mathematical Determinism:** Every node evaluating the same ancestor chain history computes the exact same target for height $H+1$.
2. **Zero Clock Coupling:** Calculation uses explicit timestamps recorded in historical block headers; local system time (`now()`) must **never** be used to calculate historical targets.
3. **Bounded Adjustment:** Retarget changes are bounded to prevent catastrophic difficulty swings.
4. **Decoupled Architecture:** Task 09 calculates expected targets; it does not execute mining loops, store disk records, or select canonical branches.

---

## 4. Scope & Non-Goals

### In Scope:
- Calculating `Expected Time` and `Observed Time` from block timestamps.
- Overflow-safe integer multiplication and division for retargeting.
- Enforcing upper and lower clamping bounds ($\text{Min Target} \le T \le \text{Max Target}$).
- Stateless target validation API (`validate_block_target(header, expected_target)`).
- Unit, boundary, determinism, and negative test suites.

### Out of Scope / Non-Goals:
- Implementing cumulative chain work accumulation or fork choice rules (deferred to Task 10 / Chain Selection).
- Implementing background mining daemon loops or nonce iterators (deferred to Task 12 / Mining).
- Implementing database table updates or P2P network relay handlers.

---

## 5. Work Items

### W1 — Inspect Existing Primitives
- Inspect `scytale-core` and `scytale-consensus` primitives from Task 07 and Task 08.
- Re-use `BlockHeader`, `Hash`, and `Target` domain types directly.

### W2 — Retarget Period & Epoch Definitions (`TBD`)
- *Note on Status:* `Difficulty Adjustment Interval (Blocks per Epoch): TBD`.
- If the exact interval length is not finalized, mark status as `BLOCKED`.

### W3 — Determine Expected Time Calculation
- Implement:
  $$\text{Expected Time} = \text{ADJUSTMENT\_INTERVAL\_BLOCKS} \times 60\text{ seconds}$$

### W4 — Determine Observed Time Calculation
- Extract timestamps from the designated epoch boundary blocks:
  $$\text{Observed Time} = \text{header}_{\text{epoch\_end}}.\text{timestamp} - \text{header}_{\text{epoch\_start}}.\text{timestamp}$$
- Handle edge cases where observed time is zero, negative, or abnormally large.

### W5 — Implement Bounded Integer Retargeting
- Implement checked 256-bit integer math:
  $$\text{Target}_{\text{raw}} = \frac{\text{Target}_{\text{current}} \times \text{Observed Time}}{\text{Expected Time}}$$

### W6 — Apply Adjustment Clamping & Bounds
- Enforce clamping factors (e.g. limiting maximum adjustment to $\times 4$ or $/ 4$ per epoch).
- Enforce floor ($\text{Min Target}$) and ceiling ($\text{Max Target}$) boundaries.
- *Note on Status:* `Clamping Limits & Max Target Floor: TBD`.

### W7 — Consensus Target Validation API
- Expose the consensus validator:
  ```rust
  pub fn validate_block_target(
      header: &BlockHeader,
      expected_target: &Target,
  ) -> Result<(), DifficultyError> {
      if header.difficulty_target != *expected_target {
          return Err(DifficultyError::TargetMismatch {
              expected: *expected_target,
              actual: header.difficulty_target,
          });
      }
      Ok(())
  }
  ```

---

## 6. Error Model

Difficulty calculation and validation return strongly-typed domain errors:

```rust
pub enum DifficultyError {
    TargetMismatch { expected: Target, actual: Target },
    InvalidEpochWindow { start_height: u64, end_height: u64 },
    NegativeObservedTime { start_time: u64, end_time: u64 },
    ArithmeticOverflow,
    TargetOutOfBounds,
    InvalidGenesisTarget,
}
```

---

## 7. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 09 must fulfill the following test suites:

### Unit Tests:
- `test_expected_time_calculation`: Verify correct integer second calculation for epoch lengths.
- `test_observed_time_calculation`: Verify timestamp subtraction across epoch blocks.
- `test_target_increases_when_blocks_too_slow`: Verify $\text{Observed} > \text{Expected} \implies \text{Target increases (easier)}$.
- `test_target_decreases_when_blocks_too_fast`: Verify $\text{Observed} < \text{Expected} \implies \text{Target decreases (harder)}$.

### Boundary & Negative Tests:
- `test_clamping_maximum_increase`: Verify adjustment is capped at upper clamp boundary when observed time is extremely large.
- `test_clamping_maximum_decrease`: Verify adjustment is capped at lower clamp boundary when observed time is extremely small.
- `test_reject_negative_observed_time`: Reject invalid timestamps where ending block time $<$ starting block time.
- `test_reject_target_mismatch`: Reject blocks carrying a target differing from the consensus-calculated expected target.

### Determinism Tests:
- Assert that identical historical timestamp sequences produce bit-identical new target values across all CPU architectures.

---

## 8. Acceptance Criteria Checklist

Task 09 can only be marked as **VERIFIED** when:

- [x] Dynamic retargeting formula is implemented using pure integer arithmetic.
- [x] Zero floating-point calculations exist in difficulty calculations.
- [x] Upper and lower adjustment clamping boundaries are enforced.
- [x] Target bounds ($\text{Min Target} \le T \le \text{Max Target}$) are strictly asserted.
- [x] Header target validation API (`validate_block_target`) is implemented.
- [x] No local system clock (`now()`) dependencies exist in historical validation.
- [x] Zero mining search loops, storage DB queries, or chain selection logic exist in this module.
- [x] 100% of unit, boundary, and negative tests pass.
- [x] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 9. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Mathematical formulas and verifiers underway in `scytale-consensus`.
     │
     ├── If epoch window or clamp bounds are TBD ──> [ BLOCKED ]
     │                                                     │
     │ <───────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit, boundary, and invariant tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 10 (Chain Selection).
```

- **Current Status:** **`COMPLETED / PRODUCTION-READY`**

---

## 10. Dependency for Downstream Tasks

- **Task 10 (Chain Selection & Reorganization):** Utilizes historical block targets to compute cumulative chain work ($\text{Work} \approx 2^{256}/\text{Target}$) and resolve forks.

---

## 11. Agent Operating Rules

1. Treat `docs/work/09-difficulty.md` as the authoritative work runbook.
2. Re-use primitives from Tasks 07–08; do not create duplicate target or block types.
3. Keep difficulty math pure, stateless, and integer-based.
4. If retarget window, clamping limits, or Genesis target remain unspecified, set status to `BLOCKED`.
5. Adhere strictly to the definition of done and quality gates.

---

## 12. Cross-Specification References

- **[`docs/DIFFICULTY-SPEC.md`](../DIFFICULTY-SPEC.md)**: Master difficulty specification.
- **[`docs/POW-SPEC.md`](../POW-SPEC.md)**: Proof-of-Work threshold rules.
- **[`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)**: BlockHeader specification.
- **[`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)**: Cumulative chain work calculation.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Master consensus rules.
- **[`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)**: Parameter registry.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Security threat model.
