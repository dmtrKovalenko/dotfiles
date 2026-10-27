#!/usr/bin/env python3
"""Check startup latency and source input with 20,000 rows of history."""

import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import shlex
import signal
import struct
import subprocess
import tempfile
import termios
import threading
import time
import tty

from verify_local import call, until


def fixture(mode, state_file):
    tty.setraw(0)
    height = max(1, os.get_terminal_size().lines)
    offset = events = 0
    pending = b""

    def redraw():
        first = 20000 - height - offset
        rows = [f"\x1b[32mhistory-{i:05}\x1b[0m colored output" for i in range(first, first + height)]
        os.write(1, ("\x1b[?2026h\x1b[H" + "\r\n".join(rows) + "\x1b[?2026l").encode())

    if mode == "alternate":
        os.write(1, b"\x1b[?1049h\x1b[?1000h\x1b[?1006h\x1b[2J")
        redraw()
    else:
        os.write(1, "".join(f"\x1b[32mhistory-{i:05}\x1b[0m colored output\r\n" for i in range(20000)).encode())
    state_file.write_text(json.dumps({"events": events, "offset": offset}))
    while True:
        data = os.read(0, 4096)
        if not data:
            return
        pending += data
        with state_file.with_suffix(".input").open("ab") as log:
            log.write(data)
        matches = list(re.finditer(rb"\x1b\[<(64|65);\d+;\d+[Mm]", pending))
        for match in matches:
            events += 1
            offset = max(0, min(20000 - height, offset + (1 if match[1] == b"64" else -1)))
            redraw()
        if matches:
            pending = pending[matches[-1].end():]
            state_file.write_text(json.dumps({"events": events, "offset": offset}))


def verify(plugin, mode, expect_scroll):
    herdr = str(Path.home() / ".local/bin/herdr")
    root = Path(tempfile.mkdtemp(prefix="copy-speed-", dir="/tmp")).resolve()
    config = root / "config/herdr"
    config.mkdir(parents=True)
    (config / "config.toml").write_text('''onboarding = false
[terminal]
default_shell = "/bin/sh"
[ui.sound]
enabled = false
[keys]
prefix = "ctrl+space"
copy_mode = []
[[keys.command]]
key = "prefix+["
type = "plugin_action"
command = "copy-search.copy"
''')
    env = {key: value for key, value in os.environ.items() if not key.startswith("HERDR_")}
    env.update(XDG_CONFIG_HOME=str(root / "config"), XDG_STATE_HOME=str(root / "state"),
               XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_HOME=str(root / "data"),
               HERDR_SOCKET_PATH=str(config / "herdr.sock"), HERDR_CONFIG_PATH=str(config / "config.toml"),
               HERDR_DISABLE_SOUND="1", HERDR_LOG="warn", TERM="xterm-256color", COLORTERM="truecolor")

    def api(method, **params):
        return call(config / "herdr.sock", method, **params)

    def cli(*args):
        return subprocess.run([herdr, *args], env=env, capture_output=True, text=True, check=True, timeout=10)

    def read(pane):
        return api("pane.read", pane_id=pane, source="visible", format="text")["read"]["text"]

    client = master = None
    stop = threading.Event()
    captured = bytearray()

    def drain():
        while not stop.is_set():
            if select.select([master], [], [], 0.1)[0]:
                try:
                    captured.extend(os.read(master, 65536))
                except OSError:
                    return

    with (root / "server.log").open("wb") as log:
        server = subprocess.Popen([herdr, "server"], env=env, stdin=subprocess.DEVNULL,
                                  stdout=log, stderr=log, start_new_session=True)
        try:
            until(lambda: (config / "herdr.sock").exists())
            cli("plugin", "link", str(plugin.resolve()))
            workspace = api("workspace.create", cwd=str(root), label="Startup verification", focus=True)
            source = workspace["root_pane"]["pane_id"]
            workspace_id = workspace["workspace"]["workspace_id"]
            base = {p["pane_id"] for p in api("pane.list", workspace_id=workspace_id)["panes"]}
            client, master = pty.fork()
            if client == 0:
                os.execve(herdr, [herdr], env)
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 140, 0, 0))
            threading.Thread(target=drain, daemon=True).start()
            until(lambda: len(captured) > 100)
            time.sleep(1)
            state = root / "fixture.json"
            cli("pane", "run", source, shlex.join(["/usr/bin/python3", str(Path(__file__).resolve()),
                                                   "--fixture", mode, "--state", str(state)]))
            until(lambda: state.exists())
            until(lambda: "history-19999" in read(source))
            report = api("pane.report_agent", pane_id=source, source="copy-speed-test", agent="codex", state="idle")
            (root / "report.json").write_text(json.dumps(report))
            (root / "pane.json").write_text(json.dumps(api("pane.get", pane_id=source)))
            print(f"Startup test: {root}", flush=True)
            initial = read(source)
            started = time.monotonic()
            os.write(master, b"\x00[")

            if expect_scroll:
                until(lambda: json.loads(state.read_text())["events"] > 0)
                print(f"REPRODUCED: old plugin sent mouse-wheel input to idle Codex after {time.monotonic() - started:.3f}s", flush=True)
                return

            def opened():
                panes = api("pane.list", workspace_id=workspace_id)["panes"]
                overlay = next((p["pane_id"] for p in panes if p["pane_id"] not in base), None)
                return overlay if overlay and "[copy]" in read(overlay) and b"[copy]" in captured else None

            overlay = until(opened, timeout=3)
            elapsed = time.monotonic() - started
            assert elapsed < 1.5, f"slow opening: {elapsed:.3f}s"
            assert json.loads(state.read_text()) == {"events": 0, "offset": 0}
            assert read(source) == initial
            if mode == "normal":
                os.write(master, b"gg")
                until(lambda: "history-19" in read(overlay) and "[1/" in read(overlay))
                assert "history-19999" not in read(overlay), "normal scrollback was reduced to visible-only"
            print(f"PASS: {mode}: 20,000 history rows, opened in {elapsed:.3f}s, zero source scroll events", flush=True)
            (root / "client.ansi").write_bytes(captured)
        finally:
            try:
                cli("server", "stop")
            except (subprocess.SubprocessError, OSError):
                server.terminate()
            server.wait(timeout=8)
            if client:
                try:
                    os.kill(client, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                os.waitpid(client, 0)
            stop.set()
            if master is not None:
                os.close(master)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--plugin", type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument("--expect-scroll", action="store_true")
    parser.add_argument("--fixture", choices=("normal", "alternate"))
    parser.add_argument("--state", type=Path)
    args = parser.parse_args()
    if args.fixture:
        fixture(args.fixture, args.state)
    else:
        for mode in (["alternate"] if args.expect_scroll else ["normal", "alternate"]):
            verify(args.plugin, mode, args.expect_scroll)
