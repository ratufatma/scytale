# Scytale Monetary Policy Framework

This document defines the architectural specification, locked parameters, and mathematical framework for the monetary policy of the Scytale blockchain engine. It establishes how asset issuance, halving epochs, supply calculation, and consensus coinbase constraints are governed by deterministic protocol rules.

---

## 1. Purpose

The monetary policy framework is designed to:
- Establish unambiguous, deterministic rules for native asset issuance across the network.
- Ensure that every node can calculate and verify the total issued supply and current block subsidy independently without trusting third parties.
- Provide a mathematically provable supply boundary derived directly from the emission schedule.
- Maintain a strict boundary between newly minted supply (block rewards) and circulating supply transfers (transaction fees).

---

## 2. Asset Specification

| Field | Status | Specification |
| :--- | :--- | :--- |
| **Asset Name** | `TBD` | Full formal name of the native currency. |
| **Ticker** | `SCY` | Base ticker symbol for Scytale denomination. |
| **Smallest Unit** | `TBD` | Atomic unit precision and denomination (e.g., $10^8$ atomic units per 1 SCY). |

---

## 3. Locked Monetary Parameters

The core macroeconomic parameters for Scytale are locked as follows:

| Parameter | Value / Status | Description |
| :--- | :--- | :--- |
| **Target Maximum Supply ($S_{\text{max}}$)** | **`42,000,000 SCY`** | Mathematical target cap of total mintable currency units. |
| **Initial Block Reward ($R_0$)** | **`10 SCY`** per block | Base subsidy awarded to the miner during Epoch 0. |
| **Target Block Interval ($T_{\text{target}}$)** | **`60 seconds`** | Target duration between consecutive blocks. |
| **Halving Interval** | **`2,100,000 blocks`** | Number of blocks in each emission epoch. |
| **Reward Reduction Schedule** | **`50% (Geometric Halving)`** | Subsidy reduces by half every 2,100,000 blocks. |
| **Genesis Issuance ($G$)** | `TBD` | Initial allocation at block 0 (defaults to 0 for pure fair launch). |

---

## 4. Emission Schedule & Epochs

Block subsidies decrease deterministically across epochs according to a discrete geometric halving curve:

| Epoch | Block Height Range | Block Reward | Total Coins Minted in Epoch | Approx. Calendar Duration |
| :---: | :---: | :---: | :---: | :---: |
| **Epoch 0** | $0 \rightarrow 2,099,999$ | `10.00 SCY` | `21,000,000 SCY` | ~3.995 years |
| **Epoch 1** | $2,100,000 \rightarrow 4,199,999$ | `5.00 SCY` | `10,500,000 SCY` | ~3.995 years |
| **Epoch 2** | $4,200,000 \rightarrow 6,299,999$ | `2.50 SCY` | `5,250,000 SCY` | ~3.995 years |
| **Epoch 3** | $6,300,000 \rightarrow 8,399,999$ | `1.25 SCY` | `2,625,000 SCY` | ~3.995 years |
| **Epoch 4** | $8,400,000 \rightarrow 10,499,999$ | `0.625 SCY` | `1,312,500 SCY` | ~3.995 years |
| ... | ... | ... | ... | ... |
| **Epoch $N$** | $N \times 2,100,000 \rightarrow \dots$ | $10 \times 2^{-N}\text{ SCY}$ | $21,000,000 \times 2^{-N}\text{ SCY}$ | ~3.995 years |

### Epoch Duration Calculation
With a 60-second block interval target:
$$\text{Epoch Duration} = \frac{2,100,000 \text{ blocks} \times 60 \text{ seconds}}{86,400 \text{ seconds/day} \times 365.25 \text{ days/year}} \approx 3.995 \text{ years}$$

> [!NOTE]
> Calendar duration is an approximation based on target block times; actual calendar elapsed time will vary naturally with Proof-of-Work difficulty adjustments and network hashrate fluctuations.

---

## 5. Mathematical Supply Derivation

The maximum supply is mathematically derived from the infinite geometric series of block rewards:

$$S_{\text{total}} = G + \sum_{e=0}^{\infty} \left( \text{Blocks Per Epoch} \times R_0 \times 2^{-e} \right)$$

Assuming pure mining launch ($G = 0$):
$$S_{\text{max}} = 2,100,000 \times 10 \times \sum_{e=0}^{\infty} \left(\frac{1}{2}\right)^e = 21,000,000 \times 2 = 42,000,000 \text{ SCY}$$

```text
Initial Reward: 10 SCY/block
      ↓
Epoch Length: 2,100,000 blocks  ──> Epoch 0 Mint: 21,000,000 SCY (50% of cap)
      ↓
Geometric Decay Sum (×2)        ──> Total Max Cap: 42,000,000 SCY
```

> [!IMPORTANT]
> The target figure `42,000,000 SCY` represents the theoretical continuous limit. The exact final consensus integer sum will be governed by smallest-unit precision and integer truncation rules established in subsequent steps.

---

## 6. Emission Duration vs. Maximum Supply

A critical conceptual distinction must be maintained:

$$\text{Emission Duration} \ne \text{Maximum Supply}$$

- Choosing **42,000,000 SCY** (with an initial reward of 10 SCY/block) instead of 21,000,000 SCY (with an initial reward of 5 SCY/block) **does not double the duration of each epoch or the overall lifetime of the network**.
- Epoch durations are governed solely by the product of $\text{Halving Interval} \times \text{Block Interval}$ ($2,100,000 \times 60\text{s} \approx 3.995\text{ years}$).
- The parameter change only affects the **issuance velocity (coins generated per unit of time)**, not the calendar progression of halving milestones.

---

## 7. Integer Arithmetic & Reward Precision

To preserve absolute determinism across all operating systems and CPU architectures:
- **No Floating-Point Arithmetic:** All monetary values, fees, and subsidies in consensus code must be calculated strictly using unsigned integer representations in atomic units (`u64` or `u128`).
- **Rounding and Bitshifts:** Subsidies will be evaluated via integer division / bitshifts ($R_{\text{atomic}}(e) = R_0 \gg e$).
- **Decisions Required Prior to Code Implementation:**
  1. Smallest atomic unit denomination (e.g., 8 decimals $\implies 1 \text{ SCY} = 100,000,000 \text{ atomic units}$).
  2. Representation of initial reward in atomic units ($R_{0,\text{atomic}} = 10 \times 10^{\text{decimals}}$).
  3. Exact terminal epoch index $e_{\text{end}}$ where bitshifting truncates $R_{\text{atomic}}(e)$ to exactly $0$.
  4. Mathematical proof verifying that the discrete sum of all integer truncated block rewards never exceeds $S_{\text{max}}$.

---

## 8. Consensus Coinbase Constraints

Every valid candidate block must satisfy the upper bound constraint enforced by the consensus engine:

$$\text{Coinbase Output Value} \le R(h) + \sum_{i=1}^{N} \text{Fee}_i$$

```text
+-------------------------------------------------------------+
|                   Total Coinbase Payout                     |
+------------------------------+------------------------------+
|       Block Subsidy R(h)     |     Sum of Transaction Fees  |
|      (New Supply Minted)     | (Transfer of Existing Supply)|
+------------------------------+------------------------------+
```

### Consensus Rules:
1. **Invalid Over-Minting:** Any block containing a coinbase transaction output sum exceeding $R(h) + \sum \text{Fee}$ is immediately rejected.
2. **Permissible Under-Claiming:** A miner may claim less than the maximum allowable coinbase value. Unclaimed coins are permanently forfeited, reducing actual circulating supply below the theoretical cap.
3. **Coinbase Isolation:** Coinbase transactions cannot spend standard UTXO inputs and must be the first transaction in a block.

---

## 9. Transaction Fees

Transaction fees are determined by the net difference between consumed inputs and newly created outputs:

$$\text{Fee} = \sum \text{Input Values} - \sum \text{Output Values}$$

- **Non-Inflationary:** Fees circulate existing supply from users to miners; they create no new coins.
- **Economic Invariant:** $\sum \text{Input Values} \ge \sum \text{Output Values}$ for every non-coinbase transaction.

---

## 10. Protocol Supply Invariants

The Scytale ledger and consensus rules enforce the following immutable invariants:

1. **Supply Upper Bound:** $\text{Issued Supply}(h) \le \text{Maximum Supply}$ for all heights $h$.
2. **Non-Negative Reward:** $R(h) \ge 0$ for all heights $h$.
3. **No Arbitrary Minting:** Coinbase cannot create supply beyond $R(h) + \sum \text{Fee}$.
4. **Conservation of Existing Supply:** Transaction fees never increase the macro coin supply.
5. **Deterministic Arithmetic:** All monetary evaluations produce identical integer results on all compliant nodes.

---

## 11. Market Principles & Protocol Boundary

- **Issuance vs. Valuation:**
  - The protocol strictly enforces issuance volume, emission pace, and consensus validity.
  - The protocol does **not** control, peg, or guarantee the exchange rate, purchasing power, or market price of SCY.
- **Block Space Scarcity & Utility:**
  - Block space is scarce. Users compete for transaction confirmation priority by offering fee density.
  - As block subsidies diminish over successive epochs, transaction fees organically transition into the primary economic incentive for network security.
- **Miner Policy Autonomy:**
  - Miners freely determine local transaction prioritization and mempool acceptance policies without altering consensus rules.
