#!/usr/bin/env python3
"""Exercise redraw behavior through a real PTY, without the user's terminal."""

import argparse
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import termios
import time


def drain(master, quiet=0.15, timeout=5):
    output = bytearray()
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], quiet)
        if not ready:
            break
        try:
            data = os.read(master, 65536)
        except OSError:
            break
        if not data:
            break
        output.extend(data)
    return bytes(output)


def verify(binary, mode, probe):
    root = Path(__file__).resolve().parent
    env = {key: value for key, value in os.environ.items() if not key.startswith("HERDR_")}
    env.update(TERM="xterm-256color", COLORTERM="truecolor")
    child, master = pty.fork()
    if child == 0:
        os.execve(str(binary), [str(binary), "--input", str(root / "fixtures/sample.ansi"),
                               "--mode", mode], env)
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 79, 220, 0, 0))
    try:
        initial = drain(master, quiet=0.4)
        assert b"[copy]" in initial or b"[word]" in initial, "plugin did not open"
        os.write(master, b"\x1b[<35;10;10M" * 40)
        hover = drain(master, quiet=0.4)
        os.write(master, b"k" if mode == "copy" else b"a")
        movement = drain(master, quiet=0.4)
        print(f"{mode}: hover emitted {len(hover)} bytes; key redraw emitted {len(movement)} bytes; "
              f"synchronized={movement.startswith(b'\x1b[?2026h') and movement.endswith(b'\x1b[?2026l')}", flush=True)
        if not probe:
            assert not hover, "passive mouse movement repainted the screen"
            assert movement.startswith(b"\x1b[?2026h"), "redraw is not synchronized"
            assert movement.endswith(b"\x1b[?2026l"), "redraw did not finish its synchronized frame"
        os.write(master, b"\x03")
        drain(master)
    finally:
        try:
            os.kill(child, signal.SIGTERM)
        except ProcessLookupError:
            pass
        os.waitpid(child, 0)
        os.close(master)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path(__file__).resolve().parent / "target/release/herdr-copy-search")
    parser.add_argument("--probe", action="store_true")
    args = parser.parse_args()
    for mode in ("copy", "extract"):
        verify(args.binary.resolve(), mode, args.probe)
