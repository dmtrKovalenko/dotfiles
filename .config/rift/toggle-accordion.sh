#!/bin/sh
# Toggle the active workspace between stack (accordion) and its previous tiling mode (bsp by default),
# like AeroSpace "layout accordion tiles".
ws=$(rift-cli query workspace-layout | jq -c '.[] | select(.is_active)')
idx=$(echo "$ws" | jq -r '.index // 0')
mode=$(echo "$ws" | jq -r '.layout_mode')
prev="${TMPDIR:-/tmp}/rift-accordion-prev-$idx"
if [ "$mode" = "stack" ]; then
  rift-cli execute workspace set-layout "$(cat "$prev" 2>/dev/null || echo bsp)"
else
  echo "$mode" > "$prev"
  rift-cli execute workspace set-layout stack
fi
