# User-installed CLI tools are available in login shells, including over SSH.
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.docker/bin:$PATH"

if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi
if [ -f "$HOME/.local/bin/env" ]; then . "$HOME/.local/bin/env"; fi

case "$(uname -s)" in
    Darwin)
        for brew_bin in /opt/homebrew/bin/brew /usr/local/bin/brew; do
            if [ -x "$brew_bin" ]; then eval "$("$brew_bin" shellenv)"; break; fi
        done
        ;;
esac
