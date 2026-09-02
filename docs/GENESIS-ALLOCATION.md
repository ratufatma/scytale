# Scytale Genesis Allocation Specification

This document defines the architectural specification, transparency framework, and supply reconciliation rules for the **Genesis Allocation** in Scytale.

---

## 1. Purpose & Core Principles

The Genesis Allocation framework ensures that the initial distribution of SCY is fully transparent, mathematically reconcilable, and publicly auditable from the genesis block onward:

- **Mathematical Reconciliation:** Every allocated SCY must be explicitly accounted for within the fixed 42,000,000 SCY maximum supply ceiling.
- **Zero Hidden Allocations:** There are no private, undocumented, or unindexed token pools.
- **On-Chain Auditability:** All genesis issuances materialize directly as visible outputs on the active ledger.
- **Integrity Guarantee:** Genesis distribution does not grant ongoing minting privileges; protocol issuance post-genesis is governed strictly by the deterministic Proof-of-Work emission schedule.

> **Core Axiom:** *Every allocated SCY must be explicitly accounted for within the fixed maximum supply.*

---

## 2. Macro Supply & Accounting Precision

Scytale enforces a strict hard ceiling across all issuance channels:

```text
Maximum Supply = 42,000,000 SCY
Smallest Unit  = quanta (1 SCY = 100,000,000 quanta)

Total Max Supply in Quanta = 4,200,000,000,000,000 quanta
```

- All balance allocations, genesis outputs, and reconciliation formulas are computed strictly in **integer quanta** (`u64`).

---

## 3. Genesis Allocation vs. Mining Reserve

The total supply encompasses all potential token creation mechanisms throughout the network's lifetime:

$$\text{Maximum Supply} = \text{Total Genesis Allocation} + \text{Future Mining Emission}$$

```text
+-------------------------------------------------------------------------+
|                    Maximum Supply: 42,000,000 SCY                       |
|                     (4,200,000,000,000,000 quanta)                      |
+------------------------------------+------------------------------------+
|      Total Genesis Allocation      |       Mining Emission Reserve      |
|    - Initial protocol allocation   |    - Proof-of-Work block subsidies |
|    - Minted at Block 0 (Genesis)   |    - Halving schedule over epochs  |
+------------------------------------+------------------------------------+
```

### Invariants:
1. **Supply Deduction:** Any SCY allocated at genesis directly reduces the remaining pool available for Proof-of-Work block subsidies:
   $$\text{Mining Reserve} = 42,000,000\text{ SCY} - \text{Total Genesis Allocation}$$
2. **Ceiling Invariant:**
   $$\text{Total Genesis Allocation} + \sum_{h=1}^{\infty} R(h) \le 42,000,000\text{ SCY}$$

---

## 4. Transparent Allocation Categories

The genesis distribution framework is organized into four distinct structural categories:

```text
Genesis Allocation
├── Founder Allocation             : TBD
├── Development / Treasury         : TBD
├── Ecosystem Growth               : TBD
└── Community / Initial Distribution: TBD
```

| Allocation Category | Strategic Purpose | Allocation Status |
| :--- | :--- | :--- |
| **Founder Allocation** | Alignment and compensation for core architecture and development contributors. | `TBD` |
| **Development / Treasury** | Long-term protocol maintenance, infrastructure, security audits, and operations. | `TBD` |
| **Ecosystem Growth** | Grants, tooling development, developer ecosystem incentives, and integrations. | `TBD` |
| **Community Distribution** | Broadening initial ownership, fostering organic participation, and decentralized dispersion. | `TBD` |

---

## 5. Category Details & Governance Frameworks

### 5.1 Founder Allocation
- **Principles:** Must be fully declared prior to network launch, verifiably anchored to genesis UTXOs, and bounded within the 42M SCY cap.
- **Specification Status:**
  - `Founder Allocation Amount: TBD`
  - `Founder Allocation Percentage: TBD`
  - `Founder Recipient / Address: TBD`
  - `Founder Vesting Schedule: TBD`

### 5.2 Development / Treasury
- **Principles:** Dedicated to sustaining core engineering, ongoing code maintenance, and long-term network infrastructure.
- **Specification Status:**
  - `Treasury Allocation Amount: TBD`
  - `Treasury Control Model: TBD` (e.g., multi-signature or protocol-governed timelocks).
  - `Treasury Release Schedule: TBD`

### 5.3 Ecosystem Growth
- **Principles:** Supports external developers, SDK builders, open-source contributors, and strategic network integrations.
- **Specification Status:**
  - `Ecosystem Allocation Amount: TBD`
  - `Ecosystem Release Schedule: TBD`

### 5.4 Community / Initial Distribution
- **Principles:** Aims to reduce token concentration and encourage early network testing and decentralized node operation.
- **Specification Status:**
  - `Community Allocation Amount: TBD`
  - `Community Distribution Mechanism: TBD`

---

## 6. Mathematical Supply Reconciliation

Scytale mandates exact integer reconciliation across all supply components:

$$\text{Founder} + \text{Treasury} + \text{Ecosystem} + \text{Community} + \text{Mining Reserve} = 42,000,000\text{ SCY}$$

$$\text{Founder}_{\text{quanta}} + \text{Treasury}_{\text{quanta}} + \text{Ecosystem}_{\text{quanta}} + \text{Community}_{\text{quanta}} + \text{Mining Reserve}_{\text{quanta}} = 4,200,000,000,000,000\text{ quanta}$$

### Audit Invariant:
- Every quantum created at genesis must map directly to a valid `TxOut` in the genesis block's transaction payload.
- No off-ledger, unindexed, or synthetic balances are permissible under any circumstances.

---

## 7. Value Provenance for Genesis UTXOs

Genesis allocations inherit Scytale's strict **Value Provenance** consensus invariant:

```text
Genesis Block (Height 0)
          ↓
Genesis Transaction
          ↓
Genesis TxID (Blake3 Digest)
          ↓
Genesis OutPoints (TxID : Index)
          ↓
Genesis UTXOs (In Active UTXO Set)
          ↓
Subsequent Valid Transactions (When Spent)
```

- Every genesis allocation is fully traceable on-chain.
- Subsequent movements of genesis funds produce clear, deterministic DAG ancestry paths on the public ledger.

---

## 8. Vesting & Release Schedules

To align long-term incentives and mitigate market impact:
- Large structural allocations (such as Founder and Treasury pools) may incorporate deterministic vesting schedules.
- **Deterministic Rules:** If vesting is implemented, unlocking criteria must be verifiable and deterministic.
- **Status:**
  - `Founder Vesting: TBD`
  - `Treasury Release: TBD`
  - `Ecosystem Release: TBD`
  - `Community Release: TBD`

---

## 9. Public Verifiability & Macro Supply State

Any node on the network can deterministically calculate the live macro state of the currency at any block height $h$:

$$\text{Unissued Supply}(h) = 4,200,000,000,000,000\text{ quanta} - \text{Genesis Allocation}_{\text{quanta}} - \sum_{i=1}^{h} R_{\text{quanta}}(i)$$

$$\text{Current Issued Supply}(h) = \text{Genesis Allocation}_{\text{quanta}} + \sum_{i=1}^{h} R_{\text{quanta}}(i)$$

```text
+-------------------------------------------------------------------------+
|                       Maximum Supply (42,000,000 SCY)                   |
+------------------------------------+------------------------------------+
|       Current Issued Supply        |          Unissued Supply           |
|    - Genesis UTXOs                 |    - Reserved for future Proof-    |
|    - Confirmed mined block rewards |      of-Work block subsidies       |
+------------------------------------+------------------------------------+
```

---

## 10. No Arbitrary Minting Guarantee

- Genesis allocation represents an explicit, one-time protocol bootstrap issuance.
- It does **not** create ongoing minting authorities, administrative backdoors, or discretionary inflation keys.
- Following block 0, the only valid mechanism for coin generation is the deterministic Proof-of-Work coinbase subsidy, which asymptotically terminates once the 42M SCY cap is reached.

---

## 11. Open Questions & Pending Parameters

The following parameters are designated as **TBD** pending final distribution decisions:

| Parameter | Status | Scope |
| :--- | :--- | :--- |
| **Founder Allocation Amount / %** | `TBD` | Quantity and percentage of genesis allocation reserved for founders. |
| **Founder Recipient & Vesting** | `TBD` | Target public keys and timelock/vesting schedule. |
| **Treasury Allocation Amount / %** | `TBD` | Quantity and percentage reserved for protocol development treasury. |
| **Treasury Control & Release** | `TBD` | Governance model and disbursement conditions. |
| **Ecosystem Allocation Amount / %** | `TBD` | Quantity and percentage dedicated to ecosystem growth. |
| **Community Allocation Amount / %** | `TBD` | Quantity and percentage allocated for community distribution. |
| **Community Distribution Method** | `TBD` | Method of initial community dispersion. |
| **Genesis Transaction Layout** | `TBD` | Exact binary payload and output array format for Block 0. |

---

## 12. Cross-Specification References

- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: 42,000,000 SCY maximum cap and quanta denomination standards.
- **[`docs/ECONOMIC-MODEL.md`](ECONOMIC-MODEL.md)**: Macroeconomic dynamics, miner incentives, and fee markets.
- **[`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md)**: Core UTXO ledger architecture and value conservation.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint lifecycle and Value Provenance.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Genesis block specification and block state transitions.
- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Canonical transaction format and validity rules.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: User-facing asset presentation and journal history.
