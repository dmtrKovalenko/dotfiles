"""Test-only Herdr and Neovim clients; the installed plugin runs entirely in Rust."""

import subprocess

from pathlib import Path
import json
import os
import socket
import tempfile
import uuid


def state_dir():
    override = os.environ.get("HERDR_FINGERS_OPEN_STATE")
    root = Path(override) if override else Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "herdr-fingers-open"
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    root.chmod(0o700)
    return root


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    fd, temporary = tempfile.mkstemp(prefix=".write-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as stream:
            json.dump(value, stream)
            stream.write("\n")
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


class HerdrError(RuntimeError):
    def __init__(self, code, message):
        self.code = code
        super().__init__(message)


class Herdr:
    def __init__(self, path=None):
        self.path = path or os.environ.get("HERDR_SOCKET_PATH")
        if not self.path:
            raise RuntimeError("Run this action inside a Herdr pane.")

    def call(self, method, **params):
        request_id = str(uuid.uuid4())
        request = {"id": request_id, "method": method, "params": params}
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(5)
            connection.connect(self.path)
            connection.sendall(json.dumps(request).encode() + b"\n")
            with connection.makefile("rb") as stream:
                raw = stream.readline(4 * 1024 * 1024)
        response = json.loads(raw)
        if response.get("id") != request_id:
            raise RuntimeError("Herdr returned an unexpected response.")
        if "error" in response:
            error = response["error"]
            raise HerdrError(error.get("code"), error.get("message", "Herdr request failed"))
        return response["result"]

    def pane(self, pane_id):
        return self.call("pane.get", pane_id=pane_id)["pane"]

    def notify(self, message):
        try:
            self.call("notification.show", title="Fingers Open", body=message)
        except (OSError, ValueError, RuntimeError):
            pass


def nvim_call(editor, method, data=None):
    # Only data is interpolated, as a quoted Vim string parsed by json_decode.
    # Paths never become shell, Ex, or Lua code.
    argument = json.dumps(data, ensure_ascii=True).replace("'", "''")
    lua = 'require("herdr_fingers").' + method + '(_A)'
    expression = "luaeval('" + lua + "', json_decode('" + argument + "'))"
    result = subprocess.run([editor["nvim_bin"], "--server", editor["socket"], "--remote-expr", expression],
                            capture_output=True, text=True, timeout=6)
    if result.returncode:
        raise RuntimeError("Neovim did not accept the request: " + result.stderr.strip()[:300])
    try:
        response = json.loads(result.stdout)
    except ValueError as error:
        raise RuntimeError("Neovim returned an invalid response.") from error
    if not response.get("ok"):
        raise RuntimeError(response.get("error", "Neovim request failed."))
    return response
