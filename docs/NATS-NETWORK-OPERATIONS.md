# Scytale NATS Network Operations

**Status:** Protocol foundation implemented; production networking pending integration
**Last reviewed:** 2026-09-09

This document is the operational companion to `P2P-NETWORK-SPEC.md`. It describes
what is currently executable and the requirements for operating NATS across VPS
instances. It does not treat a single public broker as a decentralized peer
network.

## Current repository behavior

The node currently provides:

- NATS pub/sub for blocks, transactions, and heartbeats;
- canonical height and tip hash in heartbeat messages;
- versioned `Hello`, locator, and block request message contracts;
- request/reply with a five-second timeout;
- network-id and genesis-hash handshake validation;
- in-memory peer expiry and per-peer request rate limiting.

The request/reply and registry primitives are not yet wired into the node
lifecycle. JetStream consumers, IBD application, TLS credentials, and NATS
authorization are deployment gates, not claims of current runtime behavior.

## Required VPS topology

Use one of these supported topologies:

1. A NATS cluster with a route between every NATS server. Each Scytale node
   connects to a local or nearest NATS server.
2. A NATS hub with leaf nodes at each VPS. Each leaf connects outbound to the
   hub and each Scytale node connects only to its local leaf.

Do not expose an unauthenticated `4222` listener to the public internet. Route
ports and monitoring ports must be restricted by firewall rules and private
network policy.

## Minimal NATS server policy

The production server must enable TLS and account authorization. The account
used by nodes should have access only to the Scytale subjects it needs:

```text
publish:   scytale.v1.blocks.>
publish:   scytale.v1.mempool.>
publish:   scytale.v1.peer.>
publish:   scytale.v1.sync.>
subscribe: scytale.v1.blocks.>
subscribe: scytale.v1.mempool.>
subscribe: scytale.v1.peer.>
subscribe: scytale.v1.sync.>
```

The exact `nats-server` configuration must be generated with the operator's
certificate and account credentials. Credentials must not be committed to this
repository.

## Rollout sequence

1. Deploy the NATS cluster or leaf topology and verify TLS/authentication from
   each VPS.
2. Enable JetStream and create durable streams for block and transaction
   subjects with bounded retention and maximum message size.
3. Deploy nodes with explicit `SCYTALE_NATS_URL` values; do not rely on the
   compiled default for production.
4. Enable handshake validation and reject mismatched network or genesis values.
5. Run IBD against a populated node and verify sequential validation, atomic
   commit, restart recovery, and replay after a disconnect.
6. Run duplicate, out-of-order, malformed, oversized, and flood tests.
7. Only then promote the networking row in `IMPLEMENTATION-STATUS.md` to
   `Implemented`.

## Production acceptance gates

- [ ] Multi-node handshake succeeds over TLS with authenticated accounts.
- [ ] Peer timeout removes stale peers without affecting canonical state.
- [ ] Headers and block request/reply complete IBD from genesis and from a
      partially synchronized height.
- [ ] Every downloaded block passes consensus and UTXO validation before commit.
- [ ] JetStream replay restores missed blocks and transactions after restart.
- [ ] Duplicate messages are idempotent and acknowledged safely.
- [ ] Rate limits and maximum payload sizes stop resource exhaustion.
- [ ] NATS restart, node restart, fork, reorg, and network partition tests pass.
- [ ] Firewall, backup, restore, alerting, and incident procedures are tested.

A VPS process being online is not evidence that these gates have passed.
