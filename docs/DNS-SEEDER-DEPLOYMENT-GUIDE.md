# Scytale Protocol — Autonomous DNS Seeder Deployment & Cloudflare NS Delegation Guide

```text
Target Binary : network/cmd/scytale-seeder
Module        : network/internal/seeder
Domain Target : seed.scytale.org
Nameserver    : ns1.seed.scytale.org
Default Ports : 53 (DNS UDP/TCP), 9001 (Scytale P2P Crawler)
TTL Setting   : 60 Seconds
Status        : Production Ready
```

---

## 1. Overview & Architecture

The **Scytale Autonomous DNS Seeder** (`scytale-seeder`) solves the network cold-start bootstrap problem. New validator and relay nodes joining the Scytale network can resolve DNS records (e.g. `seed.scytale.org`) to discover active, healthy peers without requiring hardcoded static IP lists.

To operate an autonomous seeder on a custom domain managed by Cloudflare:
1. **Cloudflare** remains the authoritative DNS provider for the zone apex (`scytale.org`).
2. A dedicated nameserver glue record (`ns1.seed.scytale.org`) is pointed to the seeder's public server IP.
3. The subdomain `seed.scytale.org` is delegated via an `NS` record to `ns1.seed.scytale.org`.
4. The `scytale-seeder` binary runs on the server, continuously probing the P2P mesh, filtering unhealthy/Sybil nodes, and serving live IP subsets over UDP/TCP port 53.

```text
┌────────────────────────────────────────────────────────────────────────┐
│                          GLOBAL DNS RESOLVER                           │
│                      (1.1.1.1 / 8.8.8.8 / Local ISP)                   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                         1. Query: A seed.scytale.org
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        CLOUDFLARE DNS DASHBOARD                        │
│                          (Zone: scytale.org)                           │
│                                                                        │
│   Record 1 (Glue A):                                                   │
│   • Type: A                                                            │
│   • Name: ns1.seed                                                     │
│   • Target: <YOUR_SERVER_PUBLIC_IP>                                    │
│   • Proxy: DNS Only (Grey Cloud) ◄─── CRITICAL: MUST BE GREY CLOUD!    │
│                                                                        │
│   Record 2 (NS Delegation):                                            │
│   • Type: NS                                                           │
│   • Name: seed                                                         │
│   • Target: ns1.seed.scytale.org                                       │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                         2. Referral: NS ns1.seed.scytale.org
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                     SERVER SCYTALE SEEDER DAEMON                       │
│                     (Port 53 UDP & TCP / Port 9001)                    │
│                                                                        │
│   • Listens on port 53 (UDP & TCP)                                     │
│   • Queries Active "Good Nodes" from Memory Store                      │
│   • Fisher-Yates Random Shuffle (up to 16 records)                     │
│   • Responds with Authoritative = true, TTL = 60s                      │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Step-by-Step Cloudflare DNS Configuration

### Step 1: Obtain Public IP Addresses
Deploy your server on any cloud provider (e.g. AWS, Hetzner, DigitalOcean, Linode) and note its static public IPs:
- IPv4 (required): e.g. `203.0.113.10`
- IPv6 (optional): e.g. `2001:db8::10`

### Step 2: Add Glue Host Records in Cloudflare
A glue record tells DNS resolvers where the nameserver itself is hosted:
1. Log in to your **Cloudflare Dashboard**.
2. Select the domain zone (e.g. `scytale.org`).
3. Navigate to **DNS** $\rightarrow$ **Records** $\rightarrow$ Click **Add record**:
   - **Type**: `A`
   - **Name**: `ns1.seed` *(generates `ns1.seed.scytale.org`)*
   - **IPv4 address**: `<YOUR_SERVER_PUBLIC_IP>`
   - **Proxy status**: **DNS only** *(Grey cloud icon)*
   - **TTL**: `Auto`
4. *(Optional IPv6)* Add an `AAAA` record:
   - **Type**: `AAAA`
   - **Name**: `ns1.seed`
   - **IPv6 address**: `<YOUR_SERVER_PUBLIC_IPV6>`
   - **Proxy status**: **DNS only**

> [!CAUTION]
> **NEVER PROXY (ORANGE CLOUD) THESE RECORDS.**
> Cloudflare's HTTP reverse proxy only handles web traffic on ports 80/443. Orange-clouding DNS glue/NS records will block incoming UDP/TCP port 53 packets, breaking the DNS seeder immediately.

### Step 3: Add NS Delegation Record in Cloudflare
Delegate queries for `seed.scytale.org` to your authoritative nameserver:
1. Click **Add record**:
   - **Type**: `NS`
   - **Name**: `seed` *(generates `seed.scytale.org`)*
   - **Nameserver**: `ns1.seed.scytale.org`
   - **TTL**: `Auto`
2. Click **Save**.

---

## 3. Host System Configuration & Systemd Service

### A. Firewall Configuration
Ensure port 53 (UDP and TCP) and port 9001 (P2P wire) are open:
```bash
# Allow DNS inbound traffic
sudo ufw allow 53/udp
sudo ufw allow 53/tcp

# Allow Scytale P2P crawler traffic
sudo ufw allow 9001/tcp

sudo ufw reload
```

### B. Disable Ubuntu `systemd-resolved` Port 53 Stub Listener
Ubuntu often runs a local DNS stub resolver on `127.0.0.53:53`, preventing other processes from binding to port 53:
```bash
# Check if port 53 is already bound
sudo lsof -i :53

# If systemd-resolved is using port 53, disable the stub listener:
sudo sed -i 's/#DNSStubListener=yes/DNSStubListener=no/' /etc/systemd/resolved.conf
sudo systemctl restart systemd-resolved
```

### C. Install and Run via Systemd
Compile and copy the seeder binary:
```bash
cd network
go build -v -o /usr/local/bin/scytale-seeder ./cmd/scytale-seeder
```

Create `/etc/systemd/system/scytale-seeder.service`:
```ini
[Unit]
Description=Scytale Autonomous DNS Seeder
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/var/lib/scytale
ExecStart=/usr/local/bin/scytale-seeder \
    --domain=seed.scytale.org \
    --nameserver=ns1.seed.scytale.org \
    --listen=:53 \
    --p2p-port=9001 \
    --seeds=node1.scytale.org:9001,node2.scytale.org:9001 \
    --data-file=/var/lib/scytale/seeder_nodes.json \
    --workers=16 \
    --probe-interval=15m
Restart=always
RestartSec=10s
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

Enable and start the service:
```bash
sudo mkdir -p /var/lib/scytale
sudo systemctl daemon-reload
sudo systemctl enable --now scytale-seeder
sudo systemctl status scytale-seeder
```

---

## 4. Verification & Diagnostics

### 1. Direct Server Query
Verify that your seeder process answers DNS queries directly:
```bash
dig @<YOUR_SERVER_PUBLIC_IP> seed.scytale.org A
```
Check for:
- `status: NOERROR`
- Flags containing `aa` (Authoritative Answer)
- Answer section containing active node IPs with `TTL 60`

### 2. Nameserver Record Query
```bash
dig @<YOUR_SERVER_PUBLIC_IP> seed.scytale.org NS
```
Should return:
```text
seed.scytale.org.   60   IN   NS   ns1.seed.scytale.org.
```

### 3. Recursive Trace Query
Verify the delegation path from internet root servers down to your server:
```bash
dig +trace seed.scytale.org A
```

### 4. Public Resolver Query
Once DNS propagation completes (usually 1–5 minutes), verify resolution from public DNS providers:
```bash
dig @1.1.1.1 seed.scytale.org A
dig @8.8.8.8 seed.scytale.org A
```
Both queries will return randomized subsets of healthy Scytale peer IP addresses.
