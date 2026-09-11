# Scytale Transaction Specification

This document defines the formal transaction specification for the Scytale blockchain engine. In Scytale, a transaction represents an atomic state transition that consumes existing Unspent Transaction Outputs (UTXOs) and produces new UTXOs.

---

## 1. Transaction Structural Model

A Scytale transaction is a deterministic container composed of version metadata, an array of inputs, and an array of outputs:

```text
Transaction
├── version: u32
├── inputs: Vec<TxIn>
└── outputs: Vec<TxOut>
```

### 1.1 OutPoint (Output Reference)
An `OutPoint` provides an exact, unique reference to a specific output of a previously confirmed transaction:

```text
OutPoint
├── txid: 32-byte transaction identifier (Blake3 hash)
└── index: u32 (0-indexed position of the output in the referenced transaction)
```

### 1.2 TxIn (Transaction Input)
A `TxIn` spends an existing unspent output from the active UTXO set:

```text
TxIn
├── previous_output: OutPoint
└── authorization: Vec<u8> (Cryptographic proof satisfying the locking condition)
```

### 1.3 TxOut (Transaction Output)
A `TxOut` defines new spendable value and the conditions required to spend it in a future transaction:

```text
TxOut
├── value: u64 (Integer amount denominated in quanta)
└── locking_condition: Vec<u8> (Encapsulated encumbrance required for spending)
```

---

## 2. Transaction Identity & Canonical Serialization

### 2.1 Transaction Identifier (TxID)
- Every transaction possesses a unique 32-byte identifier (`TxID`).
- The `TxID` is computed by executing the **Blake3** cryptographic hash function over the canonical binary serialization of the complete transaction payload:
  $$\text{TxID} = \text{Blake3}(\text{Serialize}_{\text{canonical}}(\text{Transaction}))$$

### 2.2 Canonical Serialization & Immutability
- **Determinism Requirement:** Every node across the network must produce byte-for-byte identical serializations for the same logical transaction structure.
- **Field Ordering:** Inputs and outputs must maintain strict array indexing; reordering inputs or outputs alters the `TxID` and constitutes a completely distinct transaction.
- **Immutability:** Once a transaction is signed and broadcast, any modification to its version, inputs, outputs, or authorization payloads invalidates its hash and cryptographic proofs.

---

## 3. Value Conservation & Transaction Fees

### 3.1 Value Accounting Unit
All transaction values, outputs, and fees are accounted for strictly in **integer quanta** ($1\text{ SCY} = 100,000,000\text{ quanta}$). Floating-point calculations are strictly prohibited.

### 3.2 Value Conservation Invariant
For every standard (non-coinbase) transaction, the total input value must strictly equal the sum of output values plus the transaction fee:

$$\sum_{i=1}^{M} \text{Input}_i.\text{value} = \sum_{j=1}^{N} \text{Output}_j.\text{value} + \text{Fee}$$

$$\text{Fee} = \sum_{i=1}^{M} \text{Input}_i.\text{value} - \sum_{j=1}^{N} \text{Output}_j.\text{value}$$

### 3.3 Strict Non-Inflationary Rule
- Ordinary transactions **cannot generate new monetary value**.
- A transaction where $\sum \text{Outputs} > \sum \text{Inputs}$ is mathematically invalid and immediately dropped by the engine.
- Transaction fees do not inflate the total supply; they represent an explicit transfer of existing quanta from transaction creators to the validating miner.

---

## 4. Transaction Validity Rules

A standard transaction is valid if and only if all of the following conditions are simultaneously satisfied:

| Rule | Requirement | Validation Scope |
| :--- | :--- | :--- |
| **Structural Integrity** | `inputs` is not empty; `outputs` is not empty; payload size is within protocol limits. | Stateless |
| **UTXO Existence** | Every referenced `previous_output` exists in the active ledger UTXO set. | Stateful (UTXO View) |
| **Exclusivity** | No two inputs within the same transaction reference the same `OutPoint` (intra-tx double spend). | Stateless |
| **Value Conservation** | $\sum \text{Input Values} \ge \sum \text{Output Values}$ in integer quanta. | Stateful (UTXO View) |
| **Authorization** | Each `TxIn.authorization` successfully satisfies the corresponding `TxOut.locking_condition`. | Stateful & Cryptographic |
| **Output Validity** | Every output value is non-zero ($> 0\text{ quanta}$) and does not exceed maximum supply. | Stateless |

---

## 5. Coinbase Transaction (Protocol Issuance)

A **Coinbase Transaction** is a specialized transaction responsible for protocol-defined issuance and miner fee settlement.

```text
Coinbase Transaction
├── version: u32
├── inputs[0]: TxIn (Carries arbitrary miner data / height signal, no OutPoint)
└── outputs[]: Vec<TxOut> (Miner reward payout destinations)
```

### Coinbase Distinctions:
1. **No Ordinary Inputs:** The coinbase transaction does not consume standard UTXOs. Its sole input carries arbitrary miner data rather than referencing an existing output.
2. **Block Position:** It must be the exact first transaction (`index 0`) in every block.
3. **Consensus Payout Limit:** The sum of all coinbase outputs cannot exceed the protocol block subsidy plus the sum of all valid transaction fees in that block:
   $$\sum \text{Coinbase Outputs} \le R_{\text{quanta}}(height) + \sum_{k=1}^{T} \text{Fee}_{\text{quanta}}(Tx_k)$$
4. **Supply Creation:** The block subsidy portion $R_{\text{quanta}}(height)$ represents newly minted supply; the fee portion represents existing supply collection.

---

## 6. Consensus Rules vs. Miner Policy

Scytale enforces a strict separation between protocol-level validation rules and local node/miner transaction selection heuristics:

```text
+-------------------------------------------------------------------------+
|                  Consensus Rules (Enforced by All Nodes)                |
|  - Valid UTXO references & no double-spending                           |
|  - Sum(Inputs) >= Sum(Outputs)                                          |
|  - Valid cryptographic authorization                                     |
|  - Canonical serialization & valid TxID                                 |
|  - Coinbase output <= Block Subsidy + Total Block Fees                  |
+------------------------------------+------------------------------------+
                                     |
                                     v
+-------------------------------------------------------------------------+
|                    Miner Policy (Local Node Autonomy)                   |
|  - Fee density prioritization (quanta-per-byte ordering)                |
|  - Mempool admission minimum fee rate                                   |
|  - Transaction replacement and eviction algorithms                      |
|  - Block template transaction packing preference                        |
+-------------------------------------------------------------------------+
```

Miners may adopt custom policies for transaction selection, but any mined block must strictly adhere to the universal consensus rules to be accepted by the network.

---

## 7. Determinism Guarantee

All validating nodes must reach an identical conclusion when evaluating any transaction against the active ledger state. Determinism is ensured by:
- Pure integer arithmetic in quanta for all value checks.
- Canonical serialization producing unambiguous byte representations.
- Stateless, side-effect-free authorization verification routines.
- Deterministic UTXO set transitions during block execution.
