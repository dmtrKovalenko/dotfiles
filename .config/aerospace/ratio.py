#!/usr/bin/env python3
"""Resize a tiled window to a percentage of the width of the display the mouse is on.
AeroSpace keeps the mouse on the focused window/monitor, so that is the right display.

  ratio.py <pct> [--window-id ID]   resize (focused window by default)
  ratio.py --width                  print that display's width in points
"""
import ctypes, subprocess, sys

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
cmd = ['aerospace', 'resize']
if '--window-id' in args:
    cmd += ['--window-id', args[args.index('--window-id') + 1]]
cmd += ['width', str(display_width() * pct // 100)]
sys.exit(subprocess.call(cmd))
