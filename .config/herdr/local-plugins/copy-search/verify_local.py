#!/usr/bin/env python3
"""Verify the local prototype with a disposable Herdr server and TTY client."""

import argparse
import base64
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import shlex
import signal
import shutil
import socket
import struct
import subprocess
import tempfile
import termios
import threading
import time
import uuid


def until(check, timeout=15):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            result = check()
            if result:
                return result
        except (OSError, RuntimeError) as error:
            last = error
        time.sleep(0.05)
    raise RuntimeError(f"Timed out: {last}")


def call(path, method, **params):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(5)
        connection.connect(str(path))
        request = {"id": str(uuid.uuid4()), "method": method, "params": params}
        connection.sendall(json.dumps(request).encode() + b"\n")
        with connection.makefile("rb") as stream:
            response = json.loads(stream.readline())
    if "error" in response:
        raise RuntimeError(str(response["error"]))
    return response["result"]


def styled_lines(text):
    """Decode SGR from pane.read so assertions inspect actual PTY colors."""
    lines = [[]]
    fg = bg = None
    bold = False
    for part in re.split(r"(\x1b\[[0-9;]*m)", text):
        if part.startswith("\x1b["):
            codes = [int(n or 0) for n in part[2:-1].split(";")]
            i = 0
            while i < len(codes):
                code = codes[i]
                if code == 0:
                    fg = bg = None
                    bold = False
                elif code == 1:
                    bold = True
                elif code == 22:
                    bold = False
                elif code in (38, 48) and i + 2 < len(codes):
                    length = 3 if codes[i + 1] == 2 else 1
                    color = tuple(codes[i + 1:i + 2 + length])
                    if code == 38:
                        fg = color
                    else:
                        bg = color
                    i += 1 + length
                elif 30 <= code <= 37:
                    fg = (5, code - 30)
                elif 90 <= code <= 97:
                    fg = (5, code - 90 + 8)
                elif 40 <= code <= 47:
                    bg = (5, code - 40)
                elif code == 39:
                    fg = None
                elif code == 49:
                    bg = None
                i += 1
        else:
            for char in part:
                if char == "\n":
                    lines.append([])
                elif char != "\r":
                    lines[-1].append((char, fg, bg, bold))
    return lines


def token_cells(text, token):
    for line in styled_lines(text):
        plain = "".join(cell[0] for cell in line)
        index = plain.find(token)
        if index >= 0:
            return line[index:index + len(token)]
    raise RuntimeError(f"Missing token {token!r}")


def verify(herdr, plugin=None, server_binary=None, rows=36, cols=120, kitty=False,
           config_file=None):
    package = Path(__file__).resolve().parent
    root = Path(tempfile.mkdtemp(prefix="copy-search-test-", dir="/tmp")).resolve()
    print(f"Verification files: {root}", flush=True)
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
description = "Color-preserving copy mode"
''')
    if config_file:
        (config / "config.toml").write_text(config_file.read_text())
    env = {key: value for key, value in os.environ.items() if not key.startswith("HERDR_")}
    env.pop("NO_COLOR", None)
    env.update(XDG_CONFIG_HOME=str(root / "config"), XDG_STATE_HOME=str(root / "state"),
               XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_HOME=str(root / "data"),
               HERDR_CONFIG_PATH=str(config / "config.toml"), HERDR_SOCKET_PATH=str(config / "herdr.sock"),
               HERDR_DISABLE_SOUND="1", HERDR_LOG="warn", TERM="xterm-256color", COLORTERM="truecolor",
               SSH_TTY="/dev/test-osc52", PATH=f"{Path(herdr).parent}:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin")
    fingers = Path.home() / ".local/share/herdr/fingers-open/runtime/herdr-fingers"
    installed_fingers = root / "data/herdr/fingers-open/runtime/herdr-fingers"
    installed_fingers.parent.mkdir(parents=True)
    installed_fingers.symlink_to(fingers)
    env.update(HERDR_FINGERS_OPEN_STATE=str(root / "fingers-state"), HERDR_FINGERS_BROWSER_REMOTE="1")
    api_path = config / "herdr.sock"

    def api(method, **params):
        return call(api_path, method, **params)

    def cli(*args):
        return subprocess.run([herdr, *args], env=env, capture_output=True, text=True, check=True, timeout=15)

    def read(pane, ansi=False):
        return api("pane.read", pane_id=pane, source="visible", format="ansi" if ansi else "text",
                   strip_ansi=not ansi)["read"]["text"]

    fixture = str(package / "fixtures/sample.ansi")
    url = "https://example.com/path?q=';$HOME&check=gx"
    unresolved = "unresolved_$HOME;literal"
    extra = f"file src/probe.rs:2:4\nurl <{url}>\nfallback {unresolved}\n"
    (root / "src").mkdir()
    (root / "src/probe.rs").write_text("first\nsecond line\nthird\n")
    publisher = root / "publish.py"
    publisher.write_text("import pathlib, time\n"
                         f"print('\\x1b[?1049h\\x1b[2J\\x1b[H' + pathlib.Path({fixture!r}).read_text() + {extra!r}, end='', flush=True)\n"
                         "for i in range(600):\n"
                         "    print(f'\\x1b[15;1Hlive output {i}', end='', flush=True)\n"
                         "    time.sleep(0.1)\n")
    client = None
    master = None
    stopped = threading.Event()
    captured = bytearray()
    editor = None
    relay = None

    def drain():
        probe = bytearray()
        replied = False
        while not stopped.is_set():
            ready, _, _ = select.select([master], [], [], 0.1)
            if ready:
                try:
                    data = os.read(master, 65536)
                except OSError:
                    return
                if not data:
                    return
                captured.extend(data)
                if kitty and not replied:
                    probe.extend(data)
                    if b"\x1b[?u\x1b[c" in probe:
                        os.write(master, b"\x1b[?1016;2$y\x1b[?7u\x1b[?62;c")
                        replied = True

    with (root / "server.log").open("wb") as log:
        server = subprocess.Popen([server_binary or herdr, "server"], env=env, stdin=subprocess.DEVNULL,
                                  stdout=log, stderr=log, start_new_session=True)
        try:
            until(lambda: api_path.exists())
            cli("plugin", "link", str(plugin or package))
            created = api("workspace.create", cwd=str(root), label="Copy plugin verification", focus=True)
            source = created["root_pane"]["pane_id"]
            workspace = created["workspace"]["workspace_id"]
            base = {p["pane_id"] for p in api("pane.list", workspace_id=workspace)["panes"]}
            client, master = pty.fork()
            if client == 0:
                os.execve(herdr, [herdr], env)
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
            reader = threading.Thread(target=drain, daemon=True)
            reader.start()
            until(lambda: len(captured) > 100)
            time.sleep(1)
            cli("pane", "run", source, shlex.join(["/usr/bin/python3", str(publisher)]))
            until(lambda: "rgb-blue" in read(source))
            source_ansi = read(source, True)
            (root / "source.ansi").write_text(source_ansi)
            def open_copy():
                for source_kind, fmt in (("recent_unwrapped", "text"), ("recent", "ansi")):
                    snapshot = api("pane.read", pane_id=source, source=source_kind, format=fmt)["read"]["text"]
                    (root / f"source-{source_kind}.{fmt}").write_text(snapshot)
                if kitty:
                    os.write(master, b"\x1b[32;5u\x1b[32;5:3u\x1b[91;1u\x1b[91;1:3u")
                else:
                    os.write(master, b"\x00[")
                pane = until(overlay)
                until(lambda: "[copy]" in read(pane))
                return pane

            def overlay():
                return next((p["pane_id"] for p in api("pane.list", workspace_id=workspace)["panes"]
                             if p["pane_id"] not in base), None)

            pane = open_copy()
            until(lambda: "[copy]" in read(pane))
            until(lambda: b"[copy]" in captured)

            def keys(*values):
                encoded = {"enter": b"\r", "escape": b"\x1b"}
                for value in values:
                    if kitty:
                        code = {"enter": 13, "escape": 27}.get(value)
                        if code is None:
                            code = ord(value)
                        os.write(master, f"\x1b[{code};1u\x1b[{code};1:3u".encode())
                    else:
                        os.write(master, encoded.get(value, value.encode()))
                    time.sleep(0.06)
                time.sleep(0.15)

            keys("/", *list("plain line"), "enter", "j", "v", "j", "$")
            selected = read(pane, True)
            (root / "selected.ansi").write_text(selected)
            for token in ("red", "bold-green", "orange25", "rgb-blue"):
                original = token_cells(source_ansi, token)
                actual = token_cells(selected, token)
                assert [(c[1], c[3]) for c in actual] == [(c[1], c[3]) for c in original], (token, original, actual)
                assert all(c[2] == (2, 60, 64, 71) for c in actual), (token, actual)
            print("PASS: selected ANSI, 256-color, RGB and bold text retains its source styling.", flush=True)
            keys("escape")
            keys("k", "0", "w", "b", "w", "v", "e")
            (root / "word-selected.ansi").write_text(read(pane, True))
            keys("enter")
            expected = b"\x1b]52;c;" + base64.b64encode(b"bold") + b"\x07"
            until(lambda: expected in captured)
            until(lambda: not overlay())
            assert "live output" in read(source)
            print("PASS: w/b/e selected 'bold', Enter closed the overlay, and Herdr forwarded the exact clipboard payload.", flush=True)
            print("PASS: the source kept producing output during selection; no Neovim process was used.", flush=True)

            # Use the real router and bridge relay, capturing only the final
            # browser delivery instead of opening a tab on the user's desktop.
            relay = subprocess.Popen([str(fingers), "browser", "relay", "--client", "a" * 32],
                                     env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True)
            assert json.loads(relay.stdout.readline())["type"] == "ready"
            delivered = []

            def acknowledge_browser():
                line = relay.stdout.readline()
                if not line:
                    return
                message = json.loads(line)
                delivered.append(message["url"])
                relay.stdin.write(json.dumps({"id": message["id"], "ok": True}) + "\n")
                relay.stdin.flush()

            browser = threading.Thread(target=acknowledge_browser, daemon=True)
            browser.start()
            pane = open_copy()
            keys("/", *list("example.com"), "enter", "g", "x")
            until(lambda: not overlay())
            browser.join(timeout=5)
            assert delivered == [url], delivered
            report = root / "fingers-state/last-action.json"
            assert json.loads(report.read_text())["route"] == "browser"
            print("PASS: gx delivered the exact URL through the real Fingers browser bridge.", flush=True)

            pane = open_copy()
            keys("/", *list("unresolved_"), "enter", "g", "x")
            until(lambda: not overlay())
            expected = b"\x1b]52;c;" + base64.b64encode(unresolved.encode()) + b"\x07"
            until(lambda: expected in captured)
            assert json.loads(report.read_text())["route"] == "copied-unresolved"
            print("PASS: gx used Fingers' unresolved-token fallback and preserved literal shell characters.", flush=True)

            # A missing router must show an error and keep navigation working.
            installed_fingers.unlink()
            pane = open_copy()
            keys("/", *list("probe.rs"), "enter", "g", "x")
            until(lambda: "open failed:" in read(pane))
            keys("g", "g", "q")
            until(lambda: not overlay())
            installed_fingers.symlink_to(fingers)

            # Register an isolated real Neovim against a real second pane.
            api("pane.split", target_pane_id=source, direction="right", focus=False)
            editor_pane = until(overlay)
            base.add(editor_pane)
            module = Path.home() / ".config/herdr/local-plugins/fingers-open/herdr_fingers.lua"
            init = root / "nvim-init.lua"
            init.write_text(f"package.loaded.herdr_fingers = dofile({json.dumps(str(module))})\n"
                            "require('herdr_fingers').setup()\n"
                            "vim.api.nvim_buf_set_name(0, 'unsaved.txt')\n"
                            "vim.api.nvim_buf_set_lines(0, 0, -1, false, {'keep my unsaved text'})\n")
            nvim = shutil.which("nvim")
            assert nvim, "Neovim is needed for the file-opening integration test"
            nvim_socket = root / "nvim.sock"
            editor_env = dict(env, HERDR_PANE_ID=editor_pane)
            editor = subprocess.Popen([nvim, "--headless", "-n", "-u", str(init), "-i", "NONE",
                                       "--listen", str(nvim_socket)], env=editor_env, cwd=root,
                                      stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            until(lambda: list((root / "fingers-state/nvim").glob("*.json")))
            api("pane.focus", pane_id=source)
            pane = open_copy()
            keys("/", *list("probe.rs"), "enter", "g", "x")
            until(lambda: not overlay())
            until(lambda: api("pane.current")["pane"]["pane_id"] == editor_pane)
            expression = "luaeval('vim.json.encode({file=vim.api.nvim_buf_get_name(0),cursor=vim.api.nvim_win_get_cursor(0),unsaved=vim.fn.getbufline(\"unsaved.txt\",1),modified=vim.fn.getbufvar(\"unsaved.txt\",\"&modified\")})')"
            state = json.loads(subprocess.check_output([nvim, "--server", str(nvim_socket),
                                                       "--remote-expr", expression], env=env, text=True, timeout=10))
            assert state == {"file": str(root / "src/probe.rs"), "cursor": [2, 3],
                             "unsaved": ["keep my unsaved text"], "modified": 1}, state
            action = json.loads(report.read_text())
            assert action["source_pane"] == source and action["route"] == "opened", action
            print("PASS: gx resolved a relative file from the source pane, opened line 2/column 4 in Neovim, preserved unsaved text, and focused the editor after closing.", flush=True)
            (root / "client.ansi").write_bytes(captured)
        finally:
            for process in (editor, relay):
                if process is not None:
                    process.terminate()
                    process.wait(timeout=5)
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
            stopped.set()
            if master is not None:
                os.close(master)
            (root / "client.ansi").write_bytes(captured)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--herdr", default=str(Path.home() / ".local/bin/herdr"))
    parser.add_argument("--plugin", type=Path)
    parser.add_argument("--server-binary")
    parser.add_argument("--rows", type=int, default=36)
    parser.add_argument("--cols", type=int, default=120)
    parser.add_argument("--kitty", action="store_true")
    parser.add_argument("--config", type=Path)
    args = parser.parse_args()
    verify(args.herdr, args.plugin, args.server_binary, args.rows, args.cols, args.kitty,
           args.config)
