# Scytale Difficulty Adjustment Specification

This document defines the formal specification for **Difficulty Adjustment** in the Scytale blockchain engine. It establishes how the consensus engine dynamically regulates the Proof-of-Work target to maintain long-term block production cadence.

---

## 1. Purpose & Objectives

The primary objective of difficulty adjustment in Scytale is to stabilize block generation velocity across fluctuating network hash rates:

- **Cadence Stabilization:** Regulates the emission and transaction settlement cadence toward the protocol target:
  $$\text{Target Block Interval} = 60\text{ seconds}$$
- **Dynamic Hashrate Adaptation:** Automatically increases difficulty when network computing power expands, and decreases difficulty when computing power contracts.
- **Deterministic Evaluation:** Enables every validating node across the network to compute and verify the exact same future target from historical chain data without relying on external oracles.
- **Statistical Convergence:** 60 seconds represents the **long-term statistical average** across many blocks, accommodating the natural Poisson variance of individual block discoveries.

---

## 2. Relationship with Proof-of-Work

Difficulty adjustment dictates the threshold evaluated during Proof-of-Work validation:

```text
Current Active Target
         ↓
Observe Block Timestamps across Epoch
         ↓
Calculate Retarget Adjustment
         ↓
New Consensus Target
         ↓
Applied to Subsequent Blocks
         ↓
Proof-of-Work Validation: Numeric(BLAKE3(Header)) <= Target
```

- Cross-References: [`docs/POW-SPEC.md`](POW-SPEC.md) and [`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md).

---

## 3. Target and Difficulty Duality

In Scytale consensus, **Target** is the concrete mathematical parameter stored in block headers and checked by nodes:

```text
Lower Target  ──>  Smaller Valid Hash Range  ──>  Higher Work Required  ──>  Higher Difficulty
Higher Target ──>  Larger Valid Hash Range   ──>  Lower Work Required   ──>  Lower Difficulty
```

- **Target ($T$):** The verified numerical boundary ($\text{Hash} \le T$).
- **Difficulty ($D$):** A human-readable and analytical metric inversely proportional to the target ($D \propto 1/T$). Consensus rules operate exclusively on the numerical target.

---

## 4. Adjustment Epoch Interval

The difficulty adjustment interval defines the number of blocks ($N$) evaluated before computing a new target:

| Parameter | Specification Status | Description |
| :--- | :--- | :--- |
| **`Adjustment Interval (N)`** | `TBD` | Number of blocks in a single difficulty epoch. |
| **`First Adjustment Boundary`** | `TBD` | Exact block height at which the initial retarget calculation triggers. |

---

## 5. Observed Time vs. Expected Time

At the conclusion of each adjustment epoch of $N$ blocks:

### 5.1 Expected Time ($T_{\text{expected}}$)
The nominal duration expected if every block were produced exactly at the 60-second target:
$$T_{\text{expected}} = N \times 60\text{ seconds}$$

### 5.2 Observed Time ($T_{\text{observed}}$)
The actual elapsed time recorded between epoch boundary blocks:
$$T_{\text{observed}} = \text{Timestamp}(\text{Block}_{e,\text{end}}) - \text{Timestamp}(\text{Block}_{e,\text{start}})$$

### 5.3 Directional Adjustment Logic
```text
T_observed < T_expected  ──>  Blocks produced too quickly  ──>  Target Decreases (Difficulty Increases)
T_observed > T_expected  ──>  Blocks produced too slowly   ──>  Target Increases (Difficulty Decreases)
T_observed == T_expected ──>  Blocks produced on target    ──>  Target Remains Unchanged
```

---

## 6. Conceptual Retarget Formula

Scytale applies a deterministic proportional adjustment ratio:

$$\text{New Target} = \text{Old Target} \times \frac{T_{\text{observed}}}{T_{\text{expected}}}$$

### Deterministic Integer Arithmetic Invariant:
- The calculation must be executed using **fixed-width, bounded integer arithmetic** without floating-point approximations.
- Multiplication and division steps must specify explicit rounding/truncation semantics to ensure bit-for-bit identical results on all node architectures.

---

## 7. Adjustment Clamping & Boundary Bounds

To prevent extreme volatility from sudden hashrate surges or drops, the protocol enforces adjustment clamping:

```text
                        Calculated Target Ratio
                                  ↓
       Clamp within [ Min Change Factor, Max Change Factor ]
                                  ↓
              Ensure: Minimum Target <= Target <= Maximum Target
                                  ↓
                       Final Consensus Target
```

### Boundary Parameters:
- `Maximum Target Change Per Adjustment: TBD` (e.g., maximum factor of $4\times$ upward or downward per epoch).
- `Minimum Target: TBD` (Absolute maximum difficulty ceiling).
- `Maximum Target: TBD` (Absolute minimum difficulty floor / genesis limit).

---

## 8. Genesis Difficulty & Chain Initialization

The network begins from a predefined baseline target:

| Parameter | Specification Status | Description |
| :--- | :--- | :--- |
| **`Genesis Target`** | `TBD` | Baseline Proof-of-Work threshold for Block 0. |
| **`Genesis Difficulty`** | `TBD` | Starting difficulty constant prior to the first retarget. |

---

## 9. Adversarial Resistance & Chain Security

Difficulty adjustment mechanisms are engineered with structural safeguards:
- **Timestamp Manipulation Resistance:** Boundary block timestamps must conform to the consensus acceptance rules defined in [`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md).
- **Damping & Clamping:** Prevents malicious actors with temporary hash power from forcing abnormal target spikes or drops.
- **Historical Verifiability:** Target calculations depend exclusively on consensus-visible on-chain headers.

---

## 10. Consensus Validation vs. Miner Autonomy

| Layer | Responsibility |
| :--- | :--- |
| **Consensus Rules (Universal)** | - Evaluates the mathematically required target for the current height.<br>- Enforces epoch boundaries and retarget formulas.<br>- Enforces clamping and target boundaries.<br>- Rejects any block whose header target deviates from the consensus formula. |
| **Miner Autonomy (Local)** | - Dynamically decides when to start/stop mining hardware.<br>- Selects nonce ranges, parallelization strategies, and mining algorithms.<br>- Miners cannot arbitrarily modify or negotiate the difficulty target. |

---

## 11. Economic Relationship with Monetary Policy

- **Issuance Velocity vs. Subsidy Rules:**
  - Fluctuations in actual block times temporarily speed up or slow down real-world issuance pacing.
  - However, the **subsidy awarded per block** remains strictly dictated by block height and the deterministic halving schedule ($10\text{ SCY} \rightarrow 5\text{ SCY} \dots$ every 2,100,000 blocks).
  - Cross-Reference: [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md).

---

## 12. Open Questions & Pending Specifications

The following parameters are designated as **TBD** pending final mathematical parameterization:

| Area | Status | Key Focus |
| :--- | :--- | :--- |
| **Adjustment Epoch Interval ($N$)** | `TBD` | Number of blocks per difficulty period. |
| **Clamping Factor Boundaries** | `TBD` | Maximum allowed upward and downward ratio per retarget. |
| **Target Representation Encoding** | `TBD` | Compact 32-bit floating exponent/mantissa vs full 256-bit scalar. |
| **Integer Arithmetic Width & Rounding** | `TBD` | `u256` / `u512` intermediate arithmetic and integer division truncation rules. |
| **Genesis Target Value** | `TBD` | Initial difficulty for Block 0. |
| **Minimum / Maximum Target Limits** | `TBD` | Hard upper and lower protocol thresholds. |
| **First Retarget Height** | `TBD` | Block height where the first adjustment executes. |

---

## 13. Cross-Specification References

- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block header layout and timestamp consensus constraints.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: BLAKE3 Proof-of-Work threshold verification.
- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md)**: BLAKE3 primitive and canonical serialization.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: 60-second block target and emission schedule.
- **[`docs/ECONOMIC-MODEL.md`](ECONOMIC-MODEL.md)**: Mining revenue and fee dynamics.
