# Scytale Genesis Specification & Zero-Balance Onboarding

This document defines the formal specification for the **Genesis Block (Block 0)**, network bootstrap initialization, zero-balance user onboarding for **Scytale Coin** (`SCY`), and the permissionless transition from zero funds to active mining participation.

---

## 1. Initial User Balance & Onboarding Axiom

Scytale enforces a strict, self-sovereign onboarding model:

$$\text{New User Initial Balance} = 0\text{ SCY} \quad (0\text{ quanta})$$

```text
       New User / Fresh Node Installation
                       ↓
         Initialize Passbook / Keystore
                       ↓
           Initial Balance: 0 SCY
        (No pre-loaded funds or credit)
```

### Core Onboarding Axioms:
1. **No Synthetic Value:** Creating a node, wallet, or Passbook account **never creates monetary value**.
2. **No Protocol Faucets:** The mainnet protocol includes no automated distribution or faucet mechanisms for newly initialized accounts.
3. **Ledger-Anchored Ownership:** Monetary value exists exclusively as verifiable Unspent Transaction Outputs (UTXOs) recorded on the canonical blockchain.

---

## 2. Zero-Balance Participation & Permissionless Mining

A zero balance does not restrict a user's ability to participate fully in the Scytale network. Operating a node and mining blocks does not require prior ownership of SCY:

```text
                New User (Balance: 0 SCY)
                           ↓
                   Launch Scytale Node
                           ↓
               Synchronize Ledger with P2P
                           ↓
               Enable Automatic Mining
                           ↓
          Compute Valid Proof-of-Work Solution
                           ↓
          Generate Block with Valid Coinbase Tx
                           ↓
            Consensus Validates & Connects Block
                           ↓
        Receive Block Subsidy & Fees into UTXO Set
                           ↓
         Passbook Reflects First Mined SCY (> 0)
```

> **Fundamental Principle:** *Access to mining is not conditioned on prior ownership of SCY.*

---

## 3. Separation of Roles: User vs. Miner

Scytale strictly decouples the human-facing user role from the protocol-level miner role:

| Role | Definition | Initial Balance Requirement |
| :--- | :--- | :--- |
| **User** | An entity utilizing Scytale Passbook to monitor balances, inspect Value Provenance, and execute payments. | Starts at **`0 SCY`**. Requires confirmed UTXOs to spend. |
| **Miner** | A node process executing Proof-of-Work computations to secure the network and assemble candidate blocks. | **`0 SCY`**. Can initiate mining immediately upon node launch. |

- No staking collateral, validator registration, or coin locking is required to propose blocks.
- Any user starting with 0 SCY can immediately bootstrap their own balance through computational mining.

---

## 4. Genesis Allocation vs. User Bootstrapping

Scytale maintains a clear distinction between protocol-level genesis allocations and individual user onboarding:

$$\text{Genesis Allocation} \ne \text{Automatic User Balance}$$

```text
+-------------------------------------------------------------------------+
|                    Maximum Supply: 42,000,000 SCY                       |
|                     (4,200,000,000,000,000 quanta)                      |
+------------------------------------+------------------------------------+
|   Total Genesis Allocation (25%)   |    Mining Emission Reserve (75%)   |
|        10,500,000 SCY              |           31,500,000 SCY           |
| (1,050,000,000,000,000 quanta)     |   (3,150,000,000,000,000 quanta)   |
|                                    |                                    |
| - Founder:    15% (6,300,000 SCY)  | - Minted over time via Proof-      |
| - Treasury:    5% (2,100,000 SCY)  |   of-Work block rewards            |
| - Ecosystem:   5% (2,100,000 SCY)  | - Available to all miners          |
+------------------------------------+------------------------------------+
```

### Invariants:
1. **Public Accounting:** All genesis allocations are declared on-chain at Block 0 and bounded within the fixed 42,000,000 SCY cap.
2. **No User Airdrops by Default:** Genesis allocations dedicated to founders, treasury, or ecosystem growth do not grant automatic starting balances to arbitrary new nodes.
3. **Path to Initial Funds:** Users acquire their initial SCY either by:
   - Receiving an on-chain transfer from an existing holder.
   - Successfully mining a valid Proof-of-Work block.

---

## 5. Automatic Continuous Mining Lifecycle

In Scytale, mining is designed as an autonomous, continuous engine lifecycle rather than a manual, per-block command:

```text
                   Scytale Node Initialization
                               ↓
                   P2P Network Handshake & Sync
                               ↓
               Start Autonomous Mining Thread Loop
                               ↓
         [ Construct Candidate Block from Mempool + Coinbase ]
                               ↓
         [ Iterate Nonce Space against Difficulty Target ]
                               ↓
           Solution Found?
             ├── NO  ──> Update Candidate Block & Continue Search
             └── YES ──> Assemble Block & Propagate to Peers
                               ↓
                 Block Accepted by Network Consensus
                               ↓
             Initiate Next Iteration of Mining Loop
```

---

## 6. Value Provenance for Initial Funds

All funds acquired by users—whether through genesis distribution or mining—possess an unbroken, verifiable **Value Provenance** chain:

```text
Mined Coin Provenance:
Valid Proof-of-Work Block
          ↓
Coinbase Transaction
          ↓
       TxID
          ↓
  OutPoint (TxID:0)
          ↓
     Active UTXO
          ↓
  Passbook Balance Updated
```

```text
Genesis Allocation Provenance:
      Genesis Block (Height 0)
                 ↓
      Genesis Transaction
                 ↓
            Genesis TxID
                 ↓
       Genesis OutPoints
                 ↓
          Genesis UTXOs
```

- Value never enters a user's balance without a corresponding state transition event recorded on the canonical ledger.

---

## 7. Development vs. Production Environment Separation

To facilitate local testing without compromising mainnet economic integrity:

- **Mainnet Environment:** Initial user balance is strictly **0 SCY**. All coin creation is bound to genesis allocations and Proof-of-Work consensus.
- **Development / Test Environment:** Isolated test suites may employ standalone testing genesis blocks or local test harnesses. These environments:
  - Do not interact with the mainnet ledger.
  - Do not inflate or affect mainnet monetary supply.
  - Do not introduce backdoor minting mechanisms into production consensus.

---

## 8. Open Questions & Pending Parameters

The following implementation parameters remain designated as **TBD**:

| Parameter | Status | Scope |
| :--- | :--- | :--- |
| **Founder Vesting Schedule** | `TBD` | Lockup duration, cliff periods, and linear release rules for founder UTXOs. |
| **Founder Recipient Addresses** | `TBD` | Public keys / locking conditions for founder allocation outputs. |
| **Treasury & Ecosystem Control** | `TBD` | Multi-signature schema and release policies for treasury/ecosystem pools. |
| **Genesis Output Layout & Addresses** | `TBD` | Binary transaction structure and locking conditions for Block 0. |
| **Genesis Block Parameters** | `TBD` | Exact timestamp, difficulty target, and nonce for Block 0. |
| **Coinbase Maturity Threshold** | `TBD` | Confirmation depth required before mined coinbase UTXOs become spendable. |
| **Automatic Mining Policy Defaults** | `TBD` | Default thread count and CPU core allocation for node miner startup. |

---

## 9. Cross-Specification References

- **[`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md)**: Breakdown of allocation categories and supply reconciliation.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: 42,000,000 SCY cap, 60-second block target, and quanta accounting.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work rules and BLAKE3 target evaluation.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block header structure and coinbase positioning.
- **[`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md)**: Core UTXO state transitions and Value Provenance.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint indexing and lifecycle rules.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: User-facing balance derivation and journal presentation.
