# Task 02 — Genesis Allocation

This document is the permanent **Task Execution Runbook** for Task 02: Genesis Allocation. It instructs agents and engineers on how to structure, audit, test, and verify the transparent initial **Scytale Coin** (`SCY`) distribution for Scytale.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 02
Task Name   : Genesis Allocation
Phase       : Economy
Level       : LIGHT → MEDIUM
Status      : PLANNED
Dependency  : Task 01 — Monetary Policy
```

### Dependency Context:
Task 02 builds directly upon the monetary foundation established in Task 01:
- Maximum Supply Ceiling ($42,000,000\text{ SCY}$)
- Denomination Conversion ($1\text{ SCY} = 100,000,000\text{ quanta}$)
- Macro Accounting Boundary ($25\%\text{ Genesis} + 75\%\text{ Mining}$)
- Consensus Monetary Invariants

### Primary References:
- [`docs/work/01-monetary-policy.md`](01-monetary-policy.md)
- [`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)
- [`docs/GENESIS-ALLOCATION.md`](../GENESIS-ALLOCATION.md)
- [`docs/GENESIS-SPEC.md`](../GENESIS-SPEC.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)

---

## 2. Objective

> **Task Goal:** *Define and establish Scytale's Genesis Allocation as a fully transparent, explicit, one-time initial distribution strictly bound to the 42,000,000 SCY maximum supply, with mathematically verifiable on-chain Value Provenance.*

### Success Invariants:
- **Zero Hidden Allocations:** Every single quantum allocated at Block 0 must be publicly declared and accounted for.
- **Zero Arbitrary Minting:** The genesis issuance creates zero future mint authority for any entity.
- **Immutable Supply Invariant:** $\text{Genesis Allocations} + \text{Mining Emission} \le 42,000,000\text{ SCY}$.
- **Complete Reconcilability:** Every bucket mathematically reconciles in integer quanta without remainder or loss.

---

## 3. Locked Allocation Baseline

The following distribution model has been formally locked by protocol consensus and must **NOT** be modified:

```text
Maximum Supply Ceiling = 42,000,000 SCY (4,200,000,000,000,000 quanta)

1. Genesis Allocation (25% / 10,500,000 SCY / 1,050,000,000,000,000 quanta)
   ├── Founder Allocation           : 15% ( 6,300,000 SCY / 630,000,000,000,000 quanta)
   ├── Development / Treasury       :  5% ( 2,100,000 SCY / 210,000,000,000,000 quanta)
   └── Ecosystem / Community        :  5% ( 2,100,000 SCY / 210,000,000,000,000 quanta)

2. Mining Emission Reserve (75% / 31,500,000 SCY / 3,150,000,000,000,000 quanta)

Total Supply: 100% / 42,000,000 SCY / 4,200,000,000,000,000 quanta

- Founder Allocation Occurrence     : One-time Genesis issuance at Block 0
- Additional Founder Mint Authority : NONE
- New User Initial Balance          : 0 SCY (Zero-balance onboarding)
```

---

## 4. Supply Reconciliation Equations

The executing agent must maintain exact integer reconciliation across all categories:

$$\text{Founder} + \text{Treasury} + \text{Ecosystem} + \text{Mining} = \text{Maximum Supply}$$

### Denomination in SCY:
$$6,300,000\text{ SCY} + 2,100,000\text{ SCY} + 2,100,000\text{ SCY} + 31,500,000\text{ SCY} = \mathbf{42,000,000\text{ SCY}}$$

### Denomination in Integer Quanta (`u64`):
$$630\text{T} + 210\text{T} + 210\text{T} + 3,150\text{T} = \mathbf{4,200,000,000,000,000\text{ quanta}}$$

---

## 5. Allocation Category Specifications

### 5.1 Founder Allocation (`15%` / `6,300,000 SCY`)
- **Nature:** One-time initial issuance executed exclusively at Block 0.
- **Authority Boundary:** Grants zero special privileges, zero recurring cuts from mining rewards, and zero future minting rights.
- **On-Chain Audit:** Moves through standard ledger UTXO transactions subject to standard consensus rules.
- **Pending Parameters (`TBD`):**
  - `Founder Recipient Public Keys / Addresses: TBD`
  - `Founder Vesting Schedule & Cliff Release Rules: TBD`
  - `Founder UTXO Binary Layout: TBD`

### 5.2 Development & Treasury Allocation (`5%` / `2,100,000 SCY`)
- **Purpose:** Protocol engineering sustainability, infrastructure maintenance, security audits, and core tool development.
- **Authority Boundary:** Strictly finite; this is **NOT** an elastic or unlimited treasury mint.
- **Pending Parameters (`TBD`):**
  - `Treasury Multi-Signature Control Model: TBD`
  - `Treasury Release Policies & Governance: TBD`

### 5.3 Ecosystem & Community Allocation (`5%` / `2,100,000 SCY`)
- **Purpose:** Developer grants, third-party tooling, community education, and ecosystem integrations.
- **Authority Boundary:** Kept modest ($5\%$) to prevent diluting Proof-of-Work mining security incentives.
- **Pending Parameters (`TBD`):**
  - `Community Grant Distribution Mechanics: TBD`
  - `Ecosystem Tranche Disbursement Policies: TBD`

### 5.4 Proof-of-Work Mining Reserve (`75%` / `31,500,000 SCY`)
- **Purpose:** Long-term decentralized emission distributed exclusively via Proof-of-Work block subsidies.
- **Critical Warning:** The agent must **NOT** alter the emission schedule independently. If the mathematical halving schedule remains unreconciled with the $31.5\text{M}$ mining cap, the task status must be marked as `BLOCKED — CONSENSUS ISSUE` and reported.

---

## 6. Genesis Allocation vs. Zero-Balance User Onboarding

Scytale strictly separates the Genesis block distribution from general user onboarding:

$$\text{Genesis Allocation} \ne \text{Automatic User Balance}$$

```text
[ User Downloads & Launches Scytale ] ──> Initial Passbook Balance = 0 SCY
                                                   │
                                                   ▼
                    [ User Activates Permissionless Node Miner ]
                                                   │
                                                   ▼
                      [ Mined Block Commits Coinbase Output ]
                                                   │
                                                   ▼
                    [ Passbook Reflects First Positive Balance ]
```

- Genesis allocations do not seed arbitrary balances into user wallets upon installation.
- Proof-of-Work mining requires **zero initial deposit** and **zero prior token ownership**.

---

## 7. Value Provenance & On-Chain Lineage

All genesis allocations materialize as canonical ledger transactions in Block 0:

```text
Genesis Block (Height 0)
          ↓
Genesis Bootstrap Transaction
          ↓
     Genesis TxID
          ↓
Genesis OutPoints (TxID : Index)
          ├── OutPoint 0: Founder Allocation       (630,000,000,000,000 quanta)
          ├── OutPoint 1: Treasury Allocation      (210,000,000,000,000 quanta)
          └── OutPoint 2: Ecosystem Allocation     (210,000,000,000,000 quanta)
```

- Cross-References: [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md) and [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md).

---

## 8. Implementation Scope & Non-Goals

### In Scope (for future implementation):
- Structuring Block 0 transaction outputs representing the 25% Genesis Allocation.
- Mathematical verification that total genesis outputs equal exactly $1,050,000,000,000,000\text{ quanta}$.
- Enforcing that Genesis outputs are immutable and provable through `redb` storage.
- Unit and integration tests validating supply arithmetic and allocation boundaries.

### Out of Scope / Non-Goals:
- Implementing Wallet UI or Passbook presentation components.
- Selecting concrete founder cryptographic keys or public addresses.
- Creating governance voting protocols or exchange integration layers.
- Altering the $42\text{M}$ maximum supply ceiling or $15/5/5/75$ distribution ratios.

---

## 9. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 02 must fulfill the following verification suites:

### Unit Tests:
- `test_allocation_percentages_sum_to_100`: Asserting $15 + 5 + 5 + 75 == 100$.
- `test_allocation_scy_amounts_sum_to_42m`: Asserting $6.3\text{M} + 2.1\text{M} + 2.1\text{M} + 31.5\text{M} == 42\text{M}$.
- `test_quanta_reconciliation_exactness`: Asserting $630\text{T} + 210\text{T} + 210\text{T} + 3,150\text{T} == 4,200\text{T}$.
- `test_zero_user_balance_invariant`: Asserting fresh keypairs instantiate with 0 spendable UTXOs.

### Consensus & Integration Invariant Tests:
- `test_genesis_output_value_exactness`: Asserting Block 0 outputs match the exact $10.5\text{M}$ SCY quota.
- `test_no_future_founder_mint`: Proving consensus rejects any block attempting to mint non-PoW founder subsidies.
- `test_genesis_value_provenance_dag`: Proving backward traversal from genesis UTXOs resolves directly to Block 0.

### Reality Tests (Future Implementation Phase):
- Execute the real `scytale-node` startup path, load Block 0 into `redb`, and assert that `UTXO_SET` contains exactly the expected Genesis OutPoints totaling $10.5\text{M}$ SCY with zero unaccounted quanta.

---

## 10. Acceptance Criteria Checklist

Task 02 can only be marked as **VERIFIED** when:

- [ ] Founder allocation is locked at `15%` ($6,300,000\text{ SCY}$).
- [ ] Treasury allocation is locked at `5%` ($2,100,000\text{ SCY}$).
- [ ] Ecosystem / Community allocation is locked at `5%` ($2,100,000\text{ SCY}$).
- [ ] Mining reserve is locked at `75%` ($31,500,000\text{ SCY}$).
- [ ] Total allocation reconciles to exactly `42,000,000 SCY` ($4,200,000,000,000,000\text{ quanta}$).
- [ ] Founder allocation is specified as a one-time issuance at Block 0.
- [ ] Zero additional founder minting authority exists.
- [ ] New user initial balance remains strictly `0 SCY`.
- [ ] Genesis Value Provenance lineage is unambiguously specified.
- [ ] Zero hidden or off-ledger allocation pathways exist.
- [ ] Mining allocation is reconciled with the emission schedule (or explicitly marked as `BLOCKED`).

---

## 11. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and accepted.
     │
     ▼
[ IN PROGRESS ] ──> Verification and implementation underway.
     │
     ├── If emission discrepancy blocks consensus ──> [ BLOCKED ]
     │                                                     │
     │ <───────────────────────────────────────────────────┘ (Discrepancy resolved)
     ▼
[ VERIFIED ]    ──> All reconciliation formulas and unit tests pass.
     │
     ▼
[ COMPLETE ]    ──> 100% acceptance criteria satisfied against real storage.
```

- **Current Status:** **`PLANNED`**

---

## 12. Dependency for Downstream Tasks

- **Task 03 (Transaction Model):** Consumes the genesis output definitions and supply accounting invariants from Task 02.
- Task 03 cannot construct transaction verification logic without stable Genesis OutPoint boundaries.

---

## 13. Agent Operating Rules

1. Treat `docs/work/02-genesis-allocation.md` as the authoritative work runbook.
2. Cross-reference Task 01 and baseline specifications before proposing changes.
3. The repository codebase and docs are the sole ground truth.
4. Never alter locked supply ratios ($15/5/5/75$) or maximum supply ($42\text{M}$ SCY).
5. If the emission schedule mismatch affects execution, mark status as `BLOCKED`.
6. Adhere strictly to the definition of done.

---

## 14. Cross-Specification References

- **[`docs/work/01-monetary-policy.md`](01-monetary-policy.md)**: Monetary policy runbook.
- **[`docs/GENESIS-ALLOCATION.md`](../GENESIS-ALLOCATION.md)**: Macro distribution specification.
- **[`docs/GENESIS-SPEC.md`](../GENESIS-SPEC.md)**: Genesis block specification.
- **[`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)**: Monetary policy and emission curves.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)**: Lineage tracking.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Master consensus rules.
- **[`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)**: Protocol constants registry.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
