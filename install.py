#!/usr/bin/env python3
"""Interactive bilingual installer for the Scytale CLI ecosystem."""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent
PYTHON = ROOT / "apps" / "scytale-py" / ".venv" / "bin" / "python"
WRAPPER = ROOT / "apps" / "scytale-py" / "scy.py"
RUST_BINARY = ROOT / "target" / "release" / "scytale-cli"
CONFIG = Path.home() / ".scytale" / "config.json"
GLOBAL_BIN = Path.home() / ".local" / "bin"
LOCAL_BIN = ROOT / "bin"
VENV = ROOT / "apps" / "scytale-py" / ".venv"
CYAN = "\033[36m"
GREEN = "\033[32m"
BOLD = "\033[1m"
RESET = "\033[0m"
LOGO = r"""____                  _             _
/ ___|   ___  _   _  _| |_   __ _   | |   ___
\___ \  / __|| | | ||_   _| / _` |  | |  / _ \
 ___) || (__ | |_| |  | |_ | (_| |_ | | |  __/
|____/  \___| \__, |   \__| \__,_(_)|_|  \___|
              |___/   E C O S Y S T E M"""


def print_logo() -> None:
    print(f"{BOLD}{CYAN}{LOGO}{RESET}")
    print(f"{GREEN}Scytale Ecosystem Installer v1.0 • Lightning-fast Layer-1{RESET}")


def ensure_venv() -> None:
    try:
        subprocess.run([str(PYTHON), "--version"], check=True, capture_output=True, text=True)
    except (OSError, subprocess.CalledProcessError):
        print(f"{GREEN}[✓] Bootstrapping isolated Python virtual environment...{RESET}")
        subprocess.run([sys.executable, "-m", "venv", str(VENV)], check=True)


def ask(prompt: str, options: tuple[str, str], default: str = "1") -> str:
    print(prompt)
    print(f"  [1] {options[0]}")
    print(f"  [2] {options[1]}")
    answer = input(f"Select [{default}]: ").strip() or default
    return "2" if answer == "2" else "1"


def installer_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Scytale Ecosystem Installer")
    parser.add_argument("--lang", choices=("en", "id"), help="Wizard language")
    parser.add_argument("--scope", choices=("global", "local"), help="Installation scope")
    parser.add_argument("--network", choices=("global", "local"), help="Default network")
    return parser.parse_args()


def write_executable(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    os.chmod(path, path.stat().st_mode | 0o111)


def install_launcher(destination: Path) -> None:
    write_executable(
        destination,
        "#!/usr/bin/env bash\n"
        "set -eu\n"
        f'exec "{PYTHON}" "{WRAPPER}" "$@"\n',
    )


def install_symlink(destination: Path) -> None:
    if destination.is_symlink() or destination.exists():
        destination.unlink()
    destination.symlink_to(RUST_BINARY)


def main() -> int:
    args = installer_args()
    ensure_venv()
    print_logo()
    language = "1" if args.lang in (None, "en") else "2"
    if args.lang is None:
        language = ask(
            "[?] Select Language / Pilih Bahasa:",
            ("English (Default)", "Bahasa Indonesia"),
        )
    english = language == "1"

    if english:
        scope = "1" if args.scope in (None, "global") else "2"
        if args.scope is None:
            scope = ask(
                "[?] Installation Scope:",
                ("Global (~/.local/bin - run 'scy' anywhere)", "Local (./bin in current workspace)"),
            )
        components = "1"
        if args.lang is None or args.scope is None or args.network is None:
            components = ask(
                "[?] Components:",
                ("CLI Engine & Python Wrapper", "Full (+ Desktop GUI Tauri - Coming Soon)"),
            )
        network = "1" if args.network in (None, "global") else "2"
        if args.network is None:
            network = ask(
                "[?] Default Network:",
                ("Global Testnet VPS (http://116.212.72.89:8332)", "Local Node (http://127.0.0.1:8332)"),
            )
    else:
        scope = "1" if args.scope in (None, "global") else "2"
        if args.scope is None:
            scope = ask(
                "[?] Lingkup Instalasi:",
                ("Global (~/.local/bin - perintah 'scy' aktif di mana saja)", "Lokal (./bin di workspace ini)"),
            )
        components = "1"
        if args.lang is None or args.scope is None or args.network is None:
            components = ask(
                "[?] Komponen:",
                ("Mesin CLI & Wrapper Python", "Lengkap (+ GUI Desktop Tauri - Segera Hadir)"),
            )
        network = "1" if args.network in (None, "global") else "2"
        if args.network is None:
            network = ask(
                "[?] Jaringan Default:",
                ("Jaringan Global VPS (http://116.212.72.89:8332)", "Simpul Lokal (http://127.0.0.1:8332)"),
            )

    config = {
        "language": "en" if english else "id",
        "install_scope": "global" if scope == "1" else "local",
        "network": "global" if network == "1" else "local",
        "node_url": "http://116.212.72.89:8332" if network == "1" else "http://127.0.0.1:8332",
        "socket_path": "/tmp/scytale.sock",
    }
    CONFIG.parent.mkdir(parents=True, exist_ok=True)
    CONFIG.write_text(json.dumps(config, indent=2) + "\n", encoding="utf-8")

    destination = GLOBAL_BIN if scope == "1" else LOCAL_BIN
    install_launcher(destination / "scy")
    install_symlink(destination / "scytale-cli")
    os.chmod(__file__, Path(__file__).stat().st_mode | 0o111)

    print()
    if english:
        print(f"Installed {components == '2' and 'full' or 'CLI'} components in {config['install_scope']} mode.")
        print(f"Configuration: {CONFIG}")
        if scope == "1" and str(GLOBAL_BIN) not in os.environ.get("PATH", "").split(os.pathsep):
            print(f"Add this directory to your PATH: export PATH=\"{GLOBAL_BIN}:$PATH\"")
            print(f"Persist it with: echo 'export PATH=\"{GLOBAL_BIN}:$PATH\"' >> ~/.bashrc")
        print("Run: scy --help")
    else:
        print(f"Komponen {config['install_scope']} terpasang di {destination}.")
        print(f"Konfigurasi: {CONFIG}")
        if scope == "1" and str(GLOBAL_BIN) not in os.environ.get("PATH", "").split(os.pathsep):
            print(f"Tambahkan ke PATH: export PATH=\"{GLOBAL_BIN}:$PATH\"")
            print(f"Simpan dengan: echo 'export PATH=\"{GLOBAL_BIN}:$PATH\"' >> ~/.bashrc")
        print("Jalankan: scy --help")
    return 0


if __name__ == "__main__":
    sys.exit(main())
