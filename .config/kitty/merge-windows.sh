#!/bin/bash
# Merge the current kitty OS window into another kitty OS window.
# Every tab of this OS window is MOVED (not relaunched) into the target, so
# running shells, ssh sessions, editors etc. all survive. When the last tab
# leaves, kitty closes the now-empty OS window by itself.
#
# Requires in kitty.conf:  allow_remote_control yes   and   listen_on unix:/tmp/kitty
# Usage: run from a kitty overlay (see the map in kitty.conf).  --dry-run prints the plan only.

DRY=0; [ "$1" = "--dry-run" ] && DRY=1
KITTEN=/Applications/kitty.app/Contents/MacOS/kitten
[ -x "$KITTEN" ] || KITTEN=kitten
TO=${KITTY_LISTEN_ON:+--to "$KITTY_LISTEN_ON"}

if [ -z "$KITTY_WINDOW_ID" ]; then echo "not inside kitty"; exit 1; fi

# Parse `kitten @ ls` once into a compact table: osw_id  tab_id  tab_active  has_this_window  tab_title  active_window_id
# NOTE: tab ids change when a tab is detached; window ids do not, so we refocus by window id at the end.
TABLE=$($KITTEN @ $TO ls | python3 -c '
import json, sys, os
me = int(os.environ["KITTY_WINDOW_ID"])
for osw in json.load(sys.stdin):
    for t in osw["tabs"]:
        mine = any(w["id"] == me for w in t["windows"])
        active_win = next((w["id"] for w in t["windows"] if w.get("is_active")), t["windows"][0]["id"] if t["windows"] else "")
        print(osw["id"], t["id"], int(t["is_active"]), int(mine), t["title"].replace("\t"," "), active_win, sep="\t")
')

CUR_OSW=$(awk -F'\t' '$4==1{print $1}' <<<"$TABLE")
[ -z "$CUR_OSW" ] && { echo "cannot find my OS window"; exit 1; }

# Candidate targets: every other OS window, labelled by its tab titles
CANDIDATES=$(awk -F'\t' -v cur="$CUR_OSW" '$1!=cur{ titles[$1]=(titles[$1]?titles[$1]" | ":"")$5 } END{ for (o in titles) print o "\t" titles[o] }' <<<"$TABLE" | sort -n)
[ -z "$CANDIDATES" ] && { echo "no other kitty window to merge into"; sleep 1; exit 0; }

if [ "$(wc -l <<<"$CANDIDATES")" -eq 1 ]; then
  TARGET_OSW=$(cut -f1 <<<"$CANDIDATES")
else
  PICK=$(fzf --prompt="Merge this window into > " --with-nth=2 --delimiter=$'\t' --height=100% <<<"$CANDIDATES") || exit 0
  TARGET_OSW=$(cut -f1 <<<"$PICK")
fi

# Anchor tab: the active tab of the target OS window (fallback: its first tab)
TARGET_TAB=$(awk -F'\t' -v o="$TARGET_OSW" '$1==o && $3==1{print $2; exit}' <<<"$TABLE")
[ -z "$TARGET_TAB" ] && TARGET_TAB=$(awk -F'\t' -v o="$TARGET_OSW" '$1==o{print $2; exit}' <<<"$TABLE")

MY_TABS=$(awk -F'\t' -v o="$CUR_OSW" '$1==o{print $2}' <<<"$TABLE")
MY_ACTIVE_WIN=$(awk -F'\t' -v o="$CUR_OSW" '$1==o && $3==1{print $6}' <<<"$TABLE")

if [ "$DRY" = 1 ]; then
  echo "current OS window: $CUR_OSW  tabs: $(tr '\n' ' ' <<<"$MY_TABS")"
  echo "target  OS window: $TARGET_OSW  anchor tab: $TARGET_TAB"
  for t in $MY_TABS; do echo "  would run: kitten @ detach-tab --match id:$t --target-tab id:$TARGET_TAB"; done
  exit 0
fi

for t in $MY_TABS; do
  $KITTEN @ $TO detach-tab --match "id:$t" --target-tab "id:$TARGET_TAB"
done
# land on the window that was active here, now living in the target OS window
[ -n "$MY_ACTIVE_WIN" ] && $KITTEN @ $TO focus-window --match "id:$MY_ACTIVE_WIN"
