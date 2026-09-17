#!/bin/sh
# Fuzzy switcher across all workspaces/monitors, rendered by Vicinae's dmenu mode.
sel=$(aerospace list-windows --all --format '[%{workspace}] %{app-name} — %{window-title}  #%{window-id}' \
  | vicinae dmenu -n "Windows" -p "Switch to window…" -f data --no-section --no-quick-look)
[ -z "$sel" ] && exit 0
aerospace focus --window-id "${sel##*#}"
