#!/bin/sh
# Toggle the active workspace between stack (accordion) and bsp, like AeroSpace "layout accordion tiles".
mode=$(rift-cli query workspace-layout | jq -r ".[] | select(.is_active) | .layout_mode")
if [ "$mode" = "stack" ]; then rift-cli execute workspace set-layout traditional; else rift-cli execute workspace set-layout stack; fi
