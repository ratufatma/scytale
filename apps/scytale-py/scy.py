#!/usr/bin/env python3
"""Human-friendly bilingual wrapper around the Rust Scytale CLI."""

import argparse
import getpass
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RUST_BINARY = ROOT / "target" / "release" / "scytale-cli"
CONFIG_PATH = Path.home() / ".scytale" / "config.json"
DEFAULT_WALLET = Path.home() / ".scytale" / "wallet.json"
QUANTA_PER_SCY = 100_000_000
CYAN = "\033[36m"
BOLD = "\033[1m"
RESET = "\033[0m"
MINI_LOGO = f"{BOLD}{CYAN}SCYTALE // LIGHTNING-FAST LAYER-1{RESET}"
DEFAULT_CONFIG = {
    "language": "en",
    "install_scope": "global",
    "network": "global",
    "node_url": "http://116.212.72.89:8332",
    "socket_path": "/tmp/scytale.sock",
}


def load_config() -> dict[str, str]:
    try:
        data = json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        data = {}
    return {**DEFAULT_CONFIG, **{key: str(value) for key, value in data.items()}}


def save_config(config: dict[str, str]) -> None:
    CONFIG_PATH.parent.mkdir(parents=True, exist_ok=True)
    CONFIG_PATH.write_text(json.dumps(config, indent=2) + "\n", encoding="utf-8")


def language(args: argparse.Namespace, config: dict[str, str]) -> str:
    return args.lang or config.get("language", "en")


def wallet_path(args: argparse.Namespace) -> Path:
    return Path(args.wallet_file).expanduser() if args.wallet_file else DEFAULT_WALLET


def account_from_wallet(path: Path) -> str:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return "Not registered"
    return str(data.get("account_number") or "Not registered")


def rust_command(args: argparse.Namespace, command: str, *values: str) -> list[str]:
    config = load_config()
    socket_path = args.socket if args.socket is not None else config["socket_path"]
    node_url = args.node_url if args.node_url is not None else config["node_url"]
    result = [str(RUST_BINARY), "--socket", socket_path, "--node-url", node_url]
    result.extend([command, *values])
    return result


def run_rust(args: argparse.Namespace, command: str, *values: str) -> tuple[int, str]:
    if not RUST_BINARY.exists():
        return 127, f"Rust binary not found: {RUST_BINARY}"
    try:
        completed = subprocess.run(
            rust_command(args, command, *values), capture_output=True, text=True, check=False
        )
    except OSError as error:
        return 127, str(error)
    return completed.returncode, (completed.stdout + "\n" + completed.stderr).strip()


def human_error(output: str, lang: str) -> str:
    lowered = output.lower()
    if "pin salah" in lowered or "failed to decrypt" in lowered or "authentication" in lowered:
        return "PIN yang Anda masukkan salah. Transaksi dibatalkan." if lang == "id" else "The PIN is incorrect. The transaction was cancelled."
    if "insufficient" in lowered or "saldo kurang" in lowered or "tidak mencukupi" in lowered:
        return "Saldo Anda tidak mencukupi untuk melakukan transaksi ini." if lang == "id" else "Your balance is insufficient for this transaction."
    if "connection refused" in lowered or "daemonnotrunning" in lowered or "daemon is not running" in lowered:
        return "Tidak dapat terhubung ke daemon scytale-node." if lang == "id" else "Cannot connect to the scytale-node daemon."
    if "not found" in lowered or "tidak ditemukan" in lowered:
        return "Nomor rekening tujuan tidak ditemukan di jaringan." if lang == "id" else "The destination account was not found on the network."
    return output or ("Perintah gagal dijalankan." if lang == "id" else "The command failed.")


def find_account(text: str) -> str:
    for token in text.replace("\n", " ").split():
        candidate = token.strip("|,:()[]")
        if len(candidate) == 10 and candidate.startswith("SCY-") and candidate[4:].isdigit():
            return candidate
    return ""


def prompt_pin(confirm: bool, lang: str) -> str:
    first_prompt = "Masukkan PIN 6 Angka: " if lang == "id" else "Enter 6-digit PIN: "
    repeat_prompt = "Ulangi PIN 6 Angka: " if lang == "id" else "Repeat 6-digit PIN: "
    pin = getpass.getpass(first_prompt)
    if len(pin) != 6 or not pin.isdigit():
        raise ValueError("PIN harus tepat 6 digit angka." if lang == "id" else "PIN must contain exactly 6 digits.")
    if confirm and pin != getpass.getpass(repeat_prompt):
        raise ValueError("PIN yang dimasukkan tidak cocok." if lang == "id" else "PIN entries do not match.")
    return pin


def parse_quanta(value: str) -> int:
    cleaned = value.strip().replace(",", ".")
    if not cleaned or cleaned.count(".") > 1 or not all(char.isdigit() or char == "." for char in cleaned):
        raise ValueError("Amount must be a SCY number, such as 0.5 or 10.")
    return int(round(float(cleaned) * QUANTA_PER_SCY))


def format_scy(quanta: int) -> str:
    return f"{quanta // QUANTA_PER_SCY}.{quanta % QUANTA_PER_SCY:08d}"


def print_new_receipt(account: str, lang: str, registered: bool) -> None:
    print("=" * 50)
    if lang == "id":
        print("              DOMPET SCYTALE BERHASIL DIBUAT")
        print(f"Nomor Rekening Anda : {account}")
        status = "Aktif & Terdaftar" if registered else "Lokal (belum tersinkron)"
        print(f"Status              : {status}")
        print("Simpan PIN Anda baik-baik. PIN digunakan untuk setiap transfer.")
    else:
        print("              SCYTALE WALLET CREATED")
        print(f"Account Number : {account}")
        status = "Active & Registered" if registered else "Local (not synchronized)"
        print(f"Status         : {status}")
        print("Keep your PIN safe. It is required for all transfers.")
    print("=" * 50)


def command_new(args: argparse.Namespace) -> int:
    lang = language(args, load_config())
    try:
        pin = prompt_pin(True, lang)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    values = ["new", "--pin", pin]
    if args.force:
        values.append("--force")
    if args.wallet_file:
        values.extend(["--file", str(wallet_path(args))])
    code, output = run_rust(args, "wallet", *values)
    if code:
        print(human_error(output, lang), file=sys.stderr)
        return code
    account = find_account(output)
    registered = bool(account)
    if not account:
        account = "Belum terdaftar" if lang == "id" else "Not registered"
    print_new_receipt(account, lang, registered)
    return 0


def command_balance(args: argparse.Namespace) -> int:
    lang = language(args, load_config())
    code, output = run_rust(args, "balance")
    if code:
        print(human_error(output, lang), file=sys.stderr)
        return code
    quanta = 0
    for line in output.splitlines():
        if "quanta" in line.lower():
            digits = "".join(char for char in line.lower().split("quanta", 1)[0] if char.isdigit())
            if digits:
                quanta = int(digits)
                break
    account = account_from_wallet(wallet_path(args))
    print("=" * 50)
    if lang == "id":
        print("                 SALDO REKENING")
        print(f"Nomor Rekening : {account}")
        print(f"Saldo Aktif    : {format_scy(quanta)} SCY")
        print("Status Node    : Terhubung")
    else:
        print("                 ACCOUNT BALANCE")
        print(f"Account Number : {account}")
        print(f"Active Balance : {format_scy(quanta)} SCY")
        print("Node Status    : Connected")
    print("=" * 50)
    return 0


def command_send(args: argparse.Namespace) -> int:
    lang = language(args, load_config())
    target = args.to or input("Rekening Tujuan / Destination account: ").strip()
    if not (target.startswith("SCY-") or target.startswith("scy1")):
        print("Invalid destination account format.", file=sys.stderr)
        return 2
    amount_text = args.amount or input("Jumlah Koin / Amount in SCY: ").strip()
    try:
        amount = parse_quanta(amount_text)
        pin = getpass.getpass("Masukkan PIN 6 Angka untuk konfirmasi: " if lang == "id" else "Enter 6-digit PIN to confirm: ")
        if len(pin) != 6 or not pin.isdigit():
            raise ValueError("PIN must contain exactly 6 digits.")
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    code, output = run_rust(args, "transfer-p2pkh", "--to", target, "--amount", str(amount), "--pin", pin)
    if code:
        print(human_error(output, lang), file=sys.stderr)
        return code
    txid = next((line.split(":", 1)[-1].strip() for line in output.splitlines() if "transaction id" in line.lower()), "N/A")
    print("=" * 50)
    print("              TRANSFER SUCCESSFUL" if lang == "en" else "              TRANSFER SCYTALE BERHASIL")
    print(f"Destination / Tujuan : {target}")
    print(f"Amount / Nominal    : {format_scy(amount)} SCY")
    print(f"TxID                : {txid[:24]}")
    print("=" * 50)
    return 0


def command_status(args: argparse.Namespace) -> int:
    lang = language(args, load_config())
    print(MINI_LOGO)
    code, output = run_rust(args, "status")
    if code:
        print(human_error(output, lang), file=sys.stderr)
        return code
    print(output)
    return 0


def command_config(args: argparse.Namespace) -> int:
    config = load_config()
    if args.lang_value:
        config["language"] = args.lang_value
    if args.network:
        config["network"] = args.network
        config["node_url"] = "http://116.212.72.89:8332" if args.network == "global" else "http://127.0.0.1:8332"
    save_config(config)
    print(json.dumps(config, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="scy", description="Bilingual Scytale user wrapper")
    parser.add_argument("--lang", choices=("en", "id"), help="Response language")
    parser.add_argument("--socket", default=None, help=argparse.SUPPRESS)
    parser.add_argument("--node-url", default=None, help=argparse.SUPPRESS)
    parser.add_argument("--wallet-file", help="Wallet path")
    parser.add_argument("--force", action="store_true", help=argparse.SUPPRESS)
    commands = parser.add_subparsers(dest="command", required=False)

    def runtime(command: argparse.ArgumentParser) -> None:
        command.add_argument("--lang", choices=("en", "id"), default=argparse.SUPPRESS, help=argparse.SUPPRESS)
        command.add_argument("--socket", default=argparse.SUPPRESS, help=argparse.SUPPRESS)
        command.add_argument("--node-url", default=argparse.SUPPRESS, help=argparse.SUPPRESS)
        command.add_argument("--wallet-file", default=argparse.SUPPRESS, help=argparse.SUPPRESS)

    new = commands.add_parser("new", aliases=["create", "buat", "buka"], help="Create a new wallet")
    runtime(new)
    new.add_argument("--force", action="store_true", help=argparse.SUPPRESS)
    new.set_defaults(handler=command_new)
    balance = commands.add_parser("balance", aliases=["bal", "saldo", "cek"], help="Show account balance")
    runtime(balance)
    balance.set_defaults(handler=command_balance)
    send = commands.add_parser("send", aliases=["transfer", "kirim"], help="Send SCY")
    runtime(send)
    send.add_argument("--to", help="Destination SCY account or address")
    send.add_argument("--amount", help="Amount in SCY")
    send.set_defaults(handler=command_send)
    status = commands.add_parser("status", aliases=["info", "kondisi"], help="Show node status")
    runtime(status)
    status.set_defaults(handler=command_status)
    config = commands.add_parser("config", aliases=["setelan"], help="Show or change configuration")
    config.add_argument("--lang", dest="lang_value", choices=("en", "id"))
    config.add_argument("--network", choices=("global", "local"))
    config.set_defaults(handler=command_config)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(sys.argv[1:] if argv is None else argv)
    if args.command is None:
        print(MINI_LOGO)
        build_parser().print_help()
        return 0
    try:
        return args.handler(args)
    except KeyboardInterrupt:
        print("\nCancelled.", file=sys.stderr)
        return 130


if __name__ == "__main__":
    sys.exit(main())