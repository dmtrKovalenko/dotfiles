#!/opt/homebrew/bin/python3
"""Numbered shortcuts use the external display, falling back when undocked."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import time

spec = importlib.util.spec_from_file_location("focus_app", Path(__file__).with_name("focus-app.py"))
focus_app = importlib.util.module_from_spec(spec)
spec.loader.exec_module(focus_app)
rift = focus_app.rift

EXTERNAL = "6DC30A0D-24E9-4C2A-A04A-4AD88327EF55"
BUILTIN = "37D8832A-2D66-02CA-B9F7-8F30A301B230"
SLACK = "com.tinyspeck.slackmacgap"

def choose_display(displays, number):
    for uuid in (EXTERNAL, BUILTIN):
        match = next((d for d in displays if d["uuid"] == uuid), None)
        if match:
            return match
    return next((d for d in displays if d["is_active_context"]), displays[0])

def workspaces(uuid):
    return json.loads(rift("query", "workspaces", "--display", uuid))

def switch(number):
    display = choose_display(json.loads(rift("query", "displays")), number)
    uuid = display["uuid"]
    index = number - 1
    focus_app.LOG.info("workspace_request number=%s display=%s", number, uuid)
    if not display["is_active_space"]:
        raise RuntimeError(
            f"Rift management is inactive on {display.get('name') or uuid}; "
            "focus that display and re-enable it with Cmd+Ctrl+Alt+Shift+Z"
        )
    if number == 3:
        # Seed an empty target desktop before focusing it: Rift's display-focus
        # command cannot establish focus there when it has no windows to raise.
        target = focus_app.find_window(SLACK)
        if not target:
            focus_app.main(SLACK)
            target = focus_app.find_window(SLACK)
        if target and target[0]["uuid"] != uuid:
            rift("execute", "display", "move-window", "--uuid", uuid,
                 "--window-id", str(target[2]["id"]["idx"]))
            for _ in range(20):
                target = focus_app.find_window(SLACK)
                if target and target[0]["uuid"] == uuid:
                    break
                time.sleep(.05)
            else:
                raise RuntimeError("Slack did not move to the target display")
    if not display["is_active_context"]:
        rift("execute", "display", "focus", "--uuid", uuid)
        visible = workspaces(uuid)
        if not any(ws["is_active"] and ws["windows"] for ws in visible):
            anchor = next((w for ws in visible if ws["index"] == index
                           for w in ws["windows"]), None)
            if anchor:
                rift("execute", "window", "focus", "--window-id", json.dumps(anchor["id"]))
        for _ in range(20):
            if any(d["uuid"] == uuid and d["is_active_context"]
                   for d in json.loads(rift("query", "displays"))):
                break
            time.sleep(.05)
        else:
            raise RuntimeError("Target display did not become active")
    if not any(ws["index"] == index and ws["is_active"] for ws in workspaces(uuid)):
        rift("execute", "workspace", "switch", str(index))
    for _ in range(20):
        workspace = next(ws for ws in workspaces(uuid) if ws["index"] == index)
        if workspace["is_active"]:
            break
        time.sleep(.05)
    else:
        raise RuntimeError("Target workspace did not become active")
    if number == 3:
        target = focus_app.find_window(SLACK)
        if not target:
            focus_app.main(SLACK)
            target = focus_app.find_window(SLACK)
        if not target:
            raise RuntimeError("Slack has no registered window")
        source_display, source_workspace, window = target
        if source_display["uuid"] != uuid:
            rift("execute", "display", "move-window", "--uuid", uuid,
                 "--window-id", str(window["id"]["idx"]))
            for _ in range(20):
                target = focus_app.find_window(SLACK)
                if target and target[0]["uuid"] == uuid:
                    break
                time.sleep(.05)
            else:
                raise RuntimeError("Slack did not move to the target display")
        if target[1]["index"] != index:
            rift("execute", "workspace", "move-window", str(index),
                 str(window["id"]["idx"]), "--follow")
        for _ in range(20):
            target = focus_app.find_window(SLACK)
            if target and target[0]["uuid"] == uuid and target[1]["index"] == index:
                focus_app.focus(target)
                break
            time.sleep(.05)
        else:
            raise RuntimeError("Slack did not reach workspace 3 on the target display")
    focus_app.LOG.info("workspace_confirmed number=%s display=%s", number, uuid)

if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in {"1", "2", "3", "4"}:
        sys.exit("usage: workspace.py 1|2|3|4")
    focus_app.configure_logging()
    try:
        switch(int(sys.argv[1]))
    except (subprocess.SubprocessError, OSError, ValueError, RuntimeError) as exc:
        focus_app.LOG.error("workspace_failed number=%s error=%s", sys.argv[1], exc)
        sys.exit(str(exc))
