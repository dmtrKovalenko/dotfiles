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
If Python is missing during clone, install it and run setup before using the
affected configs. The driver commands are local to each repository and are
registered separately on each machine.

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
