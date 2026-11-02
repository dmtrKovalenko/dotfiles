# Fingers Open

A single Rust executable containing the Fingers hint overlay, file routing,
browser bridge, remote relay, and installer. It opens files in an existing Neovim
pane and URLs in the browser on your Mac. No Python or Rust toolchain is required
to run a prebuilt binary. Neovim, Herdr, Git, SSH, and the browser remain external
applications. The installer extracts the small embedded Lua module for Neovim.

Press **Ctrl+Space, then f**, then type a hint:

- An existing file opens in Neovim in the same Herdr workspace. Fingers keeps
  the hint screen visible while opening it and switches straight to the editor's
  tab. For an editor in the same tab, focus waits for the overlay to restore the
  split layout.
- A directory opens in that editor's Oil file browser. `~`, `$HOME`, and `${HOME}`
  home paths are supported.
- An HTTP(S) URL opens in Google Chrome on the Mac, including selections on configured SSH hosts.
  If the browser connection fails, the URL is copied and a notification explains why.
- Other text is copied through the same OSC 52 clipboard mechanism as Fingers.
- **Ctrl + hint** always copies. **Shift + hint** keeps the existing paste action.

File references can include `:line`, `:line:column`, `:start-end`, or `#Lstart-Lend`.
Ranges jump to their first line. Quoted filenames can contain spaces. Relative
paths resolve against the source pane and its Git worktree. Shortened names also
search that worktree and buffers already open in its Neovim instances. Multiple
matching files or editors produce a chooser; use j/k or arrows, Enter, or Esc.
If no editor is available in the current workspace, or Neovim cannot open a file
or directory, the original selection is copied and a notification explains why.
Canceling a chooser leaves the clipboard unchanged. Unsaved buffers remain loaded.
Unresolved path-like selections show a notice when copied. The last action's
resolution details are saved privately in `~/.cache/herdr-fingers-open/last-action.json`.

## Install

Use a binary built for the machine's OS and architecture. The installed runtime is
`${XDG_DATA_HOME:-$HOME/.local/share}/herdr/fingers-open/runtime/herdr-fingers`.
Herdr 0.9.1+ and an existing Neovim configuration are required; Oil handles directories.

Build once from the source tracked in yadm (Rust 1.88+ and Cargo):

```sh
cd ~/.config/herdr/local-plugins/fingers-open/rust
cargo build --release --locked
```

On the Mac where the browser should open:

```sh
./target/release/herdr-fingers install --role local --host oppi --host sloppi
~/.local/share/herdr/fingers-open/runtime/herdr-fingers install-browser-service
~/.local/share/herdr/fingers-open/runtime/herdr-fingers register-existing
```

On a devbox, build there or copy a compatible Linux binary, then run:

```sh
./herdr-fingers install --role remote
~/.local/share/herdr/fingers-open/runtime/herdr-fingers register-existing
```

Use `--herdr /path/to/herdr` if Herdr is not in `~/.local/bin` or PATH. Repeat
`--host` for SSH aliases on the Mac. Existing browser settings and host lists are
preserved. The installer copies itself, configures the native selection action,
links both existing plugin IDs to that one executable, and installs Neovim registration.
It backs up replaced files in `~/.cache/herdr-fingers-open-backups`.
`register-existing` refreshes running Neovims without restarting their buffers.

To update, rebuild or copy a newer binary and repeat installation. On the Mac,
rerun `install-browser-service` to restart the bridge with that binary. Source
edits and `yadm pull` alone do not change the installed executable. The compiled
runtime remains usable after deleting Cargo's `target` directory.

Yadm stores the Rust source, lockfile, embedded Lua, and tests. It excludes binaries,
build caches, machine-specific browser settings, plugin registries, LaunchAgents,
logs, sockets, and editor registrations. Neither bootstrap nor SSH installs the plugin.

For direct SSH attach, use the server's custom plugin bindings:

```sh
herdr --remote oppi --remote-keybindings server
```

## Browser bridge

The Mac's per-user login service keeps a dedicated SSH connection to each configured
host, with keepalives and automatic reconnection. It does not reuse dbox's shared
SSH connection, which can become unresponsive after a network change.
The same binary on that host receives selections through a private Unix socket, forwards
them over SSH, and waits for the local browser's acknowledgement. It accepts only
HTTP and HTTPS URLs. There is no TCP listener or general command-execution API.
If multiple Macs connect to the same host, opening is refused rather than choosing
an arbitrary browser. No requests are queued for later replay.

Configuration: `~/.config/herdr/plugins/config/fingers-open/browser.json`.
Add SSH targets to `hosts`; set `browser` to `default` to use the OS default browser.

```sh
~/.local/share/herdr/fingers-open/runtime/herdr-fingers browser status
launchctl print gui/$(id -u)/com.dkovalenko.herdr-fingers-browser
```

Runtime files and connection diagnostics are under `~/.cache/herdr-fingers-open`.
The service reconnects after SSH failures. A disconnected bridge copies the selected
URL and reports the connection problem. Remote development
URLs such as `http://localhost:3000` still need the corresponding SSH port forward;
the bridge opens the selected URL unchanged on the Mac.

File routing stays on the Herdr server: remote files open in remote Neovim.
It does not map remote paths into a Mac checkout. Hints operate on visible text;
an OSC 8 hyperlink whose target is hidden behind a label is not decoded by Fingers.

## Verification

```sh
cd ~/.config/herdr/local-plugins/fingers-open/rust
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cd ..
python3 -B -m unittest -v test_nvim
python3 -B verify_herdr.py --herdr "$(command -v herdr)" \
  --nvim "$(command -v nvim)" --fingers "$PWD/rust/target/release/herdr-fingers"
```

The Rust integration tests require Neovim in PATH, or `NVIM_BIN=/path/to/nvim`.
Python is only used for the additional test drivers. The end-to-end test installs
into temporary XDG directories and starts its own disposable Herdr server. It
checks real hints, cursor positions, home paths, directory opening, chooser
cancellation, existing-editor registration, and focus across tabs and within splits.
`--url URL` additionally opens that URL through the live browser bridge and tests
clipboard fallback after disconnecting the test session from the bridge.
Test directories are printed and retained for inspection; remove them afterward.

The hint engine is vendored from `nathan-poncet/herdr-fingers` at commit
`b763a3ffe4e8f909584f2e19229f95719f3f4314`; see [rust/LICENSE](rust/LICENSE).
Native custom behavior lives in [rust/src/custom](rust/src/custom).
