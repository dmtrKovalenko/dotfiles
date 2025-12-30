# Generate completions once and cache them
if not test -f ~/.config/fish/completions/just.fish
    just --completions fish > ~/.config/fish/completions/just.fish
end

if not test -f ~/.config/fish/conf.d/zoxide_init.fish
    zoxide init fish > ~/.config/fish/conf.d/zoxide_init.fish
end

if not test -f ~/.config/fish/conf.d/fzf_init.fish
    fzf --fish > ~/.config/fish/conf.d/fzf_init.fish
end

opam env | source

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
abbr --add 'zt' 'zig test --summary all'

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
