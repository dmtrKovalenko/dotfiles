just --completions fish | source
zoxide init fish | source
opam config env	| source
fzf --fish | source

opam env | source

# Custom abbreviations
abbr --add 'rm' 'rm -rf'
abbr --add '-' 'cd -'

# custom typos
abbr --add 'jsut' 'just'
abbr --add 'jtsu' 'just'
abbr --add 'jstu' 'just'
abbr --add 'mkae' 'make'
abbr --add 'amek' 'make'
abbr --add 'maek' 'make'
abbr --add 'amke' 'make'
abbr --add 'gti' 'git'

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

# Tools
abbr --add 'y' 'yarn'
abbr --add 'cat' 'bat'
abbr --add 'ls' 'eza'
abbr --add 'j' 'just'
abbr --add 'm' 'make'
abbr --add 'rbf' 'RUST_BACKTRACE=full'

# Git
abbr --add 'grr' 'git rebase --continue'
abbr --add 'gac' 'git add --all && git commit -m'
abbr --add 'gap' 'git commit --amend --no-edit && git push --force-with-lease'
abbr --add 'gaap' 'git add --all && git commit --amend --no-edit && git push --force-with-lease'
abbr --add 'gtsnap' 'git diff --name-only | imfzf -m -q .png | xargs git checkout'
abbr --add 'grim' 'git fetch && git rebase -i --autostash origin/(__git.default_branch)'
abbr --add 'grac' 'git add --all && git rebase --continue' 
abbr --add --position anywhere --set-cursor 'gbc' 'git branch --contains % | xargs git checkout'

# Git spr
abbr --add 'sap' 'git commit --amend --no-edit && git spr update' 
abbr --add 'saap' 'git add --all && git commit --amend --no-edit && git spr update' 
abbr --add 'spu' 'git spr update'
abbr --add 'sps' 'git spr status'

