# Scytale Genesis Allocation Specification

This document defines the architectural specification, transparency framework, distribution breakdown, and supply reconciliation rules for the **Genesis Allocation** of **Scytale Coin** (`SCY`) in Scytale.

---

## 1. Distribution of Fixed Maximum Supply

Scytale enforces an immutable supply ceiling of **42,000,000 SCY** ($4,200,000,000,000,000\text{ quanta}$). The total supply of Scytale Coin is strictly partitioned across four locked categories:

### 1. Locked Distribution Breakdown

```text
Maximum Supply Ceiling = 42,000,000 SCY (4,200,000,000,000,000 quanta)

├── Genesis Allocation (31% / 13,020,000 SCY)
│   ├── Founder Allocation          : 21% ( 8,820,000 SCY / 882,000,000,000,000 quanta)
│   ├── Development / Treasury      :  5% ( 2,100,000 SCY / 210,000,000,000,000 quanta)
│   └── Ecosystem / Community       :  5% ( 2,100,000 SCY / 210,000,000,000,000 quanta)
│
└── Mining Emission Reserve         : 69% (28,980,000 SCY / 2,898,000,000,000,000 quanta)
```

### Macro Allocation Table:

| Category | Supply Share (%) | Amount (SCY) | Amount (Integer Quanta) | Distribution Method |
| :--- | :---: | :---: | :---: | :--- |
| **Founder Allocation** | `21%` | `8,820,000 SCY` | `882,000,000,000,000 quanta` | One-time Genesis Block Allocation |
| **Development / Treasury** | `5%` | `2,100,000 SCY` | `210,000,000,000,000 quanta` | One-time Genesis Block Allocation |
| **Ecosystem / Community** | `5%` | `2,100,000 SCY` | `210,000,000,000,000 quanta` | One-time Genesis Block Allocation |
| **Mining Emission Reserve** | `69%` | `28,980,000 SCY` | `2,898,000,000,000,000 quanta` | Proof-of-Work Block Subsidies |
| **Total Maximum Supply** | **`100%`** | **`42,000,000 SCY`** | **`4,200,000,000,000,000 quanta`** | Strict Consensus Ceiling |

---

## 2. Allocation Philosophy & Design Rationale

The distribution model is governed by eight foundational architectural principles:

1. **Founder Contribution Recognition:** The 21% founder allocation provides meaningful, upfront alignment for the core architectural engineering and ongoing research without requiring ongoing protocol extraction.
2. **Strict One-Time Genesis Event:** The founder allocation is minted exclusively at Block 0. There are zero ongoing founder cuts from mined blocks and zero developer tax fees.
3. **Restrained Treasury Allocation:** The 5% treasury pool is deliberately constrained to prevent excessive capital concentration under centralized or internal operational control.
4. **Measured Ecosystem Reserve:** The 5% ecosystem/community allocation provides targeted support for developer tooling, infrastructure grants, and early participation without diluting mining incentives.
5. **Mining-Centric Distribution:** The vast majority of total supply (**69%**) is reserved exclusively for Proof-of-Work miners who commit computational energy to secure the network.
6. **Strict Supply Boundary:** Every single quantum across all categories is strictly bounded within the 42,000,000 SCY cap.
7. **Zero Hidden Allocations:** No off-ledger, unindexed, synthetic, or private pools exist.
8. **No Future Mint Authority:** No protocol role, founder key, or administrative multisig possesses discretionary minting authority post-genesis.

---

## 3. Category Specifications & Genesis OutPoint Mapping

Block 0 materializes the entire Genesis Allocation in a single canonical **Genesis Bootstrap Transaction** with exactly three outputs:

### 3.1 Founder Allocation (21% / 8,820,000 SCY)
- **Genesis OutPoint:** `OutPoint(GenesisTxID, 0)`
- **Quota:** `882,000,000,000,000 quanta` ($8,820,000\text{ SCY}$)
- **Occurrence:** One-time issuance executed at Block 0.
- **Address:** `scy1nw7vhxmxyz2jlw89vz88tdv938692xk968uxn89787fa4w207s8sddvv3q`
- **Locking Script:** `73a0209bbccb9b6620952fb8e5608e75b58589f4551ac5d1f8699cbe3f93dab94ff40f88ac`

### 3.2 Development / Treasury (5% / 2,100,000 SCY)
- **Genesis OutPoint:** `OutPoint(GenesisTxID, 1)`
- **Quota:** `210,000,000,000,000 quanta` ($2,100,000\text{ SCY}$)
- **Purpose:** Protocol engineering sustainability, infrastructure maintenance, security audits, and core tool development.
- **Address:** `scy1q5nhm4ge3m2myr65x5s8jdfesy5xtm0k0ddkm2qua36rw62z06zswrq8e0`
- **Locking Script:** `73a02005277dd5198ed5b20f543520793539812865edf67b5b6da81cec743769427e8588ac`

### 3.3 Ecosystem / Community (5% / 2,100,000 SCY)
- **Genesis OutPoint:** `OutPoint(GenesisTxID, 2)`
- **Quota:** `210,000,000,000,000 quanta` ($2,100,000\text{ SCY}$)
- **Purpose:** Developer grants, third-party tooling, community education, and ecosystem integrations.
- **Address:** `scy1nrlpqplz9f8dvauz2zmmgqcjxr7xvpfc95lewxft5anvgev57kmsxce3kd`
- **Locking Script:** `73a02098fe1007e22a4ed6778250b7b4031230fc6605382d3f97192ba766c46594f5b788ac`

### 3.4 Mining Emission Reserve (69% / 28,980,000 SCY)
- **Quota:** `2,898,000,000,000,000 quanta` ($28,980,000\text{ SCY}$)
- **Purpose:** Distributed to permissionless network miners via block subsidies terminating at height `3,696,000`.
- **Incentive Alignment:** Proof-of-Work mining forms the primary, sovereign path for currency dispersion.

---

## 4. Mathematical Supply Reconciliation

Scytale mandates exact integer reconciliation across all supply components:

$$\text{Founder} + \text{Treasury} + \text{Ecosystem} + \text{Mining Reserve} = 42,000,000\text{ SCY}$$

$$8,820,000\text{ SCY} + 2,100,000\text{ SCY} + 2,100,000\text{ SCY} + 28,980,000\text{ SCY} = 42,000,000\text{ SCY}$$

### Integer Quanta Accounting:
$$882,000,000,000,000 + 210,000,000,000,000 + 210,000,000,000,000 + 2,898,000,000,000,000 = 4,200,000,000,000,000\text{ quanta}$$

```text
+-------------------------------------------------------------------------+
|                    Maximum Supply: 42,000,000 SCY                       |
|                     (4,200,000,000,000,000 quanta)                      |
+------------------------------------+------------------------------------+
|   Total Genesis Allocation (31%)   |    Mining Emission Reserve (69%)   |
|        13,020,000 SCY              |           28,980,000 SCY           |
| (1,302,000,000,000,000 quanta)     |   (2,898,000,000,000,000 quanta)   |
|                                    |                                    |
| - Founder:    21% (8,820,000 SCY)  | - Minted over time via Proof-      |
| - Treasury:    5% (2,100,000 SCY)  |   of-Work block rewards            |
| - Ecosystem:   5% (2,100,000 SCY)  |   until height 3,696,000           |
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
