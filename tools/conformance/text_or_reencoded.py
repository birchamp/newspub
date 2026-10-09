#!/usr/bin/env python3
"""Checks that extracted PDF text contains EXPECTED, either as is or re-encoded one-to-one.

macOS's PostScript conversion subsets fonts with a custom encoding, so pdftotext of a printed page returns
the right glyph sequence under different character codes. A consistent one-to-one character mapping that
turns some line into EXPECTED shows the same text was printed.
Usage: text_or_reencoded.py EXPECTED < extracted.txt
"""
import sys


def matches(line: str, want: str) -> bool:
    for start in range(len(line) - len(want) + 1):
        seg = line[start:start + len(want)]
        fwd, back = {}, {}
        if all(fwd.setdefault(a, b) == b and back.setdefault(b, a) == a for a, b in zip(seg, want)):
            return True
    return False


want = sys.argv[1]
text = sys.stdin.read()
if want in text:
    print(f"found {want!r}")
    sys.exit(0)
for line in text.splitlines():
    if matches(line, want):
        print(f"found {want!r} re-encoded as {line.strip()!r}")
        sys.exit(0)
print(f"{want!r} not found")
sys.exit(1)
