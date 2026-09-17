#!/bin/bash
# Auto layout for a workspace:
#   1st + 2nd window  -> side by side
#   3rd window        -> bottom-right quarter (stacked under the right column)
#   4th window        -> bottom-left quarter  (stacked under the left column)
#   5th and more      -> left to AeroSpace's normal tiling
# Started by AeroSpace via after-startup-command. Polls the focused workspace.

# make sure only one copy runs
for p in $(pgrep -f "$(basename "$0")"); do [ "$p" != "$$" ] && kill "$p" 2>/dev/null; done

# move the focused window in a direction until it hits the workspace edge
push() {
  local i
  for i in 1 2 3 4 5 6; do
    aerospace move --boundaries workspace --boundaries-action fail "$1" 2>/dev/null || break
  done
}

prev=""; prevws=""
while true; do
  ws=$(aerospace list-workspaces --focused 2>/dev/null) || { sleep 1; continue; }
  cur=$(aerospace list-windows --workspace focused --format '%{window-id} %{window-layout}' 2>/dev/null \
        | grep -vE 'floating|fullscreen' | awk '{print $1}' | sort -n)
  if [ "$ws" = "$prevws" ] && [ -n "$prev" ]; then
    new=$(comm -13 <(echo "$prev") <(echo "$cur"))
    n=$(echo "$cur" | grep -c .)
    for w in $new; do
      case $n in
        3)  # push to far right, then stack under the right column
          aerospace focus --window-id "$w"
          push right
          aerospace join-with left
          ;;
        4)  # leave the right column if inside it, push to far left, stack with the left column, then drop below it
          aerospace focus --window-id "$w"
          case "$(aerospace list-windows --focused --format '%{window-parent-container-layout}')" in
            v_*) aerospace move left ;;
          esac
          push left
          aerospace join-with right
          aerospace move --boundaries-action fail down 2>/dev/null
          ;;
      esac
    done
  fi
  prev=$cur; prevws=$ws
  sleep 0.1
done
