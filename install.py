#!/usr/bin/env python3
"""Interactive bilingual installer for the Scytale CLI ecosystem."""

import argparse
import json
import os
import subprocess
import sys
import tempfile
from urllib.parse import urlparse
from pathlib import Path


ROOT = Path(__file__).resolve().parent
PYTHON = ROOT / "apps" / "scytale-py" / ".venv" / "bin" / "python"
WRAPPER = ROOT / "apps" / "scytale-py" / "scy.py"
RUST_BINARY = ROOT / "target" / "release" / "scytale-cli"
CONFIG = Path.home() / ".scytale" / "config.json"
GLOBAL_BIN = Path.home() / ".local" / "bin"
LOCAL_BIN = ROOT / "bin"
VENV = ROOT / "apps" / "scytale-py" / ".venv"
COMMAND_TIMEOUT_SECONDS = 900
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
    if not PYTHON.is_file() or not os.access(PYTHON, os.X_OK):
        raise RuntimeError(f"Python virtual environment is incomplete: {PYTHON}")


def ensure_release_binary() -> None:
    if RUST_BINARY.is_file() and os.access(RUST_BINARY, os.X_OK):
        return
    print(f"{GREEN}[✓] Building release CLI binary...{RESET}")
    subprocess.run(
        ["cargo", "build", "--release", "-p", "scytale-cli"],
        cwd=ROOT,
        check=True,
        timeout=COMMAND_TIMEOUT_SECONDS,
    )
    if not RUST_BINARY.is_file() or not os.access(RUST_BINARY, os.X_OK):
        raise RuntimeError(f"Release CLI binary was not produced: {RUST_BINARY}")


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
    parser.add_argument("--node-url", help="Node HTTP gateway URL (required for global network)")
    parser.add_argument("--force", action="store_true", help="Replace existing launchers")
    parser.add_argument("--no-build", action="store_true", help="Fail if the release CLI is not built")
    return parser.parse_args()


def validate_node_url(value: str) -> str:
    parsed = urlparse(value.strip())
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        raise ValueError("node URL must be a complete http:// or https:// URL")
    return value.strip().rstrip("/")


def choose_network(args: argparse.Namespace, english: bool) -> tuple[str, str]:
    if args.network:
        network = args.network
    elif not sys.stdin.isatty():
        network = "local"
    else:
        network = "local" if ask(
            "[?] Default network:",
            ("Local node (http://127.0.0.1:8332)", "Configured public node (HTTPS URL required)"),
            default="1",
        ) == "1" else "global"

    if args.node_url:
        node_url = validate_node_url(args.node_url)
    elif network == "local":
        node_url = "http://127.0.0.1:8332"
    else:
        raise ValueError(
            "global network requires --node-url with the HTTPS URL of your deployment"
        )
    if network == "global" and not node_url.startswith("https://"):
        raise ValueError("global network requires an HTTPS node URL")
    return network, node_url


def choose_scope(args: argparse.Namespace) -> str:
    if args.scope:
        return args.scope
    if not sys.stdin.isatty():
        return "local"
    return "global" if ask(
        "[?] Installation scope:",
        ("Local (./bin)", "Global (~/.local/bin)"),
        default="1",
    ) == "2" else "local"


def write_executable(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        os.fchmod(descriptor, 0o755)
        with os.fdopen(descriptor, "w", encoding="utf-8") as file:
            descriptor = -1
            file.write(content)
            file.flush()
            os.fsync(file.fileno())
        os.replace(temporary_name, path)
        os.chmod(path, 0o755)
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        Path(temporary_name).unlink(missing_ok=True)
        raise


def install_launcher(destination: Path, force: bool) -> None:
    if destination.exists() or destination.is_symlink():
        if not force:
            raise RuntimeError(f"Refusing to replace existing launcher: {destination} (use --force)")
    write_executable(
        destination,
        "#!/usr/bin/env bash\n"
        "set -eu\n"
        f'exec "{PYTHON}" "{WRAPPER}" "$@"\n',
    )


def install_symlink(destination: Path, force: bool) -> None:
    if destination.is_symlink() or destination.exists():
        if not force:
            raise RuntimeError(f"Refusing to replace existing binary link: {destination} (use --force)")
        destination.unlink()
    destination.symlink_to(RUST_BINARY)


def write_config(config: dict[str, str]) -> None:
    CONFIG.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix="config.", suffix=".tmp", dir=CONFIG.parent
    )
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as file:
            descriptor = -1
            json.dump(config, file, indent=2, sort_keys=True)
            file.write("\n")
            file.flush()
            os.fsync(file.fileno())
        os.replace(temporary_name, CONFIG)
        os.chmod(CONFIG, 0o600)
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        Path(temporary_name).unlink(missing_ok=True)
        raise


def main() -> int:
    args = installer_args()
    english = args.lang != "id"
    print_logo()
    try:
        scope = choose_scope(args)
        network, node_url = choose_network(args, english)
        ensure_venv()
        if args.no_build:
            if not RUST_BINARY.is_file() or not os.access(RUST_BINARY, os.X_OK):
                raise RuntimeError(f"Release CLI binary not found: {RUST_BINARY}")
        else:
            ensure_release_binary()

        destination = GLOBAL_BIN if scope == "global" else LOCAL_BIN
        launcher = destination / "scy"
        binary_link = destination / "scytale-cli"
        if not args.force and (launcher.exists() or launcher.is_symlink()):
            raise RuntimeError(f"Launcher already exists: {launcher} (use --force)")
        if not args.force and (binary_link.exists() or binary_link.is_symlink()):
            raise RuntimeError(f"Binary link already exists: {binary_link} (use --force)")

        config = {
            "language": "en" if english else "id",
            "install_scope": scope,
            "network": network,
            "node_url": node_url,
            "socket_path": "/tmp/scytale.sock",
        }
        write_config(config)
        install_launcher(launcher, args.force)
        install_symlink(binary_link, args.force)
        os.chmod(__file__, Path(__file__).stat().st_mode | 0o111)
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        print(f"Installation failed: {error}", file=sys.stderr)
        return 1

    print()
    if english:
        print(f"Installed CLI and Python wrapper in {scope} mode.")
        print(f"Node: {node_url}")
        print(f"Configuration: {CONFIG}")
        if scope == "global" and str(GLOBAL_BIN) not in os.environ.get("PATH", "").split(os.pathsep):
            print(f"Add this directory to PATH: export PATH=\"{GLOBAL_BIN}:$PATH\"")
        print("Run: scy --help")
    else:
        print(f"CLI dan wrapper Python terpasang dalam mode {scope}.")
        print(f"Node: {node_url}")
        print(f"Konfigurasi: {CONFIG}")
        if scope == "global" and str(GLOBAL_BIN) not in os.environ.get("PATH", "").split(os.pathsep):
            print(f"Tambahkan direktori ini ke PATH: export PATH=\"{GLOBAL_BIN}:$PATH\"")
        print("Jalankan: scy --help")
    return 0


if __name__ == "__main__":
    sys.exit(main())
