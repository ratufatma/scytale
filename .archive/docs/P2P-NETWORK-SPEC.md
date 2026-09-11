# P2P Network Specification

## Status

The node exposes a transport-neutral network boundary. The previous broker-based implementation has been removed. Direct peer transport, discovery, handshake, propagation, and synchronization remain planned work.

## Boundary

Consensus, storage, UTXO processing, mempool admission, and mining must not depend on a transport implementation. Network adapters must submit incoming domain data through the same canonical validation paths used by local operations.

## Required Direct Transport

The future transport must provide bounded framing, authenticated peer identity, handshake validation, peer lifecycle management, request correlation, backpressure, reconnect handling, and explicit shutdown.

## Security Invariants

- Network messages never bypass block or transaction validation.
- Message sizes and queues are bounded.
- Peer identity is authenticated rather than self-declared.
- A failed peer cannot corrupt canonical storage.
- Discovery and bootstrap nodes are not consensus authorities.
