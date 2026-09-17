#!/bin/sh
# Toggle an app by bundle id:
#   not focused  -> bring it to front (launches it, or restores it if minimized)
#   focused      -> minimize its focused window, so the rest of the layout takes the space
# Usage: toggle-app.sh <bundle-id>
id=$1
front=$(aerospace list-windows --focused --format '%{app-bundle-id}' 2>/dev/null)
if [ "$front" = "$id" ]; then
  aerospace macos-native-minimize
else
  open -b "$id"
fi
