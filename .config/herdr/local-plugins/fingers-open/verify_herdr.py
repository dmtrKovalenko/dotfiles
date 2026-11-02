#!/usr/bin/env python3
"""Exercise a real hint selection in an isolated, disposable Herdr server."""

from pathlib import Path
import argparse
import json
import os
import shlex
import subprocess
import tempfile
import time

from test_support import Herdr, state_dir, nvim_call


def until(function, timeout=12):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            value = function()
            if value:
                return value
        except Exception as error:
            last = error
        time.sleep(0.05)
    raise RuntimeError("Timed out: " + str(last))


def verify(herdr, nvim, fingers, url=None):
    package = Path(__file__).resolve().parent
    root = Path(tempfile.mkdtemp(prefix="hfo-live-", dir="/tmp")).resolve()
    print("Verification files:", root, flush=True)
    config = root / "config/herdr"
    config.mkdir(parents=True)
    (config / "config.toml").write_text('onboarding = false\n[terminal]\ndefault_shell = "/bin/sh"\n[ui.sound]\nenabled = false\n')
    fingers_config = config / "plugins/config/nathan-poncet.herdr-fingers"
    fingers_config.mkdir(parents=True)
    project = root / "project"
    (project / "src").mkdir(parents=True)
    (project / "src/main.rs").write_text("first\nsecond line\nthird\n")
    subprocess.run(["git", "init", "-q", str(project)], check=True)
    init = root / "nvim-init.lua"
    init.write_text('package.loaded.herdr_fingers = dofile(' + json.dumps(str(package / "herdr_fingers.lua")) + ')\n'
                    'require("herdr_fingers").setup()\n')
    oil = Path.home() / ".local/share/nvim/lazy/oil.nvim"
    if oil.is_dir():
        with init.open("a") as file:
            file.write('vim.opt.rtp:prepend(' + json.dumps(str(oil)) + ')\nrequire("oil").setup({columns={}})\n')
    env = dict(os.environ, XDG_CONFIG_HOME=str(root / "config"), XDG_STATE_HOME=str(root / "state"),
               XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_HOME=str(root / "data"), HERDR_CONFIG_PATH=str(config / "config.toml"),
               HERDR_SOCKET_PATH=str(config / "herdr.sock"),
               HERDR_FINGERS_OPEN_STATE=str(root / "registry"), HERDR_DISABLE_SOUND="1", HERDR_LOG="warn")
    if url:
        (root / "registry").mkdir(mode=0o700)
        (root / "registry/browsers").symlink_to(state_dir() / "browsers")
        env["HERDR_FINGERS_BROWSER_REMOTE"] = "1"
    env["PATH"] = os.pathsep.join(dict.fromkeys([str(Path(nvim).parent), str(Path(herdr).parent),
                                                "/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin",
                                                env.get("PATH", "")]))
    nvim_config = root / "config/nvim"
    nvim_config.mkdir()
    (nvim_config / "init.lua").write_text("")
    host = Herdr(str(config / "herdr.sock"))

    def run_pane(pane_id, command):
        result = subprocess.run([herdr, "pane", "run", pane_id, command], env=env,
                                capture_output=True, text=True, timeout=8)
        if result.returncode:
            raise RuntimeError(result.stderr)

    with (root / "server.log").open("wb") as log:
        server = subprocess.Popen([herdr, "server"], env=env, stdin=subprocess.DEVNULL,
                                  stdout=log, stderr=log, start_new_session=True)
        try:
            until(lambda: (config / "herdr.sock").exists())
            subprocess.run([fingers, "install", "--role", "remote", "--herdr", herdr], env=env, check=True)
            host.call("workspace.create", cwd=str(project), label="Fingers Open verification", focus=True)
            source = host.call("pane.current")["pane"]
            workspace = source["workspace_id"]
            host.call("tab.create", workspace_id=workspace, label="Neovim", cwd=str(project), focus=True)
            editor_pane = host.call("pane.current")["pane"]
            run_pane(editor_pane["pane_id"], shlex.join([nvim, "-n", "-u", str(init), "-i", "NONE"]))
            registrations = until(lambda: list((root / "registry/nvim").glob("*.json")))
            editor = json.loads(registrations[0].read_text())
            base_panes = {p["pane_id"] for p in host.call("pane.list", workspace_id=workspace)["panes"]}
            subprocess.run([fingers, "register-existing"], env=env, check=True)
            host.call("pane.focus", pane_id=source["pane_id"])
            run_pane(source["pane_id"], "printf '\\033[2J\\033[Hmain.rs:2:4\\r\\n'; sleep 30")
            time.sleep(0.3)
            host.call("plugin.action.invoke", plugin_id="nathan-poncet.herdr-fingers", action_id="start")

            def overlay():
                panes = host.call("pane.list", workspace_id=workspace)["panes"]
                return next((p for p in panes if p["pane_id"] not in base_panes), None)

            hint = until(overlay)
            time.sleep(0.2)
            host.call("pane.send_keys", pane_id=hint["pane_id"], keys=["a"])
            until(lambda: nvim_call(editor, "status")["current_file"] == str(project / "src/main.rs"))
            until(lambda: host.call("pane.current")["pane"]["pane_id"] == editor_pane["pane_id"])
            expression = "luaeval('vim.json.encode(vim.api.nvim_win_get_cursor(0))')"
            cursor = json.loads(subprocess.check_output([nvim, "--server", editor["socket"], "--remote-expr", expression], text=True))
            assert cursor == [2, 3], cursor
            until(lambda: not overlay())
            print("PASS: a real Fingers hint opened the file at line 2, column 4 and focused the existing Neovim tab.", flush=True)

            def select_hint(text, chooser_keys=None):
                host.call("pane.send_keys", pane_id=source["pane_id"], keys=["ctrl+c"])
                host.call("pane.focus", pane_id=source["pane_id"])
                time.sleep(0.15)
                run_pane(source["pane_id"], "printf '\\033[2J\\033[H%s\\r\\n' " + shlex.quote(text) + "; sleep 30")
                time.sleep(0.3)
                host.call("plugin.action.invoke", plugin_id="nathan-poncet.herdr-fingers", action_id="start")
                hint = until(overlay)
                time.sleep(0.2)
                host.call("pane.send_keys", pane_id=hint["pane_id"], keys=["a"])
                if chooser_keys:
                    until(lambda: "Choose file" in host.call("pane.read", pane_id=hint["pane_id"], source="visible")["read"]["text"])
                    host.call("pane.send_keys", pane_id=hint["pane_id"], keys=chooser_keys)
                until(lambda: not overlay(), timeout=20)

            if package.is_relative_to(Path.home()):
                relative = (package / "herdr_fingers.lua").relative_to(Path.home())
                for prefix in ("$HOME", "${HOME}"):
                    reference = prefix + "/" + str(relative) + ":2:4"
                    select_hint(reference)
                    action = json.loads((root / "registry/last-action.json").read_text())
                    assert action["selection"] == reference, action
                    assert action["route"] == "opened", action
                    until(lambda: nvim_call(editor, "status")["current_file"] == str(package / "herdr_fingers.lua"))
                    until(lambda: host.call("pane.current")["pane"]["pane_id"] == editor_pane["pane_id"])
                    cursor = json.loads(subprocess.check_output([nvim, "--server", editor["socket"], "--remote-expr", expression], text=True))
                    assert cursor == [2, 3], cursor
                print("PASS: real $HOME and ${HOME} hints preserved the full path and opened line 2, column 4.", flush=True)
            if oil.is_dir():
                select_hint(str(project / "src") + "/")
                until(lambda: nvim_call(editor, "status")["current_file"] == "oil://" + str(project / "src") + "/")
                until(lambda: host.call("pane.current")["pane"]["pane_id"] == editor_pane["pane_id"])
                print("PASS: a real directory hint opened Oil and focused the existing Neovim tab.", flush=True)
            (project / "other").mkdir()
            (project / "other/main.rs").write_text("other file\nsecond\n")
            before = nvim_call(editor, "status")["current_file"]
            select_hint("main.rs:2:4", ["escape"])
            assert nvim_call(editor, "status")["current_file"] == before
            select_hint("main.rs:2:4", ["j", "enter"])
            until(lambda: nvim_call(editor, "status")["current_file"] == str(project / "src/main.rs"))
            print("PASS: ambiguous file hints allow cancellation and explicit selection.", flush=True)

            subprocess.run([herdr, "pane", "split", editor_pane["pane_id"], "--direction", "down", "--cwd", str(project), "--focus"],
                           env=env, check=True, capture_output=True)
            source = host.call("pane.current")["pane"]
            base_panes.add(source["pane_id"])
            until(lambda: host.call("pane.read", pane_id=source["pane_id"], source="visible")["read"]["text"].strip())
            select_hint("src/main.rs:3:2")
            until(lambda: host.call("pane.current")["pane"]["pane_id"] == editor_pane["pane_id"])
            cursor = json.loads(subprocess.check_output([nvim, "--server", editor["socket"], "--remote-expr", expression], text=True))
            assert cursor == [3, 1], cursor
            print("PASS: a hint in the same tab restores the split layout before focusing Neovim.", flush=True)
            if url:
                select_hint(url)
                action = json.loads((root / "registry/last-action.json").read_text())
                assert action["route"] == "browser", action
                print("PASS: a real URL hint completed the browser request through the connected bridge.", flush=True)
                (root / "registry/browsers").unlink()
                select_hint(url)
                action = json.loads((root / "registry/last-action.json").read_text())
                assert action["route"] == "copied-browser-failed", action
                assert "disconnected" in action["error"], action
                print("PASS: a real URL hint with no bridge copied the link and closed without an error overlay.", flush=True)
        except Exception:
            print("Server log:", root / "server.log", flush=True)
            try:
                print("Plugin logs:", json.dumps(host.call("plugin.log.list", plugin_id="nathan-poncet.herdr-fingers"))[:3000], flush=True)
                for pane in host.call("pane.list")["panes"]:
                    text = host.call("pane.read", pane_id=pane["pane_id"], source="visible")["read"]["text"]
                    print("Test pane", pane["pane_id"], repr(text[-1500:]), flush=True)
            except Exception as error:
                print("Unable to inspect test panes:", error, flush=True)
            raise
        finally:
            try:
                host.call("server.stop")
            except Exception:
                pass
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.terminate()
                server.wait(timeout=5)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--herdr", required=True)
    parser.add_argument("--nvim", required=True)
    parser.add_argument("--fingers", required=True)
    parser.add_argument("--url", help="Also select a URL through the live client browser bridge")
    options = parser.parse_args()
    verify(options.herdr, options.nvim, options.fingers, options.url)
