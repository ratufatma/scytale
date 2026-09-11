# Production Deployment

The development cluster in `docker-compose.yml` publishes each node RPC port directly. For a public deployment, use `docker-compose.prod.yml` so only Caddy publishes ports 80 and 443.

## Setup

1. Copy `.env.prod.example` to `.env.prod`.
2. Set `SCYTALE_DOMAIN` to a DNS name pointing to the host.
3. Generate a password hash:

   ```sh
   docker run --rm caddy:2.10-alpine caddy hash-password --plaintext 'use-a-long-random-password'
   ```

4. Put the generated hash in `.env.prod` as `RPC_BASIC_AUTH_HASH`.
5. Start the production stack:

   ```sh
   docker compose -f docker-compose.prod.yml up -d --build
   ```

Caddy terminates TLS and proxies to the node over the private Docker network. Write endpoints `/api/v1/tx` and `/api/v1/alias/bind` require HTTP Basic Authentication. The node's own per-IP write limiter remains active as a second layer.

Do not commit `.env.prod`, passwords, private keys, or Caddy data volumes. Back up `scytale_node_data` and `caddy_data` separately.