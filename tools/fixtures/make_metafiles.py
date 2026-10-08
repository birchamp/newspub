"""Writes journeys/fixtures/shapes.emf and shapes.wmf from the published MS-EMF / MS-WMF record layouts.

Both draw, in a 200 x 100 logical frame: a blue (30, 90, 160) filled rectangle over the left half, a red (220, 40, 40)
filled ellipse in the right half with no outline, and a green (40, 160, 60) 4-unit polyline along the bottom.
"""
import struct, sys, os

BLUE, RED, GREEN = (30, 90, 160), (220, 40, 40), (40, 160, 60)
def colorref(c): return c[0] | (c[1] << 8) | (c[2] << 16)

def emf():
    recs = []
    def rec(t, data): recs.append(struct.pack('<II', t, 8 + len(data)) + data)
    rec(9, struct.pack('<ii', 200, 100))            # SETWINDOWEXTEX
    rec(10, struct.pack('<ii', 0, 0))               # SETWINDOWORGEX
    rec(39, struct.pack('<IIII', 1, 0, colorref(BLUE), 0))   # CREATEBRUSHINDIRECT #1
    rec(37, struct.pack('<I', 1))                   # SELECTOBJECT brush
    rec(37, struct.pack('<I', 0x80000008))          # SELECTOBJECT NULL_PEN
    rec(43, struct.pack('<iiii', 0, 0, 100, 100))   # RECTANGLE
    rec(39, struct.pack('<IIII', 2, 0, colorref(RED), 0))
    rec(37, struct.pack('<I', 2))
    rec(42, struct.pack('<iiii', 110, 10, 190, 90)) # ELLIPSE
    rec(38, struct.pack('<IIiiI', 3, 0, 4, 0, colorref(GREEN)))  # CREATEPEN #3, width 4
    rec(37, struct.pack('<I', 3))
    rec(37, struct.pack('<I', 0x80000005))          # NULL_BRUSH
    pts = [(10, 96), (100, 96), (190, 96)]
    rec(87, struct.pack('<iiiiI', 10, 94, 190, 98, len(pts)) + b''.join(struct.pack('<hh', *p) for p in pts))  # POLYLINE16
    rec(40, struct.pack('<I', 1)); rec(40, struct.pack('<I', 2)); rec(40, struct.pack('<I', 3))
    rec(14, struct.pack('<III', 0, 16, 20))         # EOF
    body = b''.join(recs)
    nrec = len(recs) + 1
    hdr_size = 88
    total = hdr_size + len(body)
    # Bounds in device units, Frame in 0.01 mm: 200 x 100 units at 96 dpi -> 52.92 x 26.46 mm
    header = struct.pack('<iiii', 0, 0, 199, 99) + struct.pack('<iiii', 0, 0, 5292, 2646) + \
        struct.pack('<IIIIHHIIIiiii', 0x464D4520, 0x10000, total, nrec, 4, 0, 0, 0, 0, 1024, 768, 271, 203)
    return struct.pack('<II', 1, hdr_size) + header + body

def wmf():
    recs = []
    def rec(fn, *words):
        data = b''.join(struct.pack('<h', w) if isinstance(w, int) and w < 0x8000 else struct.pack('<H', w & 0xFFFF) for w in words)
        recs.append(struct.pack('<IH', 3 + len(data) // 2, fn) + data)
    def cr(c):  # COLORREF as two words (low, high)
        v = colorref(c); return (v & 0xFFFF, v >> 16)
    rec(0x020B, 0, 0)            # SETWINDOWORG (y, x)
    rec(0x020C, 100, 200)        # SETWINDOWEXT (y, x)
    rec(0x02FC, 0, *cr(BLUE), 0) # CREATEBRUSHINDIRECT -> object 0
    rec(0x012D, 0)               # SELECTOBJECT 0
    rec(0x02FA, 5, 0, 0, *cr(BLUE))  # CREATEPENINDIRECT PS_NULL -> object 1
    rec(0x012D, 1)
    rec(0x041B, 100, 100, 0, 0)  # RECTANGLE bottom, right, top, left
    rec(0x02FC, 0, *cr(RED), 0)  # brush -> object 2
    rec(0x012D, 2)
    rec(0x0418, 90, 190, 10, 110)  # ELLIPSE bottom, right, top, left
    rec(0x02FA, 0, 4, 0, *cr(GREEN))  # pen width 4 -> object 3
    rec(0x012D, 3)
    rec(0x02FC, 1, *cr(GREEN), 0)  # BS_NULL brush -> object 4
    rec(0x012D, 4)
    rec(0x0325, 3, 10, 96, 100, 96, 190, 96)  # POLYLINE count, points
    for i in range(5): rec(0x01F0, i)
    rec(0x0000)
    body = b''.join(recs)
    size_words = (18 + len(body)) // 2
    maxrec = max(len(r) // 2 for r in recs)
    header = struct.pack('<HHHIHIH', 1, 9, 0x0300, size_words, 5, maxrec, 0)
    # placeable header: bbox 0,0,200,100 at 96 units per inch
    ph = struct.pack('<IHhhhhHI', 0x9AC6CDD7, 0, 0, 0, 200, 100, 96, 0)
    chk = 0
    for (w,) in struct.iter_unpack('<H', ph): chk ^= w
    return ph + struct.pack('<H', chk) + header + body

out = sys.argv[1] if len(sys.argv) > 1 else 'journeys/fixtures'
open(os.path.join(out, 'shapes.emf'), 'wb').write(emf())
open(os.path.join(out, 'shapes.wmf'), 'wb').write(wmf())
