#!/bin/sh
# Build the dev layout on the current workspace, deterministically:
#   [ ChatGPT + kitty stack ] | Brave      (50 / 50)
STACK_A="com.openai.codex"         # ChatGPT (left, stacked)
STACK_B="net.kovidgoyal.kitty"     # kitty   (left, stacked)
RIGHT="com.brave.Browser"          # browser (right)

wid() { rift-cli query windows | jq -c --arg b "$1" "[.[] | select(.bundle_id == \$b and (.is_floating | not))][0].id // empty"; }
xof() { rift-cli query windows | jq -r --argjson id "$1" ".[] | select(.id == \$id) | .frame.origin.x"; }
rootkind() { rift-cli query layout | jq -r ".container_tree.layout_kind // \"\""; }
x() { rift-cli execute "$@" >/dev/null; }
focus() { x window focus --window-id "$1"; }
lt() { [ "$(echo "$1 < $2" | bc)" = 1 ]; }

A=$(wid "$STACK_A"); B=$(wid "$STACK_B"); R=$(wid "$RIGHT")
[ -z "$A" ] || [ -z "$B" ] || [ -z "$R" ] && { echo "need ChatGPT, kitty and Brave on this workspace" >&2; exit 1; }

x workspace set-layout traditional
# 1. flatten everything to one row
case "$(rootkind)" in *_stack) focus "$A"; x layout toggle-stack ;; esac
for i in 1 2; do for id in "$A" "$B" "$R"; do focus "$id"; x layout unjoin; done; done
# 2. browser to the far right (stop as soon as it is rightmost; extra moves would nest the others)
focus "$R"
for i in 1 2 3; do
  rx=$(xof "$R"); ax=$(xof "$A"); bx=$(xof "$B")
  if lt "$rx" "$ax" || lt "$rx" "$bx"; then x layout move-node right; else break; fi
done
# 3. join ChatGPT toward kitty, then turn that column into a stack
focus "$A"
if lt "$(xof "$B")" "$(xof "$A")"; then x layout join-window left; else x layout join-window right; fi
x layout toggle-stack
# 4. column must sit left of the browser
if lt "$(xof "$R")" "$(xof "$A")"; then focus "$A"; x layout ascend; x layout move-node left; fi
# 5. 50 / 50
focus "$A"; ~/.config/rift/ratio.py 50 >/dev/null; focus "$A"
