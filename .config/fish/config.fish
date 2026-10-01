# The following lines were added by Docker Desktop to add commands to your PATH.
export PATH="$PATH:$HOME/.docker/bin"
# End of Docker Desktop section.

# Generate completions once and cache them
set -l secretive_socket "$HOME/Library/Containers/com.maxgoedjen.Secretive.SecretAgent/Data/socket.ssh"
if test (uname) = Darwin; and test -S "$secretive_socket"; and not set -q SSH_CONNECTION
    set -gx SSH_AUTH_SOCK "$secretive_socket"
end

fish_add_path "$HOME/.local/bin" "$HOME/.cargo/bin"
for brew_prefix in /opt/homebrew /usr/local
    if test -x "$brew_prefix/bin/brew"
        fish_add_path "$brew_prefix/bin" "$brew_prefix/sbin"
        break
    end
end

mkdir -p "$HOME/.config/fish/completions" "$HOME/.config/fish/conf.d"

if type -q just; and not test -s ~/.config/fish/completions/just.fish
    just --completions fish > ~/.config/fish/completions/just.fish
end

if type -q zoxide; and not test -s ~/.config/fish/conf.d/zoxide_init.fish
    zoxide init fish > ~/.config/fish/conf.d/zoxide_init.fish
end

if type -q fzf; and not test -s ~/.config/fish/conf.d/fzf_init.fish
    fzf --fish > ~/.config/fish/conf.d/fzf_init.fish
end

if type -q opam; and test -d "$HOME/.opam"
    opam env --shell=fish 2>/dev/null | source
end

# Optimize done.fish plugin - increase min duration to reduce overhead
set -g __done_min_cmd_duration 10000

# Git prompt configuration (set once at startup)
set -g __fish_git_prompt_char_stateseparator ' '
set -g __fish_git_prompt_use_informative_chars 'yes'
set -g __fish_git_prompt_color_dirtystate yellow
set -g __fish_git_prompt_color $fish_color_normal
set -g __fish_git_prompt_color_flags $fish_color_status
set -g __fish_git_prompt_color_branch $fish_color_cwd
set -g __fish_git_prompt_char_dirtystate '~'
set -g __fish_git_prompt_char_untrackedfiles '+'
set -g __fish_git_prompt_showuntrackedfiles 'yes'
set -g __fish_git_prompt_showupstream 'no'
set -g __fish_git_prompt_show_informative_status 'no'

set -g _prompt_success_color (set_color cyan)
set -g _prompt_status_color (set_color $fish_color_status 2>/dev/null; or set_color red --bold)
set -g _prompt_user_color (set_color $fish_color_user 2>/dev/null; or set_color cyan)
set -g _prompt_cwd_color (set_color $fish_color_cwd 2>/dev/null; or set_color green)
set -g _prompt_normal (set_color normal)

# Custom abbreviations
abbr --add 'rm' 'rm -rf'
abbr --add '-' 'cd -'

abbr -a L --position anywhere --set-cursor "%| less -r"
abbr -a F --position anywhere --set-cursor "%| fzf"
abbr -a Y --position anywhere --set-cursor "%| pbcopy"

abbr --add 'c' 'cargo'
abbr --add 'cc' 'cargo check'
abbr --add 'cb' 'cargo build'
abbr --add 'cbr' 'cargo build --release'
abbr --add 'cr' 'cargo run'
abbr --add 'ct' 'cargo test'
abbr --add 'cfa' 'cargo fmt --all'
abbr --add 'zb' 'zig build  --release=fast'
abbr --add 'zt' 'zig build test --summary all'

# 💀
abbr --add 'мшь' 'vim'

# Tools
abbr --add 'y' 'yarn'
abbr --add 'cat' 'bat'
abbr --add 'ls' 'eza'
abbr --add 'j' 'just'
abbr --add 'm' 'make'

# Git
abbr --add 'grr' 'git rebase --continue'
abbr --add 'gac' 'git add --all && git commit -m'
abbr --add 'gap' 'git commit --amend --no-edit && git push --force-with-lease'
abbr --add 'gaap' 'git add --all && git commit --amend --no-edit && git push --force-with-lease'
abbr --add 'gtsnap' 'git diff --name-only | imfzf -m -q .png | xargs git checkout'
abbr --add 'grim' 'git fetch && git rebase -i --autostash origin/(__git.default_branch)'
abbr --add --position anywhere --set-cursor 'gbc' 'git branch --contains % | xargs git checkout'

# Git spr
abbr --add 'sap' 'git commit --amend --no-edit && git spr update' 
abbr --add 'saap' 'git add --all && git commit --amend --no-edit && git spr update' 
abbr --add 'spu' 'git spr update'
abbr --add 'sps' 'git spr status'


fish_add_path ~/.opencode/bin

# bun
set --export BUN_INSTALL "$HOME/.bun"
set --export PATH $BUN_INSTALL/bin $PATH

# pnpm
set -gx PNPM_HOME "$HOME/.local/share/pnpm"
if not string match -q -- $PNPM_HOME $PATH
  set -gx PATH "$PNPM_HOME" $PATH
end
# pnpm end

# The next line updates PATH for the Google Cloud SDK.
if [ -f "$HOME/dev/lightsource/google-cloud-sdk/path.fish.inc" ]; . "$HOME/dev/lightsource/google-cloud-sdk/path.fish.inc"; end

# >>> grok installer >>>
fish_add_path $HOME/.grok/bin
# <<< grok installer <<<

# Bedrock model override leaks in via the herdr daemon env; never let it pick the Claude Code model
set -e ANTHROPIC_MODEL
