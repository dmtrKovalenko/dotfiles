"""Integration check against an isolated, real Neovim RPC server."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

from test_support import nvim_call


class Neovim(unittest.TestCase):
    def test_open_file_preserves_unsaved_buffer_and_treats_path_as_data(self):
        executable = shutil.which("nvim") or "/opt/homebrew/bin/nvim"
        if not Path(executable).exists():
            self.skipTest("Neovim is not installed")
        with tempfile.TemporaryDirectory(prefix="hfo-nvim-", dir="/tmp") as directory:
            root = Path(directory).resolve()
            path = root / "file's name | let g:hacked=1.lua"
            path.write_text("first\nsecond line\nthird\n")
            unsaved = root / "unsaved.txt"
            module = Path(__file__).resolve().parent / "herdr_fingers.lua"
            init = root / "init.lua"
            init.write_text('package.loaded.herdr_fingers = dofile(' + json.dumps(str(module)) + ')\n'
                            'require("herdr_fingers").setup()\n'
                            'vim.api.nvim_buf_set_name(0, ' + json.dumps(str(unsaved)) + ')\n'
                            'vim.api.nvim_buf_set_lines(0, 0, -1, false, {"keep this unsaved text"})\n'
                            'vim.o.hidden = false\n')
            env = dict(os.environ, HERDR_FINGERS_OPEN_STATE=str(root / "state"),
                       HERDR_SOCKET_PATH=str(root / "herdr.sock"), HERDR_PANE_ID="w1:p2",
                       XDG_STATE_HOME=str(root / "nvim-state"))
            sock = str(root / "nvim.sock")
            with (root / "nvim.log").open("wb") as log:
                process = subprocess.Popen([executable, "--headless", "-n", "-u", str(init), "-i", "NONE", "--listen", sock],
                                           env=env, stdout=log, stderr=log)
                try:
                    deadline = time.monotonic() + 8
                    files = []
                    while time.monotonic() < deadline:
                        files = list((root / "state/nvim").glob("*.json"))
                        if files:
                            break
                        if process.poll() is not None:
                            self.fail((root / "nvim.log").read_text())
                        time.sleep(0.05)
                    self.assertTrue(files, "Neovim did not register")
                    editor = json.loads(files[0].read_text())
                    self.assertEqual(nvim_call(editor, "status")["pane_id"], "w1:p2")
                    nvim_call(editor, "open", {"path": str(path), "line": 2, "column": 4})
                    lua = ('vim.json.encode({file=vim.api.nvim_buf_get_name(0), cursor=vim.api.nvim_win_get_cursor(0), '
                           'modified=vim.fn.getbufvar(vim.fn.bufnr(' + json.dumps(str(unsaved)) + '),"&modified"), '
                           'text=vim.fn.getbufline(' + json.dumps(str(unsaved)) + ',1), '
                           'hacked=vim.g.hacked or false, hidden=vim.o.hidden})')
                    query = "luaeval('" + lua.replace("'", "''") + "')"
                    result = json.loads(subprocess.check_output([executable, "--server", sock, "--remote-expr", query], text=True))
                    self.assertEqual(result["file"], str(path))
                    self.assertEqual(result["cursor"], [2, 3])
                    self.assertEqual(result["modified"], 1)
                    self.assertEqual(result["text"], ["keep this unsaved text"])
                    self.assertFalse(result["hacked"])
                    self.assertFalse(result["hidden"])
                    oil = Path.home() / ".local/share/nvim/lazy/oil.nvim"
                    if oil.is_dir():
                        lua = ('vim.opt.rtp:prepend(' + json.dumps(str(oil)) + '); '
                               'require("oil").setup({columns={}}); '
                               'vim.api.nvim_set_current_buf(vim.fn.bufnr(' + json.dumps(str(unsaved)) + '))')
                        expression = "execute(" + json.dumps("lua " + lua, ensure_ascii=False) + ")"
                        subprocess.run([executable, "--server", sock, "--remote-expr", expression], check=True, capture_output=True)
                        nvim_call(editor, "open", {"path": str(root), "line": 1, "column": 1})
                        deadline = time.monotonic() + 3
                        while time.monotonic() < deadline:
                            if nvim_call(editor, "status")["current_file"].startswith("oil://"):
                                break
                            time.sleep(0.02)
                        self.assertTrue(nvim_call(editor, "status")["current_file"].startswith("oil://"))
                        result = json.loads(subprocess.check_output([executable, "--server", sock, "--remote-expr", query], text=True))
                        self.assertEqual(result["modified"], 1)
                        self.assertEqual(result["text"], ["keep this unsaved text"])
                finally:
                    subprocess.run([executable, "--server", sock, "--remote-expr", 'execute("qa!")'],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.terminate()
                        process.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
