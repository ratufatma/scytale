# Task 01 — Monetary Policy

This document is the permanent **Task Execution Runbook** for Task 01: Monetary Policy. It guides agents and engineers in executing, verifying, and delivering the monetary policy foundation for Scytale.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 01
Task Name   : Monetary Policy
Phase       : Economy
Level       : LIGHT
Status      : VERIFIED
Dependency  : None
```

### Architectural Role:
Task 01 establishes the deterministic macro-economic rules for the Scytale engine. It serves as the prerequisite foundation for:
- Genesis Allocation (Task 02)
- Coinbase Construction & Block Subsidy Calculation
- Proof-of-Work Mining Emission
- Global Supply Invariant Enforcement
- Transaction Fee Conservation
- Consensus Monetary Validation

---

## 2. Objective

> **Task Goal:** *Define and mathematically reconcile Scytale's monetary policy so that every validating node independently computes currency issuance, block rewards, circulating supply, and the immutable maximum supply ceiling with 100% deterministic integer precision.*

### Success Attributes:
- **Fixed & Immutable:** Hard supply ceiling with zero discretionary minting authority.
- **Pure Integer Precision:** Zero floating-point calculations in monetary math.
- **Fully Auditable:** Mathematical reconciliation across all allocation buckets.
- **Zero Hidden Issuance:** Every quantum is transparently traceable on the ledger.
- **User Comprehensibility:** Clear, consistent presentation across Passbook interfaces.

---

## 3. Locked Architectural Baseline

The following parameters are formally locked and must **NOT** be altered by this task:

```text
Project / Protocol       : Scytale
Native Coin Name         : Scytale Coin
Ticker / Symbol          : SCY
Smallest Accounting Unit : quanta
Denomination Rate        : 1 SCY = 100,000,000 quanta (10^8 quanta)

Maximum Supply Ceiling   : 42,000,000 SCY (4,200,000,000,000,000 quanta)

Genesis Allocation (25%) : 10,500,000 SCY (1,050,000,000,000,000 quanta)
├── Founder Allocation   : 15% ( 6,300,000 SCY / 630,000,000,000,000 quanta) [One-time]
├── Treasury / Dev       :  5% ( 2,100,000 SCY / 210,000,000,000,000 quanta)
└── Community / Ecosystem:  5% ( 2,100,000 SCY / 210,000,000,000,000 quanta)

Mining Reserve (75%)     : 31,500,000 SCY (3,150,000,000,000,000 quanta)

Initial Block Reward     : 10 SCY / block (1,000,000,000 quanta / block)
Target Block Interval    : 60 seconds
Halving Interval         : 2,100,000 blocks
Reward Reduction Rate    : 50% per epoch

New User Initial Balance : 0 SCY (Permissionless onboarding)
Primary Hashing Primitive: BLAKE3 (32 bytes)
Ledger Model             : UTXO
Storage Engine           : redb
P2P Network Runtime      : Go
Core Protocol Runtime    : Rust
```

---

## 4. Known Mathematical Discrepancy & Consensus Issue

> [!WARNING]
> ### CONSENSUS ISSUE — REQUIRES RESOLUTION
> 
> A mathematical discrepancy exists between the locked allocation percentages and the baseline halving formula:
> 
> 1. **Locked Allocation Rule:**
>    - Genesis Allocation = $10,500,000\text{ SCY}$ ($25\%$)
>    - Mining Emission Allocation = $\mathbf{31,500,000\text{ SCY}}$ ($75\%$)
>    - Total Ceiling = $\mathbf{42,000,000\text{ SCY}}$ ($100\%$)
> 
> 2. **Baseline Halving Series Calculation:**
>    $$\text{Theoretical Mined Sum} = 10\text{ SCY} \times 2,100,000\text{ blocks} \times \sum_{k=0}^{\infty} \left(\frac{1}{2}\right)^k = 21,000,000 \times 2 = \mathbf{42,000,000\text{ SCY}}$$
> 
> 3. **The Mismatch:**
>    - Genesis ($10.5\text{M}$) + Theoretical Mining ($42\text{M}$) = **$52,500,000\text{ SCY}$**, which breaches the $42\text{M}$ maximum supply cap.
>    - **Mandatory Operating Rule:** The executing agent must **NOT** silently invent a parameter fix or make arbitrary assumptions. This issue must be explicitly documented and flagged as requiring formal protocol consensus resolution before consensus code is written.

---

## 5. Scope & Non-Goals

### In Scope:
- Asset denomination and integer `quanta` conversion.
- Mathematical reconciliation of all allocation categories to $42,000,000\text{ SCY}$.
- Definition of the macro supply equation ($\text{Genesis} + \text{Mining} + \text{Unissued} = \text{Max Supply}$).
- Specification of consensus monetary invariants (conservation of value, non-inflationary fees, coinbase ceilings).
- Cross-specification audit of monetary terminology.

### Out of Scope / Non-Goals:
- Writing Rust source code or creating new crates.
- Implementing transaction, block, mempool, or storage data structures.
- Creating genesis block binary payloads or choosing recipient addresses.
- Implementing signature verification or cryptographic authorization algorithms.
- Modifying locked supply caps ($42\text{M}$) or allocation percentages ($15/5/5/75$).

---

## 6. Work Items

The executing agent must perform the following work items:

### W1 — Verify Denomination Consistency
- Validate that $1\text{ SCY} = 100,000,000\text{ quanta}$ ($10^8$).
- Ensure all monetary arithmetic and internal accounting can be represented as `u64` integer quanta without floating-point operations.
- *Acceptance:* Zero monetary rules require floating-point arithmetic.

### W2 — Verify Supply Allocation Reconciliation
- Audit allocation quotas in integer quanta:
  - Founder: $630,000,000,000,000\text{ quanta}$ ($6.3\text{M}$)
  - Treasury: $210,000,000,000,000\text{ quanta}$ ($2.1\text{M}$)
  - Ecosystem: $210,000,000,000,000\text{ quanta}$ ($2.1\text{M}$)
  - Mining: $3,150,000,000,000,000\text{ quanta}$ ($31.5\text{M}$)
  - Total: $4,200,000,000,000,000\text{ quanta}$ ($42\text{M}$)
- *Acceptance:* Sum of all categories equals exactly $4,200,000,000,000,000\text{ quanta}$.

### W3 — Define Emission Relationship
- Formulate the immutable macro relationship:
  $$\text{Maximum Supply} = \text{Genesis Allocation} + \text{Issued Mining Rewards} + \text{Unissued Mining Reserve}$$
- *Acceptance:* No currency issuance pathway exists outside this mathematical boundary.

### W4 — Document Reward Schedule Discrepancy
- Document the exact mathematical conditions under which the $10\text{ SCY}$ halving series interacts with the $31.5\text{M}$ mining cap.
- *Acceptance:* Explicitly designated as `CONSENSUS ISSUE — REQUIRES RESOLUTION` across all related documents.

### W5 — Formalize Consensus Monetary Invariants
- Codify the core monetary invariants:
  1. $\text{Total Circulating Supply} \le 42,000,000\text{ SCY}$.
  2. For ordinary transactions: $\sum \text{Inputs} = \sum \text{Outputs} + \text{Fee}$.
  3. For coinbase transactions: $\text{Coinbase Value} \le R(\text{height}) + \sum \text{Fees}$.
  4. Transaction fees transfer existing value; they never create new quanta.
  5. All amounts are strictly unsigned 64-bit integers (`u64`).

### W6 — Cross-Document Consistency Audit
- Cross-check monetary definitions across:
  - `docs/MONETARY-POLICY.md`
  - `docs/ECONOMIC-MODEL.md`
  - `docs/GENESIS-ALLOCATION.md`
  - `docs/GENESIS-SPEC.md`
  - `docs/CONSENSUS-SPEC.md`
  - `docs/PROTOCOL-CONSTANTS.md`
- Ensure zero contradictions, zero hidden assumptions, and clear `TBD` / `Requires Resolution` markings.

---

## 7. Deliverables

1. A verified, mathematically consistent monetary policy specification.
2. Formally documented supply reconciliation in integer quanta.
3. Explicit documentation of the mining emission halving discrepancy.
4. Formal consensus monetary invariants.
5. Updated `docs/PROTOCOL-CONSTANTS.md` registry.
6. Clean cross-specification alignment across all documentation.

---

## 8. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), future implementation of this task must be accompanied by the following test suites:

### Unit Tests (During Code Construction):
- `test_quanta_to_scy_conversion`: Precise string formatting and parsing.
- `test_integer_arithmetic_safety`: Checked arithmetic asserting zero overflow on max supply multiplications.
- `test_allocation_sum_exactness`: Verifying that $630\text{T} + 210\text{T} + 210\text{T} + 3,150\text{T} == 4,200\text{T}$.
- `test_block_reward_halving_curve`: Epoch subsidy calculation across halving boundaries.

### Consensus Invariant Tests:
- `test_oversupply_block_rejection`: Rejecting blocks where coinbase output exceeds subsidy + fees.
- `test_non_coinbase_inflation_rejection`: Rejecting transactions where output sum exceeds input sum.
- `test_zero_fee_conservation`: Asserting solvency when fee equals zero.

### Reality & End-to-End Tests (Future Implementation Phase):
- Launch a real node process, mine Block 1 on low difficulty, and verify that the created coinbase UTXO equals exactly $10\text{ SCY}$ ($1,000,000,000\text{ quanta}$).
- Verify that a fresh node with 0 SCY dynamically derives a positive balance in the Passbook viewer upon block confirmation.

---

## 9. Acceptance Criteria Checklist

Task 01 is considered **COMPLETE** when all of the following criteria are satisfied:

- [x] Monetary denomination locked ($1\text{ SCY} = 100,000,000\text{ quanta}$).
- [x] Maximum supply ceiling locked ($42,000,000\text{ SCY}$).
- [x] Genesis allocations reconcile to exactly $10,500,000\text{ SCY}$ ($25\%$).
- [x] Mining allocation reconciles to exactly $31,500,000\text{ SCY}$ ($75\%$).
- [x] Emission halving discrepancy is documented as `CONSENSUS ISSUE — REQUIRES RESOLUTION`.
- [x] Consensus monetary invariants are codified and unambiguous.
- [x] Pure integer accounting (`u64` quanta) is mandated across all monetary rules.
- [x] Cross-document consistency is verified with zero contradictions.
- [x] Required future unit, invariant, and reality tests are identified.
- [x] No unbacked or hidden issuance pathways exist.

---

## 10. Definition of Done (DoD) & Status Lifecycle

A task must maintain one of the following official lifecycle statuses:

```text
[ PLANNED ]     ──> Task runbook drafted and approved.
     │
     ▼
[ IN PROGRESS ] ──> Active work underway on work items.
     │
     ├── If consensus-critical blocker discovered ──> [ BLOCKED ]
     │                                                    │
     │ <──────────────────────────────────────────────────┘ (Blocker resolved)
     ▼
[ VERIFIED ]    ──> All work items completed and quality gates passed.
     │
     ▼
[ COMPLETE ]    ──> 100% acceptance criteria satisfied and signed off.
```

- **Current Status:** **`VERIFIED`**

---

## 11. Dependency for Downstream Tasks

- **Task 02 (Genesis Allocation):** Requires the denomination, maximum supply cap, and genesis accounting quota ($10.5\text{M}$ SCY) defined in Task 01.
- Task 01 must be completed or its macro boundaries stabilized before Task 02 can proceed to implementation.

---

## 12. Agent Operating Instructions

An agent executing Task 01 must adhere to these operating rules:
1. Treat this document (`docs/work/01-monetary-policy.md`) as the primary work contract.
2. Read all referenced specifications before proposing modifications.
3. Do not rely on ephemeral chat history; repository documentation is the sole ground truth.
4. Never expand the scope beyond defined work items (W1–W6).
5. Report and document conflicts rather than guessing or making silent fixes.
6. Update the task status strictly based on verified evidence.
7. Stop work and report when acceptance criteria are met.

---

## 13. Cross-Specification References

- **[`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)**: Baseline monetary policy and halving formulas.
- **[`docs/ECONOMIC-MODEL.md`](../ECONOMIC-MODEL.md)**: Tokenomics and fee market dynamics.
- **[`docs/GENESIS-ALLOCATION.md`](../GENESIS-ALLOCATION.md)**: 15/5/5/75 supply distribution breakdown.
- **[`docs/GENESIS-SPEC.md`](../GENESIS-SPEC.md)**: Genesis block specification.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Master consensus validation invariants.
- **[`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)**: Protocol parameter registry.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: QA framework and quality gates.
