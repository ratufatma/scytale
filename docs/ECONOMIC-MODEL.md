# Scytale Economic Model

This document outlines the baseline economic mechanisms governing Scytale, detailing miner incentives, token issuance (emission), and the transaction fee market.

---

## 1. Miner Revenue Model

Miners secure the network through Proof-of-Work computation and earn revenue composed of two distinct components:

$$\text{Miner Revenue} = \text{Block Subsidy (Emission)} + \sum \text{Transaction Fees}$$

```text
+-------------------------------------------------------------+
|                        Miner Revenue                        |
+------------------------------+------------------------------+
|        Block Subsidy         |       Transaction Fees       |
|    - Protocol-defined rules  |    - User-defined difference |
|    - Predictable schedule    |    - Market-driven pricing   |
|    - Supply inflation        |    - Direct miner reward     |
+------------------------------+------------------------------+
```

---

## 2. Emission Schedule (Block Subsidy)

- **Source:** Minted according to deterministic, hardcoded protocol consensus rules.
- **Issuance Mechanism:** Awarded directly via the coinbase transaction of each valid block.
- **Subsidy Decay:** Follows a predictable geometric halving schedule parameterized by block height:
  - Initial block subsidy: 50 coins (with fixed decimal precision).
  - Halving interval: every 210,000 blocks.
  - Final cap: Asymptotic supply limit, after which block subsidy ceases ($0$), shifting miner incentives entirely to transaction fees.

---

## 3. Transaction Fee Structure

- **Calculation:** Every valid non-coinbase transaction implicitly awards a fee determined by:
  $$\text{Fee} = \sum \text{Input Values} - \sum \text{Output Values}$$
- **Beneficiary:** The fee is collected exclusively by the miner who successfully validates and incorporates the transaction into a confirmed block.
- **Purpose:** Compensates miners for consuming scarce block verification, propagation, and storage resources.

---

## 4. Block Space & Fee Market Dynamics

1. **Scarcity of Block Space:**
   - Blocks have finite capacity (defined by protocol byte limits and execution constraints).
   - When demand for ledger state transitions exceeds available block capacity per unit time, pending transactions accumulate in the mempool.

2. **Fee Prioritization as Miner Policy:**
   - Miners are economically incentivized to order and pack candidate transactions from their mempool by fee density (fee-per-byte or fee-per-unit-weight).
   - **Non-Consensus Nature:** The fee market mechanism is strictly a miner policy optimization, not a consensus validation rule. Miners remain free to select, order, or omit any valid transaction according to their local preferences.

3. **User Fee Bidding:**
   - Transactors can adjust their offered fee to signal urgency and compete for inclusion in immediate blocks during periods of network congestion.

---

## 5. Token Valuation & Utility Realities

- **Protocol Guarantees vs. Market Forces:**
  - The Scytale protocol defines and enforces technical rules: scarcity, verification, emission, and state integrity.
  - The protocol does not guarantee, peg, or mandate the financial price or purchasing power of the native token.
- **Utility & Demand Formation:**
  - Demand for the token arises from its utility as the native medium to settle ledger state transitions (transaction fees) and as an unencumbered digital bearer asset within the UTXO ecosystem.
  - Market valuation is determined purely by external supply and demand dynamics, liquidity, and participant consensus, rather than any protocol-enforced "intrinsic value".
