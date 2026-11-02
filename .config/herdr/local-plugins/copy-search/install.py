#!/usr/bin/env python3
"""Build and install the dotfiles copy-mode plugin on the current machine."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


SOURCE = Path(__file__).resolve().parent


def xdg_path(name, fallback):
    return Path(os.environ.get(name) or Path.home() / fallback).expanduser()


def package(binary, destination):
    if not binary.is_file():
        raise SystemExit(f"Release binary does not exist: {binary}")
    if destination == SOURCE:
        raise SystemExit("The install destination must differ from the source directory")
    manifest = (SOURCE / "herdr-plugin.toml").read_text()
    manifest = manifest.replace(
        '[[build]]\ncommand = ["cargo", "build", "--release", "--locked"]\n', ""
    ).replace(
        "$HERDR_PLUGIN_ROOT/target/release/herdr-copy-search",
        "$HERDR_PLUGIN_ROOT/herdr-copy-search",
    )
    if "[[build]]" in manifest or "/target/release/" in manifest:
        raise SystemExit("Unexpected source manifest; refusing to install a build-dependent runtime")
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".install-", dir=destination) as staging:
        staging = Path(staging)
        shutil.copy2(binary, staging / "herdr-copy-search")
        (staging / "herdr-copy-search").chmod(0o755)
        (staging / "herdr-plugin.toml").write_text(manifest)
        for name in ("LICENSE", "README.md", "UPSTREAM"):
            shutil.copy2(SOURCE / name, staging / name)
        for file in staging.iterdir():
            file.replace(destination / file.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, help="Reuse a release binary for this OS and architecture")
    parser.add_argument("--herdr", help="Herdr executable (defaults to PATH, then ~/.local/bin/herdr)")
    parser.add_argument("--no-link", action="store_true", help="Package only; do not contact Herdr")
    parser.add_argument("--destination", type=Path,
                        default=xdg_path("XDG_DATA_HOME", ".local/share") / "herdr/copy-search")
    parser.add_argument("--build-dir", type=Path,
                        default=xdg_path("XDG_CACHE_HOME", ".cache") / "herdr-copy-search/target")
    args = parser.parse_args()
    herdr = None
    if not args.no_link:
        candidate = args.herdr or shutil.which("herdr") or str(Path.home() / ".local/bin/herdr")
        herdr = shutil.which(os.path.expanduser(candidate))
        if not herdr:
            parser.error("Herdr is missing; install it first, pass --herdr PATH, or use --no-link")
    if args.binary:
        binary = args.binary.expanduser().resolve()
    else:
        cargo = shutil.which("cargo") or shutil.which(str(Path.home() / ".cargo/bin/cargo"))
        if not cargo:
            parser.error("Cargo is missing; install Rust or pass --binary PATH")
        build_dir = args.build_dir.expanduser().resolve()
        subprocess.run([cargo, "build", "--release", "--locked", "--manifest-path",
                        str(SOURCE / "Cargo.toml"), "--target-dir", str(build_dir)],
                       cwd=SOURCE, check=True)
        binary = build_dir / "release/herdr-copy-search"
    destination = args.destination.expanduser().resolve()
    package(binary, destination)
    if herdr:
        subprocess.run([herdr, "plugin", "link", str(destination)], check=True)
        subprocess.run([herdr, "server", "reload-config"], check=True)
    print(f"Installed: {destination}")
    if herdr:
        print("Linked copy-search and reloaded config. No server restart was requested.")


if __name__ == "__main__":
    main()
