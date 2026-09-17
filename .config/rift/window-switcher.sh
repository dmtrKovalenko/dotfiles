#!/bin/sh
# Fuzzy switcher across the workspaces of the current display, rendered by Vicinae's dmenu mode.
sel=$(rift-cli query workspaces \
  | jq -r '.[] as $w | $w.windows[] | "[\($w.name)] \(.app_name // "?") — \(.title)  #\(.id | tojson)"' \
  | vicinae dmenu -n "Windows" -p "Switch to window…" -f data --no-section --no-quick-look)
[ -z "$sel" ] && exit 0
rift-cli execute window focus --window-id "${sel##*#}"
