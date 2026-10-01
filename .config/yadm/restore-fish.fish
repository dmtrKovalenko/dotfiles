# Plugin sources are bundled in Git; a fresh HOME lacks Fisher's registry.
set -g fish_function_path "$HOME/.config/fish/functions" $fish_function_path
source "$HOME/.config/fish/functions/fisher.fish"
set -l plugins (string lower -- (string match --regex -- '^[^\s]+$' < "$HOME/.config/fish/fish_plugins"))
if not set -q _fisher_plugins
    set -U _fisher_plugins $plugins
end
fisher update
or exit 1
for plugin in $plugins
    set -l registry _fisher_(string escape --style=var -- (string lower -- $plugin))_files
    set -q $registry
    or exit 1
end
