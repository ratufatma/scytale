import socket
import json
import time
import sys
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

def read_lines(s: socket.socket, buffer: str):
    while "\n" not in buffer:
        chunk = s.recv(4096).decode("utf-8")
        if not chunk:
            break
        buffer += chunk
    lines = buffer.split("\n")
    return lines[:-1], lines[-1]

def main():
    print(f"[*] Menghubungi Stratum Pool di {HOST}:{PORT}...")
    try:
        with socket.create_connection((HOST, PORT), timeout=10) as s:
            print("[+] Koneksi TCP berhasil dibuat.")
            buffer = ""
            current_difficulty = 1.0

            # 1. Kirim mining.subscribe
            sub_req = {
                "id": 1,
                "method": "mining.subscribe",
                "params": ["scytale-test-miner/1.0", None]
            }
            s.sendall((json.dumps(sub_req) + "\n").encode("utf-8"))
            print("[>] Terkirim: mining.subscribe")

            extranonce1 = None
            en2_size = 4
            latest_job = None

            # Baca respon subscribe & notifikasi awal
            for _ in range(5):
                lines, buffer = read_lines(s, buffer)
                for line in lines:
                    line = line.strip()
                    if not line:
                        continue
                    msg = json.loads(line)
                    print(f"[<] Diterima: {line[:100]}...")
                    if msg.get("id") == 1:
                        result = msg.get("result")
                        assert result is not None, "Hasil subscribe tidak boleh null"
                        extranonce1 = result[1]
                        en2_size = result[2]
                        print(f"[V] Handshake Subscribe OK! Extranonce1: {extranonce1}, Extranonce2 Size: {en2_size} bytes")
                    elif msg.get("method") == "mining.set_difficulty":
                        current_difficulty = float(msg.get("params")[0])
                        print(f"[+] Kesulitan Share dari Pool: {current_difficulty}")
                    elif msg.get("method") == "mining.notify":
                        latest_job = msg.get("params")
                if extranonce1 is not None:
                    break

            assert extranonce1 is not None, "Gagal menerima extranonce1 dari pool"

            # 2. Kirim mining.authorize
            worker_name = "scy19kf72spzrs8v6e54tvcq48aeanzr3ux63r4x062h0e7rge3kad0s4v5cna.worker_rig1"
            auth_req = {
                "id": 2,
                "method": "mining.authorize",
                "params": [worker_name, "x"]
            }
            s.sendall((json.dumps(auth_req) + "\n").encode("utf-8"))
            print(f"\n[>] Terkirim: mining.authorize ({worker_name})")

            authorized = False
            for _ in range(5):
                lines, buffer = read_lines(s, buffer)
                for line in lines:
                    line = line.strip()
                    if not line:
                        continue
                    msg = json.loads(line)
                    print(f"[<] Diterima: {line[:100]}...")
                    if msg.get("id") == 2:
                        assert msg.get("result") is True, "Otorisasi worker harus bernilai true"
                        authorized = True
                        print("[V] Otorisasi Worker Berhasil (result: true)!")
                    elif msg.get("method") == "mining.set_difficulty":
                        current_difficulty = float(msg.get("params")[0])
                        print(f"[+] Kesulitan Share dari Pool: {current_difficulty}")
                    elif msg.get("method") == "mining.notify":
                        latest_job = msg.get("params")
                if authorized and latest_job is not None:
                    break

            assert authorized, "Worker gagal diotorisasi"

            # Tunggu notifikasi job jika belum diterima
            if latest_job is None:
                print("[*] Menunggu job mining.notify dari pool...")
                while latest_job is None:
                    lines, buffer = read_lines(s, buffer)
                    for line in lines:
                        line = line.strip()
                        if not line:
                            continue
                        msg = json.loads(line)
                        if msg.get("method") == "mining.set_difficulty":
                            current_difficulty = float(msg.get("params")[0])
                            print(f"[+] Kesulitan Share dari Pool: {current_difficulty}")
                        elif msg.get("method") == "mining.notify":
                            latest_job = msg.get("params")
                            break

            print("\n[+] Job mining diterima dari server Stratum:")
            job_id = latest_job[0]
            prev_hash_hex = latest_job[1]
            coinbase1_hex = latest_job[2]
            coinbase2_hex = latest_job[3]
            merkle_branches = latest_job[4]
            version_hex = latest_job[5]
            bits_hex = latest_job[6]
            curtime_hex = latest_job[7]
            clean_jobs = latest_job[8]
            utxo_root_hex = latest_job[9] if len(latest_job) > 9 else "0" * 64

            print(f"    Job ID          : {job_id}")
            print(f"    Prev Hash       : {prev_hash_hex}")
            print(f"    Coinbase1 Len   : {len(coinbase1_hex) // 2} bytes")
            print(f"    Coinbase2 Len   : {len(coinbase2_hex) // 2} bytes")
            print(f"    Merkle Branches : {len(merkle_branches)} branches")
            print(f"    Version         : {version_hex}")
            print(f"    Bits            : {bits_hex}")
            print(f"    CurTime         : {curtime_hex}")
            print(f"    UTXO Root       : {utxo_root_hex}")

            # 3. Rekonstruksi Merkle Root & Transaksi Coinbase
            extranonce2 = "00000001"
            raw_cb = (
                bytes.fromhex(coinbase1_hex)
                + bytes.fromhex(extranonce1)
                + bytes.fromhex(extranonce2)
                + bytes.fromhex(coinbase2_hex)
            )

            # Hitung BLAKE3 dari coinbase mentah
            current_hash = blake3.blake3(raw_cb).digest()
            for branch in merkle_branches:
                hasher = blake3.blake3()
                hasher.update(current_hash)
                branch_bytes = bytes.fromhex(branch)
                if branch_bytes != b"\x00" * 32:
                    hasher.update(branch_bytes)
                current_hash = hasher.digest()

            merkle_root = current_hash
            print(f"\n[+] Merkle Root Rekonstruksi: {merkle_root.hex()}")

            # 4. Tentukan Target Share Sesuai Tingkat Kesulitan Worker
            base_target = compact_to_target(0x1d00ffff)
            if current_difficulty <= 0.0:
                share_target = (1 << 256) - 1
            elif current_difficulty < 1.0:
                mult = int(1.0 / current_difficulty)
                share_target = min((1 << 256) - 1, base_target * mult)
            else:
                diff_scaled = max(1, int(current_difficulty * 1000.0))
                share_target = (base_target // diff_scaled) * 1000

            print(f"[+] Share Difficulty Aktif : {current_difficulty}")
            print(f"[+] Share Target (Hex)     : {hex(share_target)}")

            # 5. Iterasi PoW Nonce Lokal
            version = int(version_hex, 16)
            version_bytes = version.to_bytes(4, "little")
            prev_hash_bytes = bytes.fromhex(prev_hash_hex)
            merkle_root_bytes = merkle_root
            utxo_root_bytes = bytes.fromhex(utxo_root_hex)
            curtime = int(curtime_hex, 16)
            curtime_bytes = curtime.to_bytes(8, "little")
            bits = int(bits_hex, 16)
            bits_bytes = bits.to_bytes(4, "little")

            header_preimage_prefix = (
                version_bytes
                + prev_hash_bytes
                + merkle_root_bytes
                + utxo_root_bytes
                + curtime_bytes
                + bits_bytes
            )

            print("[*] Memulai pencarian PoW nonce...")
            t_start = time.time()
            found_nonce = None
            found_hash = None

            for nonce in range(2_000_000):
                nonce_bytes = nonce.to_bytes(8, "little")
                header_120 = header_preimage_prefix + nonce_bytes
                h_bytes = blake3.blake3(header_120).digest()

                h_le = int.from_bytes(h_bytes, "little")
                h_be = int.from_bytes(h_bytes, "big")

                if h_le <= share_target or h_be <= share_target:
                    found_nonce = nonce
                    found_hash = h_bytes.hex()
                    break

            dur = time.time() - t_start
            assert found_nonce is not None, "Gagal menemukan nonce valid dalam iterasi"
            print(f"[V] Share Valid Ditemukan!")
            print(f"    Nonce         : {found_nonce} (0x{found_nonce:016x})")
            print(f"    Hash BLAKE3   : {found_hash}")
            print(f"    Waktu Iterasi : {dur:.4f} detik ({found_nonce / max(dur, 0.0001):.0f} H/s)")

            # 6. Kirim mining.submit
            time_hex = f"{curtime:08x}"
            nonce_hex = f"{found_nonce:016x}"
            submit_req = {
                "id": 3,
                "method": "mining.submit",
                "params": [
                    worker_name,
                    job_id,
                    extranonce2,
                    time_hex,
                    nonce_hex
                ]
            }
            s.sendall((json.dumps(submit_req) + "\n").encode("utf-8"))
            print(f"\n[>] Terkirim: mining.submit (Nonce: 0x{nonce_hex}, Time: 0x{time_hex}, En2: {extranonce2})")

            # 7. Tunggu Respon Server
            submit_accepted = False
            for _ in range(5):
                lines, buffer = read_lines(s, buffer)
                for line in lines:
                    line = line.strip()
                    if not line:
                        continue
                    msg = json.loads(line)
                    print(f"[<] Diterima: {line}")
                    if msg.get("id") == 3:
                        assert msg.get("result") is True, f"Submit share ditolak: {msg.get('error')}"
                        submit_accepted = True
                        print("\n[SUCCESS] Share Diterima oleh Stratum Pool Server (result: true)!")
                if submit_accepted:
                    break

            assert submit_accepted, "Server tidak mengonfirmasi penerimaan share"
            print("[SUCCESS] Pengujian End-to-End SSP-1 Mining Submit Selesai Sempurna!")

    except Exception as e:
        print(f"[X] Kesalahan saat pengujian: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    main()
