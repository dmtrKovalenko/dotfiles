#!/bin/sh
# Scratchpad on rift workspace "S" (index 4).
#   stash -> send the focused window to S
#   show  -> pull the most recently stashed window into the current workspace, floating, and focus it.
#            If nothing is stashed, jump to S. If already on S, jump back.
S=4
case "$1" in
  stash) rift-cli execute workspace move-window "$S" ;;
  show)
    ws=$(rift-cli query workspaces)
    cur=$(printf '%s' "$ws" | jq -r '.[] | select(.is_active) | .index')
    if [ "$cur" = "$S" ]; then rift-cli execute workspace last; exit 0; fi
    id=$(printf '%s' "$ws" | jq -c --argjson s "$S" '.[] | select(.index == $s) | .windows[0].id // empty')
    if [ -z "$id" ]; then rift-cli execute workspace switch "$S"; exit 0; fi
    rift-cli execute workspace move-window "$cur" "$id"
    rift-cli execute window focus --window-id "$id"
    rift-cli execute window toggle-float
    ;;
  *) echo "usage: $0 stash|show" >&2; exit 2 ;;
esac
