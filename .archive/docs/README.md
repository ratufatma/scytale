# Scytale Documentation

This directory has one active documentation path and one historical archive.
The Rust implementation is the source of truth for values marked `Active`.

## Start Here

| Audience | Read | Purpose |
|---|---|---|
| New developer | [Developer Guide](DEVELOPER-GUIDE.md) | Build, run, test, and locate code |
| Node operator | [README operator guide](../README.md#cli-operator-guide) | Install CLI, run a node, mine, and use wallets |
| Protocol developer | [Protocol Reference](PROTOCOL-REFERENCE.md) | Canonical consensus and tokenomics values |
| Auditor | [Audit Matrix](AUDIT-MATRIX.md) | Requirement, code, test, and documentation traceability |
| Maintainer | [Implementation Status](IMPLEMENTATION-STATUS.md) | Implemented features and known gaps |

## Active Documents

### Protocol

- [Protocol Reference](PROTOCOL-REFERENCE.md)
- [Genesis Specification](GENESIS-SPEC.md)
- [Genesis Allocation](GENESIS-ALLOCATION.md)
- [Monetary Policy](MONETARY-POLICY.md)
- [Consensus](CONSENSUS-SPEC.md)
- [Block Format](BLOCK-SPEC.md)
- [Transactions](TRANSACTION-SPEC.md)
- [UTXO](UTXO-SPEC.md)
- [Proof of Work](POW-SPEC.md)
- [Difficulty](DIFFICULTY-SPEC.md)
- [Chain Selection](CHAIN-SELECTION-SPEC.md)
- [Authorization](AUTHORIZATION-SPEC.md)
- [Hashing and Serialization](HASHING-AND-SERIALIZATION-SPEC.md)

### Runtime and Operations

- [Architecture](ARCHITECTURE.md)
- [Node Lifecycle](NODE-LIFECYCLE-SPEC.md)
- [Storage](STORAGE-SPEC.md)
- [Mempool](MEMPOOL-SPEC.md)
- [Mining Lifecycle](MINING-LIFECYCLE-SPEC.md)
- [Networking](P2P-NETWORK-SPEC.md)
- [Testing Strategy](TESTING-STRATEGY.md)
- [Security Threat Model](SECURITY-THREAT-MODEL.md)

### Applications and Contracts

- [Passbook](PASSBOOK-CONCEPT.md)
- [Value Provenance](VALUE-PROVENANCE-SPEC.md)
- [Smart Contracts](SMART_CONTRACTS.md)
- [Developer Guide](DEVELOPER-GUIDE.md)
- [Vault Contract](../contracts/vault/src/lib.rs)
- [Scytale SDK](../crates/scytale-sdk/src/lib.rs)

## Audit Inputs

These reports are useful working inputs but are not protocol authority:

- [Documentation consistency report](DOCS_CONSISTENCY_REPORT.md)
- [Repository scan report](SCAN_REPORT.md)
- [Decision recommendations](DECISION_RECOMMENDATIONS.md)

Their conclusions must be rechecked against the current source tree and the
test matrix before being used as release evidence.

## Status Rules

- `Active`: describes the current repository and has a code source.
- `Partial`: implemented in part; gaps are listed in the status and audit matrix.
- `Proposed`: design material, not a current protocol rule.
- `Historical`: retained for context only; never use as an implementation reference.
- `Superseded`: replaced by a newer document.

Working notes, release runbooks, and duplicate task records are under
[archive/work-history](archive/work-history/README.md). They are not active
specifications.

## Documentation Rules

1. Define each protocol value once in [Protocol Reference](PROTOCOL-REFERENCE.md).
2. Link to the reference instead of copying tokenomics into every document.
3. Every active claim must name a source file or test.
4. Keep unimplemented behavior explicitly labeled `Not implemented`.
5. Run the audit commands in [Audit Matrix](AUDIT-MATRIX.md) before release.
