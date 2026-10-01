#!/opt/homebrew/bin/python3
"""Focus an app through Rift's display/workspace model, or launch it."""
import json
import logging
from logging.handlers import RotatingFileHandler
from pathlib import Path
import subprocess
import sys
import time

CLI = "/opt/homebrew/bin/rift-cli"
LOG = logging.getLogger("rift.focus-app")

def configure_logging():
    directory = Path.home() / ".cache" / "rift"
    directory.mkdir(parents=True, exist_ok=True)
    handler = RotatingFileHandler(
        directory / "focus-app.log", maxBytes=262144, backupCount=2
    )
    handler.setFormatter(logging.Formatter(
        "%(asctime)s pid=%(process)d %(levelname)s %(message)s"
    ))
    LOG.addHandler(handler)
    LOG.setLevel(logging.INFO)

def rift(*args):
    return subprocess.run(
        [CLI, *args], capture_output=True, text=True, timeout=3, check=True
    ).stdout

def find_window(bundle_id):
    candidates = []
    for display in json.loads(rift("query", "displays")):
        for workspace in json.loads(rift("query", "workspaces", "--display", display["uuid"])):
            for window in workspace["windows"]:
                if window.get("bundle_id") == bundle_id:
                    rank = (
                        window.get("is_focused", False),
                        display["is_active_context"] and workspace["is_active"],
                        display["is_active_context"],
                        workspace["is_active"],
                    )
                    candidates.append((rank, display, workspace, window))
    return max(candidates, key=lambda item: item[0])[1:] if candidates else None

def focus(target):
    display, workspace, window = target
    LOG.info("target app=%s display=%s workspace=%s window=%s",
             window.get("bundle_id"), display["uuid"], workspace["index"], window["id"])
    if not display["is_active_context"]:
        rift("execute", "display", "focus", "--uuid", display["uuid"])
        # An empty active workspace gives display-focus no window to raise.
        # Raise the known target to establish command context, then explicitly
        # switch its workspace below (raising alone can leave it inactive).
        visible = json.loads(rift("query", "workspaces", "--display", display["uuid"]))
        if not any(ws["is_active"] and ws["windows"] for ws in visible):
            rift("execute", "window", "focus", "--window-id", json.dumps(window["id"]))
        # Wait for the new command context before switching its workspace.
        for _ in range(20):
            if any(
                item["uuid"] == display["uuid"] and item["is_active_context"]
                for item in json.loads(rift("query", "displays"))
            ):
                break
            time.sleep(0.05)
        else:
            raise RuntimeError("Rift did not focus the target display")
    workspaces = json.loads(rift("query", "workspaces", "--display", display["uuid"]))
    if not any(
        item["index"] == workspace["index"] and item["is_active"]
        for item in workspaces
    ):
        rift("execute", "workspace", "switch", str(workspace["index"]))
        for _ in range(20):
            if any(
                item["index"] == workspace["index"] and item["is_active"]
                for item in json.loads(rift("query", "workspaces", "--display", display["uuid"]))
            ):
                break
            time.sleep(0.05)
        else:
            raise RuntimeError("Rift did not activate the target workspace")
    rift("execute", "window", "focus", "--window-id", json.dumps(window["id"]))
    # An accepted command is not proof that macOS actually focused the window.
    for _ in range(10):
        workspaces = json.loads(rift("query", "workspaces", "--display", display["uuid"]))
        if any(
            ws["is_active"] and candidate["id"] == window["id"] and candidate["is_focused"]
            for ws in workspaces for candidate in ws["windows"]
        ):
            # Unlike direct window-focus, display-focus also warps the pointer
            # to its last-focused window's center, including on the same display.
            rift("execute", "display", "focus", "--uuid", display["uuid"])
            LOG.info("focus_confirmed app=%s", window.get("bundle_id"))
            return
        time.sleep(0.1)
    raise RuntimeError("Rift accepted the focus command but focus was not confirmed")

def main(bundle_id):
    LOG.info("request app=%s", bundle_id)
    target = find_window(bundle_id)
    if target:
        focus(target)
        return
    LOG.info("launch app=%s", bundle_id)
    subprocess.run(["/usr/bin/open", "-b", bundle_id], check=True, timeout=5)
    # Launch Services can return before Rift registers the app's window.
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        target = find_window(bundle_id)
        if target:
            focus(target)
            return
        time.sleep(0.1)
    raise RuntimeError("App launched, but Rift did not register a window within 5 seconds")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: focus-app.py BUNDLE_ID")
    configure_logging()
    try:
        main(sys.argv[1])
    except (subprocess.SubprocessError, OSError, ValueError, RuntimeError) as exc:
        LOG.error("failed app=%s error=%s stderr=%s",
                  sys.argv[1], exc, getattr(exc, "stderr", None))
        print(f"rift focus-app: {exc}", file=sys.stderr)
        sys.exit(1)
