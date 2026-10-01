#!/opt/homebrew/bin/python3
"""Move the active workspace to another display and verify its focus handoff."""
import fcntl
import importlib.util
import json
from pathlib import Path
import sys
import time

spec = importlib.util.spec_from_file_location("focus_app", Path(__file__).with_name("focus-app.py"))
focus_app = importlib.util.module_from_spec(spec)
spec.loader.exec_module(focus_app)
rift = focus_app.rift

def move():
    displays = json.loads(rift("query", "displays"))
    if len(displays) < 2:
        return
    source = next(d for d in displays if d["is_active_context"])
    workspace = next(w for w in json.loads(rift("query", "workspaces", "--display", source["uuid"]))
                     if w["is_active"])
    windows = workspace["windows"]
    if not windows:
        return
    anchor = next((w for w in windows if w["is_focused"]), windows[0])
    rift("execute", "display", "move-workspace", "--direction", "right", "--wrap-around")
    for _ in range(30):
        for display in json.loads(rift("query", "displays")):
            if display["uuid"] == source["uuid"]:
                continue
            for destination in json.loads(rift("query", "workspaces", "--display", display["uuid"])):
                window = next((w for w in destination["windows"] if w["id"] == anchor["id"]), None)
                if window:
                    if destination["index"] != workspace["index"]:
                        raise RuntimeError("Workspace move changed its number")
                    focus_app.focus((display, destination, window))
                    rift("execute", "display", "move-mouse", "--uuid", display["uuid"])
                    for _ in range(20):
                        if any(d["uuid"] == display["uuid"] and d["is_active_context"]
                               for d in json.loads(rift("query", "displays"))):
                            focus_app.LOG.info("workspace_move_confirmed display=%s workspace=%s",
                                               display["uuid"], destination["index"])
                            return
                        time.sleep(.05)
                    raise RuntimeError("Moved workspace did not become the command context")
        time.sleep(.05)
    raise RuntimeError("Workspace did not move to another display")

if __name__ == "__main__":
    focus_app.configure_logging()
    with (Path.home() / ".cache/rift/move-workspace.lock").open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            sys.exit(0)
        try:
            move()
        except Exception as exc:
            focus_app.LOG.exception("workspace_move_failed")
            sys.exit(str(exc))
