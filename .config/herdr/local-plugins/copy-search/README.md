# Herdr copy mode

The local copy-search variant is vendored here so dotfiles can reproduce the
same selector on macOS and Linux. [UPSTREAM](UPSTREAM) records the base commit;
the original MIT [LICENSE](LICENSE) is included.

## Install on each machine

Apply the dotfiles first, install Herdr, then run:

```sh
python3 ~/.config/herdr/local-plugins/copy-search/install.py
```

Python 3 and a current stable Rust toolchain are needed for installation from
source. The installer uses `cargo build --release --locked`, copies the runtime
to `${XDG_DATA_HOME:-$HOME/.local/share}/herdr/copy-search`, links it with Herdr,
and reloads config without restarting the server. Build files live separately
under `${XDG_CACHE_HOME:-$HOME/.cache}/herdr-copy-search/target` and can be removed
after installation. Re-run the same command after changing the source.

At runtime there is one Rust executable plus the plugin manifest. Selection,
search and copying require no compiler, Python or Neovim. The installed manifest
has no build step. Clone, pull and the general yadm bootstrap do not install this
optional plugin automatically.

To reuse a binary built for the same OS and architecture:

```sh
python3 ~/.config/herdr/local-plugins/copy-search/install.py --binary /path/to/herdr-copy-search
```

`--herdr PATH` selects a Herdr executable. `--no-link --destination DIR` creates
a standalone package without contacting Herdr; copy the whole package to a
compatible host and run `herdr plugin link DIR` there. A macOS binary cannot run
on Linux, and Linux ARM and x86_64 need different binaries.

Herdr runs plugins on the machine hosting the pane. Apply these dotfiles and
run the installer on **oppi, poppi, sloppi and floppi** as well as the Mac. SSH
alone does not copy the plugin. For custom remote bindings, connect with
`--remote-keybindings server` when required by your Herdr client.

## Configuration and keys

The tracked [Herdr config](../../config.toml) disables the built-in
`keys.copy_mode` binding and maps `prefix+[` and `prefix+shift+c` to
`copy-search.copy`. With the tracked prefix, press **Ctrl+Space, then [**.
The tracked [plugin config](../../plugins/config/copy-search/config.toml) makes
the cursor orange (`#ff9e64`) with dark text (`#161819`). Registries, other plugin
state and compiled binaries stay untracked.

- `h j k l`, `w b e`, `0 ^ $`, `gg G`: move.
- `v` / `V`: character / line selection; `iw` / `aw`, `iW` / `aW`: text objects.
- `/` / `?`: search; `n` / `N`: next / previous match.
- `y`: copy; Enter: copy and close; `q`: close.
- Escape: clear selection, then close.
- `gx`: open the token under the cursor using the custom Fingers router.

`gx` needs the optional [Fingers setup](../fingers-open/README.md) on the source
machine. It invokes `herdr/fingers-open/runtime/herdr-fingers open-selection`
under `XDG_DATA_HOME` (default `~/.local/share`), passes the token on stdin, and
preserves the source pane for file resolution. Tokens are not interpolated into
shell commands. Files with `:line:column` or `#Lline` open in a registered Neovim;
URLs use Fingers' configured browser or remote bridge. Unresolved tokens use its
copy fallback. A missing router displays an error and keeps copy mode open.
Neovim is needed only for Fingers file-opening actions.

The selector preserves PTY foreground colors and text attributes; selection
changes the background to `#3c4047`. Passive mouse motion is ignored and redraws
use synchronized output. It is a snapshot of colored text, with plain-text
clipboard output through OSC 52; terminal graphics and hyperlinks are omitted.

Reads request passive ANSI snapshots, up to 1,000 recent rows. Regular terminal
scrollback remains available. Full-screen applications provide their current
screen: copy mode does not scroll the application to harvest older content.
Scroll the application first when you need older output. Herdr 0.9.5 can omit
soft-wrap metadata for full-screen snapshots, so `gx` cannot always join a token
split across rows. It does not guess that adjacent rows form one URL.

## Verify changes

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 verify_rendering.py
python3 verify_startup.py --plugin ~/.local/share/herdr/copy-search
python3 verify_local.py --plugin ~/.local/share/herdr/copy-search
python3 verify_ssh.py oppi
```

`verify_rendering.py` checks the binary with a disposable PTY. The other checks
need an installed Herdr executable; `verify_local.py` also needs the custom
Fingers runtime and Neovim for its file-opening cases. They use disposable
servers and captured terminal output. The SSH check needs an already configured
host and plugin; it starts a named test session and leaves existing sessions
running. Verification scripts default to `~/.local/bin/herdr`;
`verify_local.py --herdr PATH` can select another executable.

## Remove

Unlink `copy-search`, remove the `copy-search.copy` command block and
`copy_mode = []` override from the Herdr config, then run
`herdr server reload-config` to restore native copy mode.
