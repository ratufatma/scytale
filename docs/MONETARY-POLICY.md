# Scytale Monetary Policy Framework

This document defines the architectural specification and mathematical framework for the monetary policy of the Scytale blockchain engine. It establishes how asset issuance, supply calculation, and coinbase constraints are governed by deterministic protocol rules.

---

## 1. Purpose

The monetary policy framework is designed to:
- Establish unambiguous, deterministic rules for native asset issuance across the network.
- Ensure that every node can calculate and verify the total issued supply and current block subsidy independently without trusting third parties.
- Provide a mathematically provable supply boundary derived directly from the emission schedule rather than arbitrary estimations.
- Separate new asset issuance from the transfer of existing units via transaction fees.

---

## 2. Asset Specification

The concrete naming, branding, and denomination divisions are deliberately left as open parameters at this baseline stage:

| Field | Baseline Status | Description |
| :--- | :--- | :--- |
| **Asset Name** | `TBD` | Full formal name of the native currency. |
| **Ticker** | `TBD` | Short market/protocol symbol (e.g., 3-5 uppercase characters). |
| **Smallest Unit** | `TBD` | Name and fractional power (decimals) of the atomic unit (e.g., $10^8$ atomic units per 1 coin). |

---

## 3. Monetary Parameters

The core variables governing Scytale's monetary supply are defined below. All parameters remain `TBD` until mathematically locked in subsequent design phases:

| Parameter | Baseline Status | Function & Impact on Total Issuance |
| :--- | :--- | :--- |
| **Maximum Supply ($S_{\text{max}}$)** | `TBD` | The hard asymptotic ceiling of total coins that can ever be minted. Derived strictly from the sum of genesis allocation and the integral of the emission schedule. |
| **Initial Block Reward ($R_0$)** | `TBD` | The base amount of newly minted atomic units awarded to the miner for the initial block period. Determines the starting rate of inflation. |
| **Block Interval ($T_{\text{target}}$)** | `TBD` | The target time between consecutive blocks. Determines emission velocity (coins issued per unit time) in conjunction with the block reward. |
| **Emission Schedule** | `TBD` | The deterministic function $R(h)$ governing how block reward decreases over chain height $h$ (e.g., discrete step halvings, continuous geometric decay, or epoch-based reduction). |
| **Reward Reduction Interval** | `TBD` | The number of blocks (or time epochs) between reward adjustments. Controls the pacing of supply deceleration. |
| **Genesis Issuance ($G$)** | `TBD` | Any initial pre-allocated or genesis-minted supply created at block 0. If 0, the engine follows a pure fair-launch emission from mining. |

---

## 4. Emission Model & Coinbase Payout

The creation of new currency units is strictly confined to the **Coinbase Transaction** of valid blocks.

```text
+-------------------------------------------------------------+
|                   Total Coinbase Payout                     |
+------------------------------+------------------------------+
|       Block Subsidy R(h)     |     Sum of Transaction Fees  |
|      (New Supply Minted)     | (Transfer of Existing Supply)|
+------------------------------+------------------------------+
```

### Critical Separation: Issuance vs. Transfer
1. **Block Subsidy ($R(h)$):**
   - Represents **new supply creation**.
   - Expands the total circulating supply ($S_{\text{issued}}$).
   - Decreases over time according to the protocol-defined emission schedule until it permanently reaches zero.
2. **Transaction Fees ($\sum \text{Fee}$):**
   - Represents the **transfer of existing supply** from transaction creators to the validating miner.
   - Does **not** inflate or alter the total circulating supply.
   - Sustains long-term miner incentives once the block subsidy decays to zero.

---

## 5. Mathematical Supply Calculation

The maximum supply is not an arbitrary constant; it is the exact mathematical sum of genesis issuance and all block rewards over the entire lifecycle of the blockchain:

$$S_{\text{total}} = G + \sum_{h=0}^{H_{\text{end}}} R(h)$$

Where:
- $G$ = Genesis allocation (atomic units created at height $h = 0$).
- $R(h)$ = Protocol-defined block reward at height $h$.
- $H_{\text{end}}$ = The block height at which $R(h)$ truncates to $0$ atomic units.

```text
Block Reward R(h)
       ↓
Emission Schedule (Decay Function)
       ↓
Total Cumulative Issuance
       ↓
Maximum Supply Cap (S_max)
```

> [!IMPORTANT]
> Any future selection of $S_{\text{max}}$ must be derived by evaluating $\sum R(h)$ with exact integer precision. No parameters will be selected purely on aesthetic appeal.

---

## 6. Reward Precision & Integer Arithmetic

To ensure absolute determinism across all platforms and architectures:
- **No Floating-Point Arithmetic:** All block reward calculations, fee summations, and balance verifications must use fixed-precision integer arithmetic in atomic units (`u64` or `u128`).
- **Rounding & Truncation:** Reward reduction formulas that involve division (e.g., bitshifts $R_0 \gg \text{halvings}$ or integer divisions) must specify explicit truncation semantics.
- **Terminal Emission ($R(h) \to 0$):** Because atomic units cannot be fractionally divided, the block reward will naturally reach exactly zero once the calculated reward falls below 1 atomic unit.
- **Exact Final Issuance:** The actual maximum supply will be identical to the discrete integer sum of all minted units.

---

## 7. Consensus Coinbase Constraint

The consensus engine enforces a strict upper bound on every candidate block:

$$\text{Coinbase Output Value} \le R(h) + \sum_{i=1}^{N} \text{Fee}_i$$

### Enforcement Rules:
1. **Over-minting Invalidation:** If a miner constructs a block where the total coinbase outputs exceed $R(h) + \sum \text{Fee}$, the block is mathematically invalid and will be instantly rejected by all validating nodes.
2. **Under-claiming (Permissible):** A miner may legally choose to claim less than the maximum allowable coinbase value. Any unclaimed reward is permanently unissued, effectively reducing the final circulating supply below $S_{\text{max}}$.

---

## 8. Transaction Fees

Transaction fees are determined strictly by the ledger state transition:

$$\text{Fee} = \sum \text{Input Values} - \sum \text{Output Values}$$

- **Invariants:**
  - $\sum \text{Input Values} \ge \sum \text{Output Values}$ for all non-coinbase transactions.
  - Fees are non-inflationary: they circulate existing tokens and do not alter the macro supply.

---

## 9. Supply State Taxonomy

Nodes can deterministically compute the current macro state of the currency at any block height $h$:

```text
+-------------------------------------------------------------+
|                    Maximum Supply (S_max)                   |
+------------------------------+------------------------------+
|     Current Issued Supply    |       Unissued Supply        |
|    (Circulating in UTXOs)    |    (To be mined in future)   |
+------------------------------+------------------------------+
```

1. **Maximum Supply ($S_{\text{max}}$):** The theoretical upper bound of total coins that can ever exist based on the emission formula.
2. **Current Issued Supply ($S_{\text{issued}}(h)$):** The cumulative sum of all block subsidies actually minted from genesis up to block height $h$, verifiable by querying historical block headers or the active UTXO state.
3. **Unissued Supply ($S_{\text{unissued}}(h)$):** The remaining supply reserved to be minted by future block rewards ($S_{\text{max}} - S_{\text{issued}}(h)$).

---

## 10. Market Principles & Protocol Boundary

- **Issuance vs. Valuation:**
  - The protocol dictates issuance volume, emission pace, and consensus validity.
  - The protocol does **not** control, peg, or guarantee the exchange rate, purchasing power, or market price of the native asset.
- **Block Space Scarcity & Utility:**
  - Block space is finite. Users compete for priority ledger inclusion by offering higher fee densities.
  - Network demand for transaction settlement drives transaction fee revenue for miners, establishing an organic market equilibrium.
- **Policy Autonomy:**
  - Miners are free to accept, reject, or prioritize transactions according to their own economic preferences, provided the mined block strictly satisfies network consensus constraints.
