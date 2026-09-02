# Scytale Monetary Policy Framework

This document defines the architectural specification, locked parameters, and mathematical framework for the monetary policy of the Scytale blockchain engine. It establishes how asset issuance, denomination units, halving epochs, supply calculation, and consensus coinbase constraints are governed by deterministic protocol rules.

---

## 1. Purpose

The monetary policy framework is designed to:
- Establish unambiguous, deterministic rules for native asset issuance across the network.
- Ensure that every node can calculate and verify the total issued supply and current block subsidy independently without trusting third parties.
- Provide a mathematically provable supply boundary derived directly from the emission schedule.
- Enforce strict integer-based accounting in atomic units (`quanta`) without floating-point ambiguity.
- Maintain a strict boundary between newly minted supply (block rewards) and circulating supply transfers (transaction fees).

---

## 2. Asset Denomination & Units

Scytale defines a single native coin, **Scytale Coin** (`SCY`), with two standardized denomination tiers:

```text
Project / Protocol : Scytale
Native Coin        : Scytale Coin
Asset Symbol       : SCY
Smallest Unit      : quanta
Conversion         : 1 SCY = 100,000,000 quanta (10^8 quanta)
```

| Denomination | Role | Representation |
| :--- | :--- | :--- |
| **`SCY`** | Primary human-readable display & economic unit. | External interface / UI representation. |
| **`quanta`** | Smallest internal accounting & consensus unit. | Integer value stored in ledger and UTXOs (`u64`). |

### Accounting Invariant:
- **`Scytale Coin`** is the native coin of the Scytale network. `SCY` and `quanta` are **not two distinct assets**; they represent two denominations of the exact same native coin.
- **Strict Integer Accounting:** All monetary accounting across transactions, UTXOs, fees, coinbase distributions, genesis allocations, and supply conservation is performed strictly in unsigned 64-bit integer **`quanta`** (`u64`).
- **Zero Floating-Point Consensus:** Floating-point numbers are strictly forbidden in consensus, validation, and balance accounting to eliminate rounding and non-deterministic divergence.

---

## 3. Locked Monetary Parameters

The core macroeconomic parameters for Scytale are locked as follows:

| Parameter | Value in SCY | Value in Quanta | Description |
| :--- | :--- | :--- | :--- |
| **Target Maximum Supply ($S_{\text{max}}$)** | **`42,000,000 SCY`** | `4,200,000,000,000,000 quanta` | Target ceiling of total mintable currency. |
| **Initial Block Reward ($R_0$)** | **`10 SCY`** / block | `1,000,000,000 quanta` / block | Subsidy awarded per block during Epoch 0. |
| **Target Block Interval ($T_{\text{target}}$)** | **`60 seconds`** | — | Target duration between consecutive blocks. |
| **Halving Interval** | **`2,100,000 blocks`** | — | Number of blocks in each emission epoch. |
| **Reward Reduction Schedule** | **`50%`** | — | Subsidy reduces by half every 2,100,000 blocks. |
| **Genesis Allocation ($G$)** | **`10,500,000 SCY`** (`25%`) | `1,050,000,000,000,000 quanta` | One-time genesis allocation (Founder 15%, Treasury 5%, Ecosystem 5%). |
| **Mining Emission Reserve** | **`31,500,000 SCY`** (`75%`) | `3,150,000,000,000,000 quanta` | Total supply reserved for Proof-of-Work mining distribution. |

---

## 4. Emission Schedule & Halving Epochs

Block subsidies decrease deterministically across epochs according to a discrete geometric halving curve:

| Epoch | Block Height Range | Reward per Block (SCY) | Reward per Block (quanta) | Era Total Minted (SCY) | Approx. Calendar Duration |
| :---: | :---: | :---: | :---: | :---: | :---: |
| **Epoch 0** | $0 \rightarrow 2,099,999$ | `10.00 SCY` | `1,000,000,000 quanta` | `21,000,000 SCY` | ~3.995 years |
| **Epoch 1** | $2,100,000 \rightarrow 4,199,999$ | `5.00 SCY` | `500,000,000 quanta` | `10,500,000 SCY` | ~3.995 years |
| **Epoch 2** | $4,200,000 \rightarrow 6,299,999$ | `2.50 SCY` | `250,000,000 quanta` | `5,250,000 SCY` | ~3.995 years |
| **Epoch 3** | $6,300,000 \rightarrow 8,399,999$ | `1.25 SCY` | `125,000,000 quanta` | `2,625,000 SCY` | ~3.995 years |
| **Epoch 4** | $8,400,000 \rightarrow 10,499,999$ | `0.625 SCY` | `62,500,000 quanta` | `1,312,500 SCY` | ~3.995 years |
| ... | ... | ... | ... | ... | ... |
| **Epoch $N$** | $N \times 2,100,000 \rightarrow \dots$ | $10 \times 2^{-N}\text{ SCY}$ | $10^9 \times 2^{-N}\text{ quanta}$ | $2.1 \times 10^7 \times 2^{-N}\text{ SCY}$ | ~3.995 years |

### Epoch Duration Calculation
With a 60-second block interval target:
$$\text{Epoch Duration} = \frac{2,100,000 \text{ blocks} \times 60 \text{ seconds}}{86,400 \text{ seconds/day} \times 365.25 \text{ days/year}} \approx 3.995 \text{ years}$$

> [!NOTE]
> Calendar duration is an approximation based on target block intervals; actual elapsed calendar time will vary with Proof-of-Work difficulty adjustments and network hashrate fluctuations.

---

## 5. Mathematical Supply Derivation & Macro Supply Equation

The maximum supply is not an arbitrary constant; it is derived from the strict macro supply equation:

$$\text{Maximum Supply} = \text{Genesis Allocation} + \text{Issued Mining Rewards} + \text{Unissued Mining Reserve}$$

In integer **`quanta`**:

$$\mathbf{S_{\text{max}}} = G_{\text{quanta}} + M_{\text{issued,quanta}} + M_{\text{unissued,quanta}} = \mathbf{4,200,000,000,000,000\text{ quanta}}$$

### Macro Allocation Quota Proof:
$$\begin{aligned}
\text{Founder Allocation (15%)} &= 630,000,000,000,000\text{ quanta} \quad (6,300,000\text{ SCY}) \\
\text{Treasury Allocation (5%)} &= 210,000,000,000,000\text{ quanta} \quad (2,100,000\text{ SCY}) \\
\text{Ecosystem Allocation (5%)} &= 210,000,000,000,000\text{ quanta} \quad (2,100,000\text{ SCY}) \\
\text{Mining Reserve (75%)} &= 3,150,000,000,000,000\text{ quanta} \quad (31,500,000\text{ SCY}) \\
\hline
\mathbf{\text{Total Maximum Supply}} &= \mathbf{4,200,000,000,000,000\text{ quanta}} \quad (\mathbf{42,000,000\text{ SCY}})
\end{aligned}$$

---

## 6. Known Emission Schedule Discrepancy & Consensus Issue

> [!WARNING]
> ### CONSENSUS ISSUE — REQUIRES RESOLUTION
> 
> A mathematical discrepancy exists between the locked allocation percentages and the baseline halving formula:
> 
> 1. **Locked Allocation Rule:**
>    - Genesis Allocation ($25\%$) = $10,500,000\text{ SCY}$ ($1,050,000,000,000,000\text{ quanta}$)
>    - Authorized Mining Allocation ($75\%$) = $\mathbf{31,500,000\text{ SCY}}$ ($\mathbf{3,150,000,000,000,000\text{ quanta}}$)
>    - Maximum Supply Ceiling ($100\%$) = $\mathbf{42,000,000\text{ SCY}}$ ($\mathbf{4,200,000,000,000,000\text{ quanta}}$)
> 
> 2. **Baseline Halving Series Calculation:**
>    $$\text{Theoretical Mined Sum} = 10\text{ SCY} \times 2,100,000\text{ blocks} \times \sum_{k=0}^{\infty} \left(\frac{1}{2}\right)^k = 21,000,000 \times 2 = \mathbf{42,000,000\text{ SCY}}$$
> 
> 3. **The Conflict:**
>    - Adding the Genesis Allocation ($10.5\text{M}$ SCY) to the unadjusted infinite halving sum ($42\text{M}$ SCY) produces **$52,500,000\text{ SCY}$**, violating the immutable $42,000,000\text{ SCY}$ ceiling by $10,500,000\text{ SCY}$.
> 
> 4. **Technical Resolution Options for Consensus Decision:**
>    - **Option A (Subsidy Hard Cap):** Keep $R_0 = 10\text{ SCY}$ and $2,100,000\text{ blocks}$ halving; terminate/zero out mining rewards once cumulative mined supply reaches exactly $3,150,000,000,000,000\text{ quanta}$ ($31.5\text{M}$ SCY).
>    - **Option B (Reward Recalibration):** Recalibrate initial reward to $R_0 = 7.5\text{ SCY}$ ($750,000,000\text{ quanta}$) with $2,100,000\text{ blocks}$ halving ($7.5 \times 2.1\text{M} \times 2 = 31.5\text{M}$ SCY).
>    - **Option C (Halving Interval Recalibration):** Keep $R_0 = 10\text{ SCY}$ and adjust halving interval to $1,575,000\text{ blocks}$ ($10 \times 1.575\text{M} \times 2 = 31.5\text{M}$ SCY).
---

## 7. Pending Consensus Details: Rounding & Reward Termination

The target cap `42,000,000 SCY` represents the continuous geometric ceiling. The final consensus implementation must explicitly formalize the following **pending consensus details**:

1. **Integer Bitshift/Division Semantics:** Explicit specification of integer truncation when halving odd quanta amounts ($R_{\text{quanta}}(e) = R_{0,\text{quanta}} \gg e$).
2. **Terminal Epoch Index ($e_{\text{end}}$):** The exact epoch at which integer division truncates the block subsidy to $0\text{ quanta}$.
3. **Exact Cumulative Bound Proof:** Formal proof verifying that the discrete integer sum of all minted quanta $\sum R_{\text{quanta}}(h)$ remains strictly $\le 4,200,000,000,000,000\text{ quanta}$.

---

## 8. Emission Duration vs. Maximum Supply

A critical conceptual distinction:

$$\text{Emission Duration} \ne \text{Maximum Supply}$$

- Setting **42,000,000 SCY** (with 10 SCY initial reward) instead of 21,000,000 SCY (with 5 SCY initial reward) **does not double epoch duration or network lifetime**.
- Epoch duration is determined strictly by $\text{Halving Interval} \times \text{Block Interval}$ ($2,100,000 \times 60\text{s} \approx 3.995\text{ years}$).
- The parameter change only affects the **issuance velocity (quanta generated per unit of time)**, not the calendar progression of halving milestones.

---

## 9. Consensus Coinbase Constraints

Every candidate block must satisfy the consensus upper bound:

$$\text{Coinbase Output Value (quanta)} \le R_{\text{quanta}}(h) + \sum_{i=1}^{N} \text{Fee}_i(\text{quanta})$$

```text
+-------------------------------------------------------------+
|                   Total Coinbase Payout                     |
+------------------------------+------------------------------+
|       Block Subsidy R(h)     |     Sum of Transaction Fees  |
|      (New Supply Minted)     | (Transfer of Existing Supply)|
+------------------------------+------------------------------+
```

### Consensus Rules:
1. **Invalid Over-Minting:** If the sum of coinbase outputs exceeds $R_{\text{quanta}}(h) + \sum \text{Fee}_{\text{quanta}}$, the block is invalid and immediately rejected.
2. **Permissible Under-Claiming:** A miner may legally claim fewer quanta than the maximum allowable coinbase value. Any unclaimed quanta are permanently unissued, reducing the final circulating supply below $S_{\text{max}}$.
3. **Coinbase Isolation:** Coinbase transactions do not spend standard UTXO inputs and must be the first transaction in a block.

---

## 10. Transaction Fees

Transaction fees represent the difference between consumed input values and newly created output values:

$$\text{Fee} = \sum \text{Input Values} - \sum \text{Output Values} \quad (\text{in quanta})$$

- **Non-Inflationary:** Fees circulate existing quanta from users to miners; they create no new supply.
- **Economic Invariant:** $\sum \text{Input Values} \ge \sum \text{Output Values}$ for every non-coinbase transaction.

---

## 11. Protocol Supply Invariants

The Scytale ledger and consensus rules enforce the following immutable invariants:

1. **Supply Upper Bound:** $\text{Issued Supply}(h) \le 4,200,000,000,000,000\text{ quanta}$ for all heights $h$.
2. **Non-Negative Reward:** $R_{\text{quanta}}(h) \ge 0$ for all heights $h$.
3. **No Arbitrary Minting:** Coinbase cannot create supply beyond $R_{\text{quanta}}(h) + \sum \text{Fee}_{\text{quanta}}$.
4. **Conservation of Existing Supply:** Transaction fees never increase total circulating supply.
5. **Deterministic Arithmetic:** All monetary evaluations produce identical integer quanta results on all compliant nodes.

---

## 12. Market Principles & Protocol Boundary

- **Issuance vs. Valuation:**
  - The protocol strictly enforces issuance volume, emission pace, and consensus validity.
  - The protocol does **not** control, peg, or guarantee the exchange rate, purchasing power, or market price of SCY.
- **Block Space Scarcity & Utility:**
  - Block space is finite. Users compete for transaction confirmation priority by offering higher fee density in quanta per byte.
  - As block subsidies diminish over successive epochs, transaction fees organically transition into the primary economic incentive for network security.
- **Miner Policy Autonomy:**
  - Miners freely determine local transaction prioritization and mempool acceptance policies without altering consensus rules.

---

## 13. Cross-Specification References

- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Genesis block specification and zero-balance onboarding.
- **[`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md)**: Transparent genesis allocation framework.
- **[`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md)**: UTXO state transitions and Value Provenance.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block header structure and coinbase limits.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work threshold evaluation.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: 60-second difficulty adjustment.
- **[`docs/ECONOMIC-MODEL.md`](ECONOMIC-MODEL.md)**: Macroeconomic dynamics and fee market.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: User-facing asset presentation and journal history.
