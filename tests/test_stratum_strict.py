import socket
import json
import time
import sys
import threading
import blake3

HOST = "127.0.0.1"
PORT = 3333

def compact_to_target(compact: int) -> int:
    exponent = compact >> 24
    mantissa = compact & 0x007FFFFF
    if exponent <= 3:
        return mantissa >> (8 * (3 - exponent))
    else:
        return mantissa << (8 * (exponent - 3))

def calculate_share_target(compact: int, difficulty: float) -> int:
    base_target = compact_to_target(compact)
    if difficulty <= 0.0:
        return (1 << 256) - 1
    elif difficulty < 1.0:
        mult = int(1.0 / difficulty)
        return min((1 << 256) - 1, base_target * mult)
    else:
        diff_scaled = max(1, int(difficulty * 1000.0))
        return (base_target // diff_scaled) * 1000

class StratumClient:
    def __init__(self, host=HOST, port=PORT, timeout=10):
        self.host = host
        self.port = port
        self.timeout = timeout
        self.sock = None
        self.buffer = ""
        self.extranonce1 = None
        self.en2_size = 4
        self.difficulty = 1.0
        self.latest_job = None

    def connect(self):
        self.sock = socket.create_connection((self.host, self.port), timeout=self.timeout)

    def close(self):
        if self.sock:
            try:
                self.sock.close()
            except Exception:
                pass
            self.sock = None

    def send_json(self, req: dict):
        payload = (json.dumps(req) + "\n").encode("utf-8")
        self.sock.sendall(payload)

    def read_message(self, timeout=5.0):
        self.sock.settimeout(timeout)
        while "\n" not in self.buffer:
            chunk = self.sock.recv(4096).decode("utf-8")
            if not chunk:
                return None
            self.buffer += chunk
        line, self.buffer = self.buffer.split("\n", 1)
        line = line.strip()
        if not line:
            return self.read_message(timeout)
        msg = json.loads(line)
        if msg.get("method") == "mining.set_difficulty":
            self.difficulty = float(msg.get("params")[0])
        elif msg.get("method") == "mining.notify":
            self.latest_job = msg.get("params")
        return msg

    def wait_for_response(self, req_id: int, timeout=5.0):
        t0 = time.time()
        while time.time() - t0 < timeout:
            msg = self.read_message(timeout=1.0)
            if msg and msg.get("id") == req_id:
                return msg
        return None

    def subscribe(self, agent="scytale-strict-test/1.0"):
        self.send_json({"id": 1, "method": "mining.subscribe", "params": [agent, None]})
        t0 = time.time()
        while time.time() - t0 < 5.0 and self.extranonce1 is None:
            msg = self.read_message(timeout=1.0)
            if msg and msg.get("id") == 1:
                result = msg.get("result")
                self.extranonce1 = result[1]
                self.en2_size = result[2]
                return msg
        return None

    def authorize(self, worker_name="scy19kf72spzrs8v6e54tvcq48aeanzr3ux63r4x062h0e7rge3kad0s4v5cna.worker1"):
        self.send_json({"id": 2, "method": "mining.authorize", "params": [worker_name, "x"]})
        t0 = time.time()
        while time.time() - t0 < 5.0:
            msg = self.read_message(timeout=1.0)
            if msg and msg.get("id") == 2:
                return msg
        return None

    def wait_for_job(self, timeout=5.0):
        t0 = time.time()
        while time.time() - t0 < timeout and self.latest_job is None:
            self.read_message(timeout=1.0)
        return self.latest_job

    def build_merkle_root(self, extranonce2_hex: str) -> bytes:
        job = self.latest_job
        coinbase1_bytes = bytes.fromhex(job[2])
        coinbase2_bytes = bytes.fromhex(job[3])
        en1_bytes = bytes.fromhex(self.extranonce1)
        en2_bytes = bytes.fromhex(extranonce2_hex)
        merkle_branches = job[4]

        raw_cb = coinbase1_bytes + en1_bytes + en2_bytes + coinbase2_bytes
        current = blake3.blake3(raw_cb).digest()
        for branch in merkle_branches:
            hasher = blake3.blake3()
            hasher.update(current)
            branch_b = bytes.fromhex(branch)
            if branch_b != b"\x00" * 32:
                hasher.update(branch_b)
            current = hasher.digest()
        return current

    def find_share(self, extranonce2_hex="00000001", max_nonces=2_000_000, start_nonce=0):
        job = self.latest_job
        merkle_root = self.build_merkle_root(extranonce2_hex)

        version = int(job[5], 16)
        version_bytes = version.to_bytes(4, "little")
        prev_hash_bytes = bytes.fromhex(job[1])
        merkle_root_bytes = merkle_root
        utxo_root_bytes = bytes.fromhex(job[9]) if len(job) > 9 else b"\x00" * 32
        curtime = int(job[7], 16)
        curtime_bytes = curtime.to_bytes(8, "little")
        bits = int(job[6], 16)
        bits_bytes = bits.to_bytes(4, "little")

        target = calculate_share_target(0x1d00ffff, self.difficulty)

        prefix = (
            version_bytes
            + prev_hash_bytes
            + merkle_root_bytes
            + utxo_root_bytes
            + curtime_bytes
            + bits_bytes
        )

        for nonce in range(start_nonce, start_nonce + max_nonces):
            nonce_bytes = nonce.to_bytes(8, "little")
            header = prefix + nonce_bytes
            h = blake3.blake3(header).digest()

            h_le = int.from_bytes(h, "little")
            h_be = int.from_bytes(h, "big")
            if h_le <= target or h_be <= target:
                return nonce, curtime, h.hex()

        return None, curtime, None

# ── Skenario A: Handshake & Authorize ──────────────────────────────────────────
def test_scenario_a():
    print("\n" + "=" * 60)
    print("▶ SKENARIO A: Handshake & Authorize Normal")
    print("=" * 60)
    client = StratumClient()
    try:
        client.connect()
        sub_resp = client.subscribe()
        assert sub_resp is not None and sub_resp.get("result") is not None, "Subscribe gagal"
        print(f"[A] Subscribe Berhasil: en1={client.extranonce1}, en2_size={client.en2_size}")

        auth_resp = client.authorize()
        assert auth_resp is not None and auth_resp.get("result") is True, "Authorize gagal"
        print(f"[A] Authorize Berhasil: result={auth_resp.get('result')}")

        job = client.wait_for_job()
        assert job is not None, "Job mining tidak diterima"
        print(f"[A] Job Diterima: job_id={job[0]}, prev_hash={job[1][:16]}...")
        return True, "Handshake, otorisasi, dan penyiaran job valid"
    except Exception as e:
        return False, str(e)
    finally:
        client.close()

# ── Skenario B: Replay Attack (Duplicate Share) ────────────────────────────────
def test_scenario_b():
    print("\n" + "=" * 60)
    print("▶ SKENARIO B: Replay Attack (Duplicate Share Detection)")
    print("=" * 60)
    client = StratumClient()
    try:
        client.connect()
        client.subscribe()
        client.authorize()
        client.wait_for_job()

        en2 = "00000001"
        nonce, curtime, h = client.find_share(extranonce2_hex=en2, start_nonce=100)
        assert nonce is not None, "Gagal menemukan share valid untuk replay test"
        print(f"[B] Share Valid Ditemukan: nonce={nonce} (0x{nonce:016x}), hash={h[:24]}...")

        time_hex = f"{curtime:08x}"
        nonce_hex = f"{nonce:016x}"
        req_1 = {
            "id": 101,
            "method": "mining.submit",
            "params": [
                "scy19kf72spzrs8v6e54tvcq48aeanzr3ux63r4x062h0e7rge3kad0s4v5cna.worker_b",
                client.latest_job[0],
                en2,
                time_hex,
                nonce_hex
            ]
        }

        # 1. Kirim share pertama kali
        client.send_json(req_1)
        resp_1 = client.wait_for_response(101)
        print(f"[B] Kiriman 1 Response: {resp_1}")
        assert resp_1 and resp_1.get("result") is True, f"Kiriman pertama harus diterima: {resp_1}"

        # 2. Kirim ulang persis share yang sama
        req_2 = dict(req_1)
        req_2["id"] = 102
        client.send_json(req_2)
        resp_2 = client.wait_for_response(102)
        print(f"[B] Kiriman 2 Response (Replay): {resp_2}")
        assert resp_2 is not None, "Tidak menerima respon untuk replay"
        err = resp_2.get("error")
        assert err is not None, "Replay share harus ditolak dengan error"
        assert err.get("code") == 22, f"Expected error code 22 (DuplicateShare), got {err.get('code')}"
        print(f"[B] Replay Berhasil Ditolak: code={err.get('code')} message={err.get('message')}")
        return True, "Duplicate share berhasil ditolak dengan Error Code 22"
    except Exception as e:
        return False, str(e)
    finally:
        client.close()

# ── Skenario C: High Hash / Bad Nonce (Low Difficulty Share) ──────────────────
def test_scenario_c():
    print("\n" + "=" * 60)
    print("▶ SKENARIO C: High Hash / Bad Nonce (Target Mismatch)")
    print("=" * 60)
    client = StratumClient()
    try:
        client.connect()
        client.subscribe()
        client.authorize()
        client.wait_for_job()

        en2 = "00000001"
        merkle_root = client.build_merkle_root(en2)
        job = client.latest_job

        version = int(job[5], 16)
        version_bytes = version.to_bytes(4, "little")
        prev_hash_bytes = bytes.fromhex(job[1])
        merkle_root_bytes = merkle_root
        utxo_root_bytes = bytes.fromhex(job[9]) if len(job) > 9 else b"\x00" * 32
        curtime = int(job[7], 16)
        curtime_bytes = curtime.to_bytes(8, "little")
        bits = int(job[6], 16)
        bits_bytes = bits.to_bytes(4, "little")

        target = calculate_share_target(0x1d00ffff, client.difficulty)
        prefix = (
            version_bytes
            + prev_hash_bytes
            + merkle_root_bytes
            + utxo_root_bytes
            + curtime_bytes
            + bits_bytes
        )

        # Cari nonce yang hash-nya DI ATAS target (bad share / low difficulty)
        bad_nonce = None
        for nonce in range(100):
            nonce_bytes = nonce.to_bytes(8, "little")
            header = prefix + nonce_bytes
            h = blake3.blake3(header).digest()
            h_le = int.from_bytes(h, "little")
            h_be = int.from_bytes(h, "big")
            if h_le > target and h_be > target:
                bad_nonce = nonce
                break

        assert bad_nonce is not None, "Gagal menemukan bad nonce"
        print(f"[C] Mengirim Bad Nonce: {bad_nonce} (hash di atas batas kesulitan share)")

        req = {
            "id": 201,
            "method": "mining.submit",
            "params": [
                "scy19kf72spzrs8v6e54tvcq48aeanzr3ux63r4x062h0e7rge3kad0s4v5cna.worker_c",
                job[0],
                en2,
                f"{curtime:08x}",
                f"{bad_nonce:016x}"
            ]
        }
        client.send_json(req)
        resp = client.wait_for_response(201)
        print(f"[C] Response Bad Nonce: {resp}")
        assert resp is not None, "Tidak menerima respon dari server"
        err = resp.get("error")
        assert err is not None, "Bad nonce harus ditolak dengan error"
        assert err.get("code") == 23, f"Expected error code 23 (LowDifficultyShare), got {err.get('code')}"
        print(f"[C] High Hash Berhasil Ditolak: code={err.get('code')} message={err.get('message')}")
        return True, "High hash berhasil ditolak dengan Error Code 23"
    except Exception as e:
        return False, str(e)
    finally:
        client.close()

# ── Skenario D: Stale Job / Unknown Job ID ─────────────────────────────────────
def test_scenario_d():
    print("\n" + "=" * 60)
    print("▶ SKENARIO D: Stale / Unknown Job ID ('999999')")
    print("=" * 60)
    client = StratumClient()
    try:
        client.connect()
        client.subscribe()
        client.authorize()
        client.wait_for_job()

        req = {
            "id": 301,
            "method": "mining.submit",
            "params": [
                "scy19kf72spzrs8v6e54tvcq48aeanzr3ux63r4x062h0e7rge3kad0s4v5cna.worker_d",
                "999999",
                "00000001",
                "6aa543ea",
                "0000000000000001"
            ]
        }
        client.send_json(req)
        resp = client.wait_for_response(301)
        print(f"[D] Response Stale Job: {resp}")
        assert resp is not None, "Tidak menerima respon dari server"
        err = resp.get("error")
        assert err is not None, "Job palsu harus ditolak dengan error"
        assert err.get("code") == 21, f"Expected error code 21 (JobNotFound), got {err.get('code')}"
        print(f"[D] Stale Job Berhasil Ditolak: code={err.get('code')} message={err.get('message')}")
        return True, "Unknown job_id berhasil ditolak dengan Error Code 21"
    except Exception as e:
        return False, str(e)
    finally:
        client.close()

# ── Skenario E: Multi-Worker Stress Test (10 Threads) ─────────────────────────
def worker_thread(idx: int, results: dict):
    client = StratumClient()
    try:
        client.connect()
        sub_resp = client.subscribe(agent=f"scytale-stress-rig/{idx}")
        assert sub_resp and sub_resp.get("result"), f"Worker {idx} subscribe failed"

        worker_name = f"scy19kf72spzrs8v6e54tvcq48aeanzr3ux63r4x062h0e7rge3kad0s4v5cna.stress_{idx}"
        auth_resp = client.authorize(worker_name=worker_name)
        assert auth_resp and auth_resp.get("result") is True, f"Worker {idx} authorize failed"

        client.wait_for_job()
        assert client.latest_job is not None, f"Worker {idx} received no job"

        en2 = f"{idx + 1:08x}"
        nonce, curtime, h = client.find_share(
            extranonce2_hex=en2,
            max_nonces=2_000_000,
            start_nonce=idx * 50_000
        )
        assert nonce is not None, f"Worker {idx} failed to find valid share"

        submit_req = {
            "id": 400 + idx,
            "method": "mining.submit",
            "params": [
                worker_name,
                client.latest_job[0],
                en2,
                f"{curtime:08x}",
                f"{nonce:016x}"
            ]
        }
        client.send_json(submit_req)
        resp = client.wait_for_response(400 + idx)
        assert resp and resp.get("result") is True, f"Worker {idx} submit rejected: {resp}"
        results[idx] = True
    except Exception as e:
        results[idx] = str(e)
    finally:
        client.close()

def test_scenario_e():
    print("\n" + "=" * 60)
    print("▶ SKENARIO E: Multi-Worker Stress Test (10 Worker Serentak)")
    print("=" * 60)
    num_workers = 10
    threads = []
    results = {}

    t_start = time.time()
    for i in range(num_workers):
        t = threading.Thread(target=worker_thread, args=(i, results))
        threads.append(t)
        t.start()

    for t in threads:
        t.join(timeout=15.0)

    elapsed = time.time() - t_start
    success_count = sum(1 for v in results.values() if v is True)
    print(f"[E] {success_count}/{num_workers} Worker Berhasil Selesai dalam {elapsed:.2f} detik.")

    if success_count == num_workers:
        return True, f"10 worker berhasil terhubung, handshake, dan submit share serentak ({elapsed:.2f}s)"
    else:
        errors = [f"Worker {k}: {v}" for k, v in results.items() if v is not True]
        return False, f"Kegagalan pada worker: {'; '.join(errors)}"

# ── Main Orchestrator & Report Table ──────────────────────────────────────────
def main():
    print("=" * 70)
    print("  SUITE PENGUJIAN KETAT EMBEDDED STRATUM SERVER (SSP-1 PROTOCOL)")
    print("=" * 70)

    scenarios = [
        ("Skenario A: Handshake & Authorize", test_scenario_a),
        ("Skenario B: Replay Attack (Duplicate Share)", test_scenario_b),
        ("Skenario C: High Hash / Bad Nonce", test_scenario_c),
        ("Skenario D: Stale Job ('999999')", test_scenario_d),
        ("Skenario E: Multi-Worker Stress (10 Threads)", test_scenario_e),
    ]

    report = []
    all_passed = True

    for name, func in scenarios:
        passed, msg = func()
        report.append((name, passed, msg))
        if not passed:
            all_passed = False

    print("\n" + "=" * 70)
    print("                   TABEL STATUS KELULUSAN PENGUJIAN")
    print("=" * 70)
    print(f"{'Skenario Pengujian':<45} | {'Status':<8} | Catatan")
    print("-" * 70)
    for name, passed, msg in report:
        status_str = "PASS" if passed else "FAIL"
        print(f"{name:<45} | {status_str:<8} | {msg}")
    print("=" * 70)

    if all_passed:
        print("\n[KESIMPULAN] Seluruh skenario adversarial & stress test BERHASIL.")
        sys.exit(0)
    else:
        print("\n[KESIMPULAN] Terdapat skenario yang GAGAL.")
        sys.exit(1)

if __name__ == "__main__":
    main()
