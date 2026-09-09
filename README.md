# Scytale

**Scytale** is a modular, high-performance Layer-1 blockchain engine designed with:
- **Redb** for fast, embedded, ACID-compliant key-value storage.
- **Deterministic UTXO Model** with authenticated state commitment (`utxo_root` in 120-Byte BlockHeader).
- **Proof-of-Work (PoW)** consensus powered by CPU-friendly BLAKE3 hashing.
- **Deterministic Zero-Float Fee Market** with integer-only arithmetic.
- **NATS network transport** for block and transaction broadcast.

---

## 📁 Repository Structure

```text
scytale/
├── Cargo.toml                        # Workspace Manifest
├── README.md                         # Project Overview
├── Dockerfile                        # Multi-stage production container build
├── docker-compose.yml                # Cluster topology (Mining, Relay, Seeder, Coldstart)
├── crates/
│   ├── scytale-primitives/           # Core cryptographic types, BlockHash, & Bech32 encoding
│   ├── scytale-core/                 # Blocks, 120B Header, Transactions, UTXOs & Merkle trees
│   ├── scytale-script/               # Forth-like stack execution engine & opcodes
│   ├── scytale-storage/              # ACID redb engine, UTXO table, snapshots & indexers
│   ├── scytale-consensus/            # PoW validation, BLAKE3 target, emissions & reorg rules
│   ├── scytale-mempool/              # Transaction mempool & priority fee market
│   ├── scytale-mining/               # CPU miner worker, template builder & coinbase generation
│   └── scytale-bridge/               # IPC framing and network event types
├── apps/
│   ├── scytale-node/                 # Full node daemon, HTTP RPC gateway & NATS transport
│   └── scytale-cli/                  # Wallet management, keygen, balance & miner operator CLI
├── web/
│   └── explorer/                     # Embedded live block explorer and RPC dashboard
├── scripts/                          # Automated cluster verification & chaos test suites
└── docs/                             # Consolidated architecture & technical specifications
```

---

## 🚀 Getting Started

### Prerequisites
- **Rust**: 1.75+ (2021 edition)
- **Docker & Docker Compose**: (optional, for multi-node cluster verification)

### Build & Check
```bash
# Build & check all Rust workspace crates
cargo check --workspace
cargo build --workspace
cargo test --workspace

```

### Run Node
```bash
cargo run -p scytale-node -- --help
```

---

## Documentation

Use the [documentation index](docs/README.md) as the single entry point.

- [Developer Guide](docs/DEVELOPER-GUIDE.md)
- [Protocol Reference](docs/PROTOCOL-REFERENCE.md)
- [Implementation Status](docs/IMPLEMENTATION-STATUS.md)
- [Audit Matrix](docs/AUDIT-MATRIX.md)
- [Historical working documents](docs/archive/work-history/README.md)

## CLI Operator Guide

This section documents the implemented `scytale-cli` commands. The CLI uses two
node interfaces:

- **IPC socket** for node control and local queries. The default is
	`/tmp/scytale.sock`.
- **HTTP gateway** for address-based passbook queries, wallet account
	registration, and transaction submission. The default bind is
	`0.0.0.0:8332` (`http://127.0.0.1:8332` locally).

Use `--socket` and `--node-url` when the node uses non-default addresses:

```bash
scytale-cli --socket /tmp/scytale.sock \
	--node-url http://127.0.0.1:8332 status
```

### Build and install the CLI

From the repository root:

```bash
cargo build --release -p scytale-node -p scytale-cli
```

The binaries are written to `target/release/`. The optional installer creates
the `scy` launcher and installs `scytale-cli`:

```bash
python3 install.py --lang id --scope global --network local
scy --help
```

### Start a node

Start a standalone local node with easy test difficulty and automatic mining:

```bash
target/release/scytale-node start \
	--data-dir .scytale-dev \
	--socket /tmp/scytale.sock \
	--no-p2p \
	--http-bind 127.0.0.1:8332 \
	--target 0x217fffff \
	--mine
```

For a network node, configure the NATS broker with `--nats`:

```bash
target/release/scytale-node start \
	--data-dir .scytale \
	--socket /tmp/scytale.sock \
	--http-bind 0.0.0.0:8332 \
	--nats nats://127.0.0.1:4222 \
	--mine
```

Because the HTTP gateway binds an interface, restrict access with a firewall or
an explicit bind address when running on an internet-facing host. The gateway
also exposes transaction submission, so it should not be treated as a
read-only public endpoint.

The `--target` option is intended for local or testnet difficulty. Do not copy
the local target to a production network without checking the network rules.

To set the miner payout locking script, configure it when starting the node:

```bash
--miner-payout 010203
```

This value is a locking script in hexadecimal, not a Bech32 wallet address.
The payout configuration belongs to the node process and is not changed by
`scytale-cli mine start`.

### Inspect and control the node

```bash
scytale-cli status
scytale-cli mine start
scytale-cli mine stop
scytale-cli stop
```

Mining can also be enabled at node startup with `scytale-node --mine`. The
miner is a background worker. It builds a candidate block from the canonical
tip and mempool, searches a BLAKE3 proof of work, validates the solved block,
and then commits or broadcasts it.

### Create and restore a wallet

The default wallet file is `~/.scytale/wallet.json`. Wallet files contain
sensitive key material and should not be copied to an untrusted machine.

```bash
scytale-cli wallet new
scytale-cli wallet info
```

The `wallet new` command asks for a six-digit PIN. If the HTTP gateway is
available, it also attempts to register the wallet account. If the gateway is
unavailable, the wallet can still be created locally and registered later.

Create a wallet with a BIP-39 recovery phrase:

```bash
scytale-cli wallet new --mnemonic --words 12
```

The recovery phrase is only printed when `--dev` or `--raw` is used. Store it
offline and never place it in shell history or source control.

Restore a wallet from a recovery phrase:

```bash
scytale-cli wallet restore \
	--phrase "word1 word2 word3 ..." \
	--file ~/.scytale/wallet.json
```

Use `--force` only when intentionally replacing an existing wallet file.

### Check balance and passbook

```bash
scytale-cli balance
scytale-cli wallet info
scytale-cli passbook
```

For an address-based passbook query, use a Bech32 `scy1...` address:

```bash
scytale-cli passbook show scy1... --limit 50
scytale-cli passbook statement scy1... \
	--output passbook-statement.json \
	--verify
```

`1 SCY` equals `100000000` quanta. The `balance` and `passbook` commands use
the IPC socket, while the address-based passbook subcommands use the HTTP
gateway.

### Send a P2PKH transfer

The recommended wallet transfer flow signs locally with the wallet key and
submits the signed transaction through the node socket:

```bash
scytale-cli transfer-p2pkh \
	--to scy1... \
	--amount 100000000 \
	--fee 1000 \
	--wallet-file ~/.scytale/wallet.json
```

The amount and fee are integer quanta. The command selects wallet UTXOs,
creates change back to the sender, asks for the wallet PIN, signs every input,
and submits the transaction to the mempool.

An `SCY-########` account number can also be used as the recipient when the
HTTP gateway is reachable. The lower-level `send` command is for identity
store locking scripts or aliases:

```bash
scytale-cli account new alice
scytale-cli account new bob
scytale-cli account switch alice
scytale-cli send --to bob --amount 100000000 --fee 1000
```

### Embed data and trace provenance

Store up to 80 bytes in an OP_RETURN output. Text is interpreted as UTF-8;
values beginning with `0x` are interpreted as hexadecimal:

```bash
scytale-cli embed-data --data "hello scytale" --fee 1000
scytale-cli embed-data --data 0xdeadbeef --fee 1000
```

Trace the value history of a transaction output:

```bash
scytale-cli provenance \
	--txid TRANSACTION_ID_HEX \
	--index 0 \
	--max-depth 20
```

### Smart contract tools

The contract commands operate on eUTXO WebAssembly contracts:

```bash
scytale-cli contract inspect --wasm path/to/contract.wasm
scytale-cli contract build --path contracts/vault --package vault
scytale-cli contract deploy \
	--wasm target/wasm32-unknown-unknown/release/contract.wasm \
	--amount 100000000 \
	--datum HEX_DATUM \
	--dry-run
scytale-cli contract call \
	--utxo TX_HASH:0 \
	--wasm path/to/contract.wasm \
	--datum HEX_DATUM \
	--redeemer HEX_REDEEMER \
	--to scy1... \
	--dry-run
```

Use `--dry-run` during development. Deployment and calls require sufficient
wallet funds and a reachable HTTP gateway unless they are being simulated.

### Troubleshooting

If the CLI reports that the daemon is unavailable, check that the node is
running and that both paths match:

```bash
test -S /tmp/scytale.sock
curl http://127.0.0.1:8332/api/v1/status
scytale-cli --socket /tmp/scytale.sock \
	--node-url http://127.0.0.1:8332 status
```

For a complete command list, use:

```bash
scytale-cli --help
scytale-cli wallet --help
scytale-cli contract --help
```














