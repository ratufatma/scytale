# Scytale Hashing and Canonical Serialization Specification

This document defines the technical specification for cryptographic hashing, canonical binary serialization, and protocol identifier derivation within the Scytale blockchain engine.

---

## 1. Cryptographic Hash Primitive: BLAKE3

Scytale standardizes on **BLAKE3** as its primary cryptographic hashing primitive:

```text
Canonical Byte Sequence
          ↓
        BLAKE3
          ↓
    32-Byte Hash
```

| Property | Value | Description |
| :--- | :--- | :--- |
| **Algorithm** | `BLAKE3` | High-performance, cryptographically secure hash function. |
| **Output Digest Size** | `32 bytes (256 bits)` | Fixed-width byte array for all protocol digests. |
| **Role** | Core Hashing Primitive | Used for transaction identifiers, integrity verification, and tree commitments. |

### Cryptographic Scope & Purpose:
- Scytale utilizes well-established cryptographic primitives as fundamental building blocks, concentrating architectural innovation on protocol rules, ledger state models, economic mechanisms, and system execution.
- **Functional Boundary:** BLAKE3 functions strictly as a **collision-resistant cryptographic hashing primitive**. It does not inherently provide digital signatures, identity authentication, ownership authorization, encryption, or consensus logic.

---

## 2. Canonical Serialization

Hashing is deterministic only if the input byte sequence is uniquely and unambiguously defined. Scytale enforces **Canonical Serialization** across all protocol objects:

```text
Same Semantic Object
        ↓
Same Canonical Encoding
        ↓
Same Byte Sequence
        ↓
Same BLAKE3 Digest
```

```text
Different Serialized Bytes
        ↓
Different Hash Preimage
        ↓
Potentially Different Identifier
```

### Canonical Serialization Requirements:
1. **Absolute Determinism:** Every compliant node, regardless of CPU endianness, platform architecture, or runtime environment, must serialize identical logical data structures to the exact same sequence of bytes.
2. **Unambiguous Encodings:** Variable-length structures (e.g., vectors, byte arrays) must have clear, unambiguous length prefixes without multiple valid representations.
3. **Strict Member Ordering:** Arrays and collections whose order is semantically significant (such as transaction inputs and outputs) must strictly preserve index positions.
4. **Specification Status:**
   - `Serialization Format: TBD` (Binary encoding format specification remains open until final serialization milestone).
   - `Canonical Encoding Rules: TBD where low-level integer byte order / framing details remain open`.

> [!IMPORTANT]
> The concrete binary serialization encoding will be locked in a dedicated serialization implementation milestone without pre-committing to arbitrary third-party format crates.

---

## 3. Transaction Identifier (`TxID`)

A transaction's unique identity is deterministically derived from its canonical serialization:

$$\text{TxID} = \text{BLAKE3}(\text{Serialize}_{\text{canonical}}(\text{Transaction}))$$

```text
Transaction Struct (version, inputs, outputs)
                     ↓
         Canonical Serialization
                     ↓
             BLAKE3 Hash
                     ↓
        TxID (32-byte Identifier)
```

### Integration with Ledger State (`OutPoint`):
The `TxID` forms the primary cryptographic component of an `OutPoint`:

```text
OutPoint
├── txid: 32-byte Blake3 transaction hash
└── output_index: u32 (0-indexed output position)
```

Every unspent output on the ledger is thus anchored directly to the cryptographic digest of the creating transaction.

---

## 4. Hashing Boundaries: Protocol Data vs. Presentation

Scytale strictly isolates consensus-critical hashed data from presentation layers:

| Layer | Examples | Impact on Hash / TxID |
| :--- | :--- | :--- |
| **Protocol Canonical Representation** | Binary-encoded fields: `version`, `inputs`, `outputs`, integer values in `quanta`. | **Direct & Deterministic:** Any change produces a completely new hash/TxID. |
| **Presentation & Display Layer** | CLI tables, JSON formatting, pretty-printed outputs, whitespace, UI labels. | **Zero Impact:** Presentation variations do not alter the underlying canonical byte sequence. |

---

## 5. Network Determinism Requirements

Scytale consensus requires that:

> **Any validating node receiving the same protocol object and executing the same canonical serialization routine must produce the exact same byte sequence and the exact same cryptographic hash.**

### Systemic Implications:
- **Transaction Identification (`TxID`):** Unambiguous across all nodes and mempools.
- **UTXO Referencing (`OutPoint`):** Guarantees zero ambiguity when referencing spendable inputs.
- **Block & Chain References:** Assures deterministic block headers and state roots.
- **Value Provenance:** Guarantees that transaction ancestry DAGs are globally consistent.
- **Consensus Invariant:** Eliminates malleability stemming from variable serialization representations.

---

## 6. Protocol Identifier Policy

Scytale maintains a disciplined policy regarding cryptographic identifiers:
- **Identifier Minimization:** New hash algorithms or digest lengths are not introduced without explicit architectural necessity.
- **Current Locked Identifiers:**
  - `Transaction Identifier (TxID)` $\implies$ `BLAKE3 (32 bytes)`
- **Pending Identifiers:**
  - `Block Identifier (BlockID)` $\implies$ `TBD` (To be locked upon block header structure formalization).

---

## 7. Domain Separation Strategy

To prevent cross-type collision attacks where distinct protocol objects (e.g., a transaction and a future block header) could produce identical hashes if their serialized bytes happen to collide, Scytale plans a **Domain Separation** strategy.

```text
Conceptual Domain Separation:
"TX"    || canonical_tx_bytes    ──> BLAKE3 ──> TxID
"BLOCK" || canonical_block_bytes ──> BLAKE3 ──> BlockID
```

- **Status:** `Domain Separation Strategy: Pending protocol definition`
- Specific prefix tags, context strings, or keyed hashing mechanisms will be locked alongside the block header specification.

---

## 8. Versioning & Protocol Evolution

Canonical serialization rules are strictly consensus-critical:
- Any modification to binary encoding layouts, field serializations, or integer representations will alter hash outputs.
- Altering serialization for existing structures would break historical `TxID` calculation, invalidate `OutPoint` references, and fracture ledger provenance.
- All future evolutions of canonical serialization formats must be managed through formal protocol version transitions.
