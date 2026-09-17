#!/usr/bin/env python3
"""Resize the focused tiled window to a percentage of the width of the display the mouse is on.
  ratio.py <pct>     resize focused window
  ratio.py --width   print that display's width in points
"""
import ctypes, json, subprocess, sys

cg = ctypes.cdll.LoadLibrary('/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics')
class CGPoint(ctypes.Structure): _fields_ = [('x', ctypes.c_double), ('y', ctypes.c_double)]
class CGSize(ctypes.Structure):  _fields_ = [('w', ctypes.c_double), ('h', ctypes.c_double)]
class CGRect(ctypes.Structure):  _fields_ = [('origin', CGPoint), ('size', CGSize)]
cg.CGDisplayBounds.restype = CGRect; cg.CGDisplayBounds.argtypes = [ctypes.c_uint32]
cg.CGEventCreate.restype = ctypes.c_void_p
cg.CGEventGetLocation.restype = CGPoint; cg.CGEventGetLocation.argtypes = [ctypes.c_void_p]
cg.CGMainDisplayID.restype = ctypes.c_uint32

def display_width():
    ids = (ctypes.c_uint32 * 16)(); n = ctypes.c_uint32()
    cg.CGGetActiveDisplayList(16, ids, ctypes.byref(n))
    m = cg.CGEventGetLocation(cg.CGEventCreate(None))
    for i in range(n.value):
        r = cg.CGDisplayBounds(ids[i])
        if r.origin.x <= m.x < r.origin.x + r.size.w and r.origin.y <= m.y < r.origin.y + r.size.h:
            return int(r.size.w)
    return int(cg.CGDisplayBounds(cg.CGMainDisplayID()).size.w)

args = sys.argv[1:]
if not args or args[0] == '--width':
    print(display_width()); sys.exit(0)
pct = int(args[0])
wins = json.loads(subprocess.check_output(['rift-cli', 'query', 'windows']))
focused = next((w for w in wins if w.get('is_focused')), None)
if not focused:
    sys.exit('no focused window')
cur = focused['frame']['size']['width'] if 'size' in focused['frame'] else focused['frame']['w']
dw = display_width()
target = dw * pct / 100
# rift resize-by takes a fraction of the SCREEN width to add to the window's split
sys.exit(subprocess.call(['rift-cli', 'execute', 'window', 'resize-by', f'--amount={(target - cur) / dw:.4f}']))
