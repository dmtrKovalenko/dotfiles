#!/bin/sh
# Scratchpad on workspace "S".
#   stash -> send the focused window to S (it disappears from the current layout)
#   show  -> pull the most recently stashed window into the current workspace, floating, and focus it.
#            If nothing is stashed, jump to S. If already on S, jump back.
S=S
case "$1" in
  stash) aerospace move-node-to-workspace "$S" ;;
  show)
    cur=$(aerospace list-workspaces --focused)
    if [ "$cur" = "$S" ]; then aerospace workspace-back-and-forth; exit 0; fi
    id=$(aerospace list-windows --workspace "$S" --format '%{window-id}' | head -1)
    if [ -z "$id" ]; then aerospace workspace "$S"; exit 0; fi
    aerospace move-node-to-workspace --window-id "$id" "$cur"
    aerospace layout --window-id "$id" floating
    aerospace focus --window-id "$id"
    ;;
  *) echo "usage: $0 stash|show" >&2; exit 2 ;;
esac
