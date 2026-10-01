# SSH development environment

On Debian/Ubuntu, install the tools needed to clone this repository, then run:

```sh
sudo apt-get update
sudo apt-get install -y ca-certificates curl git yadm
yadm clone --bootstrap https://github.com/dmtrkovalenko/dotfiles.git
exec fish
```

On macOS, install yadm with Homebrew first. Linux uses apt-get (Debian/Ubuntu),
dnf (Fedora), or pacman (Arch); macOS uses Homebrew. Linux binary downloads
support x86_64 and arm64 on recent distributions with glibc. Package installation
requires root or sudo. On macOS, Homebrew needs the Xcode command line tools.

Bootstrap installs CLI tools for SSH: Git/GitHub CLI, Fish, tmux, ripgrep, fd,
fzf, jq, bat, eza, just, zoxide, C/C++ build tools, CMake, Python/uv, Rust,
Node LTS, Neovim and tree-sitter. It restores Fish plugins from `fish_plugins`
and Neovim plugins from `lazy-lock.json`. A local `~/dev/fff.nvim` checkout is
used when present; other machines install the GitHub plugin. Project-specific
packages, language servers (`:Mason`), other language toolchains, credentials,
and secret decryption remain manual.

The desktop `~/Brewfile` is an inventory; bootstrap never runs it. It installs
no GUI apps or Rift, loads no LaunchAgents, and changes no desktop settings.
It also leaves the login shell alone unless requested:

```sh
~/.config/yadm/bootstrap --change-shell
```

After pulling dotfile updates, run `yadm bootstrap` again. Installed tools are
kept, and unchanged plugin manifests skip downloads. Plugin completion markers
live in `${XDG_CACHE_HOME:-~/.cache}/yadm-bootstrap`; remove `fish-ready` or
`nvim-ready` there to force a restore. A fresh install needs downloads and can
take minutes; subsequent runs should take seconds. Failed steps return a
nonzero status and can be retried with the same command.

# Portable paths with editable live configs

Keep editing the normal config files, including through applications. For the
files listed in `~/.gitattributes`, Git replaces the current home directory with
`@@DOTFILES_HOME@@` when staging and expands it when checking out. Rift log names
also replace the login name with `@@DOTFILES_USER@@`. The driver preserves config
formatting and escapes JSON, TOML, XML, Lua strings, and Kitty kitten arguments.
Shell commands in Herdr, Fish, and `.profile` use quoted `$HOME` directly.

Set up an existing checkout once:

```sh
python3 ~/.config/yadm/setup-path-filter.py
```

Fresh `yadm clone` runs setup through the `post_clone` hook. Bootstrap also runs
setup after installing Python. Setup expands untouched placeholder files and
preserves local edits; it never stages or commits files. Python 3.11+ is required.
If Python is missing during clone, the hook defers setup to bootstrap. Run
`yadm bootstrap` before using the affected configs. The driver commands are
local to each repository and are registered separately on each machine.

Use `yadm add <file>` and `yadm commit` as usual. `yadm diff --cached` shows the
portable contents; the live files retain usable absolute paths. Add a file to
`.gitattributes` only after adding support for its path syntax to the driver,
then stage that specific file with `yadm add --renormalize <file>`.

The filter only replaces the current home path and Rift log-name prefix. It
doesn't replace account names, email addresses, arbitrary usernames, executable
locations such as `/opt/homebrew`, or paths belonging to other users. Home paths
containing control characters are rejected. This is path portability, not a
secret scrubber.

OpenAI/work configs, credentials, Herdr sessions and plugin registries, logs,
backups, and experimental Rift builds are outside this change. Herdr's custom
plugin bindings require those optional plugins to be installed separately.
