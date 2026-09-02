# Scytale Genesis Allocation Specification

This document defines the architectural specification, transparency framework, distribution breakdown, and supply reconciliation rules for the **Genesis Allocation** of **Scytale Coin** (`SCY`) in Scytale.

---

## 1. Distribution of Fixed Maximum Supply

Scytale enforces an immutable supply ceiling of **42,000,000 SCY** ($4,200,000,000,000,000\text{ quanta}$). The total supply of Scytale Coin is strictly partitioned across four locked categories:

```text
42,000,000 SCY (100%)
├── Genesis Allocation (25% / 10,500,000 SCY)
│   ├── Founder Allocation          : 15% ( 6,300,000 SCY)
│   ├── Development / Treasury      :  5% ( 2,100,000 SCY)
│   └── Ecosystem / Community       :  5% ( 2,100,000 SCY)
│
└── Mining Emission Reserve         : 75% (31,500,000 SCY)
```

### Macro Allocation Table:

| Category | Supply Share (%) | Amount (SCY) | Amount (Integer Quanta) | Distribution Method |
| :--- | :---: | :---: | :---: | :--- |
| **Founder Allocation** | `15%` | `6,300,000 SCY` | `630,000,000,000,000 quanta` | One-time Genesis Block Allocation |
| **Development / Treasury** | `5%` | `2,100,000 SCY` | `210,000,000,000,000 quanta` | One-time Genesis Block Allocation |
| **Ecosystem / Community** | `5%` | `2,100,000 SCY` | `210,000,000,000,000 quanta` | One-time Genesis Block Allocation |
| **Mining Emission Reserve** | `75%` | `31,500,000 SCY` | `3,150,000,000,000,000 quanta` | Proof-of-Work Block Subsidies |
| **Total Maximum Supply** | **`100%`** | **`42,000,000 SCY`** | **`4,200,000,000,000,000 quanta`** | Strict Consensus Ceiling |

---

## 2. Allocation Philosophy & Design Rationale

The distribution model is governed by eight foundational architectural principles:

1. **Founder Contribution Recognition:** The 15% founder allocation provides meaningful, upfront alignment for the core architectural engineering and ongoing research without requiring ongoing protocol extraction.
2. **Strict One-Time Genesis Event:** The founder allocation is minted exclusively at Block 0. There are zero ongoing founder cuts from mined blocks and zero developer tax fees.
3. **Restrained Treasury Allocation:** The 5% treasury pool is deliberately constrained to prevent excessive capital concentration under centralized or internal operational control.
4. **Measured Ecosystem Reserve:** The 5% ecosystem/community allocation provides targeted support for developer tooling, infrastructure grants, and early participation without diluting mining incentives.
5. **Mining-Centric Distribution:** The vast majority of total supply (**75%**) is reserved exclusively for Proof-of-Work miners who commit computational energy to secure the network.
6. **Strict Supply Boundary:** Every single quantum across all categories is strictly bounded within the 42,000,000 SCY cap.
7. **Zero Hidden Allocations:** No off-ledger, unindexed, synthetic, or private pools exist.
8. **No Future Mint Authority:** No protocol role, founder key, or administrative multisig possesses discretionary minting authority post-genesis.

---

## 3. Category Specifications

### 3.1 Founder Allocation (15% / 6,300,000 SCY)
- **Occurrence:** One-time issuance executed at Block 0.
- **Constraints:**
  - Carries no ongoing percentage of block subsidies or transaction fees.
  - Grants no special voting, staking, or governance privileges.
  - Fully bound to genesis ledger outputs with verifiable Value Provenance.
- **Specification Status:**
  - `Founder Recipient / Address: TBD`
  - `Founder Vesting Schedule: TBD`

### 3.2 Development / Treasury (5% / 2,100,000 SCY)
- **Purpose:** Protocol maintenance, cryptographic audits, core node infrastructure, security bug bounties, and essential operational needs.
- **Design Intent:** Kept small (5%) to prevent internal centralization of protocol capital.
- **Specification Status:**
  - `Treasury Control Model: TBD` (e.g., timelocked multi-signature governance).
  - `Treasury Release Policy: TBD`

### 3.3 Ecosystem / Community (5% / 2,100,000 SCY)
- **Purpose:** Open-source developer grants, client SDKs, documentation tooling, integrations, and initial community bootstrapping initiatives.
- **Design Intent:** Kept limited (5%) to avoid aggressive supply dilution against Proof-of-Work miners.
- **Specification Status:**
  - `Community Distribution Mechanism: TBD`
  - `Ecosystem Release Policy: TBD`

### 3.4 Mining Emission Reserve (75% / 31,500,000 SCY)
- **Purpose:** Distributed to permissionless network miners via block subsidies over successive halving epochs as specified in [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md), [`docs/POW-SPEC.md`](POW-SPEC.md), and [`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md).
- **Incentive Alignment:** Proof-of-Work mining forms the primary, sovereign path for currency dispersion.

---

## 4. Mathematical Supply Reconciliation

Scytale mandates exact integer reconciliation across all supply components:

$$\text{Founder} + \text{Treasury} + \text{Ecosystem} + \text{Mining Reserve} = 42,000,000\text{ SCY}$$

$$6,300,000\text{ SCY} + 2,100,000\text{ SCY} + 2,100,000\text{ SCY} + 31,500,000\text{ SCY} = 42,000,000\text{ SCY}$$

### Integer Quanta Accounting:
$$630,000,000,000,000 + 210,000,000,000,000 + 210,000,000,000,000 + 3,150,000,000,000,000 = 4,200,000,000,000,000\text{ quanta}$$

```text
+-------------------------------------------------------------------------+
|                    Maximum Supply: 42,000,000 SCY                       |
|                     (4,200,000,000,000,000 quanta)                      |
+------------------------------------+------------------------------------+
|   Total Genesis Allocation (25%)   |    Mining Emission Reserve (75%)   |
|        10,500,000 SCY              |           31,500,000 SCY           |
| (1,050,000,000,000,000 quanta)     |   (3,150,000,000,000,000 quanta)   |
|                                    |                                    |
| - Founder:    15% (6,300,000 SCY)  | - Minted via Proof-of-Work         |
| - Treasury:    5% (2,100,000 SCY)  |   block subsidies over halving     |
| - Ecosystem:   5% (2,100,000 SCY)  |   epochs                           |
+------------------------------------+------------------------------------+
```

---

## 5. Genesis Allocation vs. Mining Emission

Scytale strictly separates the nature and provenance of genesis coin allocations from mined coins:

$$\text{Genesis Allocation} \ne \text{Mining Emission}$$

$$\text{Genesis Allocation} + \text{Mining Emission} = \text{Maximum Supply Boundary}$$

- **Genesis Allocations:** Minted as direct outputs in the Block 0 transaction to establish initial development and ecosystem foundations.
- **Mined Coins:** Minted incrementally in response to verified, unforgeable thermodynamic Proof-of-Work.

---

## 6. Public Accounting & Value Provenance

Genesis allocations are fully auditable on the public ledger:

```text
Genesis Allocation Lineage:
Genesis Block (Height 0)
          ↓
Genesis Transaction
          ↓
Genesis TxID (Blake3 Digest)
          ↓
Genesis OutPoints (TxID : Index)
          ↓
Genesis UTXOs (Active Set)
          ↓
Subsequent Valid Transactions (When Spent)
```

```text
Mining Emission Lineage:
Mined Block (Height H)
          ↓
Coinbase Transaction
          ↓
Coinbase TxID (Blake3 Digest)
          ↓
Coinbase OutPoint (TxID : 0)
          ↓
Mined UTXO (Active Set)
          ↓
Subsequent Valid Transactions (When Spent)
```

- Every single quantum held in any wallet has a deterministic provenance path that can be independently audited backward to Block 0 or a valid Proof-of-Work block.

---

## 7. Initial User Balance Invariant

The presence of a genesis allocation does **not** alter the onboarding principle for new users:

$$\text{New User Initial Balance} = 0\text{ SCY} \quad (0\text{ quanta})$$

- Fresh nodes and Passbook instances begin at `0 SCY`.
- Users obtain SCY either through valid peer-to-peer transfers or by running a node to mine blocks permissionlessly.

---

## 8. Open Questions & Pending Parameters

The following structural parameters remain designated as **TBD**:

| Parameter | Status | Scope |
| :--- | :--- | :--- |
| **Founder Vesting Schedule** | `TBD` | Lockup duration, cliff periods, and linear release rules for founder UTXOs. |
| **Founder Recipient Addresses** | `TBD` | Public keys / locking conditions for founder allocation outputs. |
| **Treasury Control Model** | `TBD` | Multi-signature schema and cryptographic threshold parameters. |
| **Treasury Release Policy** | `TBD` | Milestone-based disbursement criteria. |
| **Community Distribution Method** | `TBD` | Mechanics for dispersing community/ecosystem grants. |
| **Ecosystem Release Policy** | `TBD` | Tranche release schedules for ecosystem development. |
| **Genesis Transaction Layout** | `TBD` | Exact binary payload and output array format for Block 0. |
| **Genesis Block Parameters** | `TBD` | Exact timestamp, initial difficulty target, and nonce for Block 0. |

---

## 9. Cross-Specification References

- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Genesis block specification and zero-balance onboarding.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: 42,000,000 SCY cap, 60-second block target, and quanta accounting.
- **[`docs/ECONOMIC-MODEL.md`](ECONOMIC-MODEL.md)**: Macroeconomic dynamics, miner incentives, and fee markets.
- **[`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md)**: Core UTXO ledger architecture and value conservation.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint lifecycle and Value Provenance.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Genesis block specification and block state transitions.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work validation rules.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: 60-second target block adjustment.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: User-facing asset presentation and journal history.
