#!/usr/bin/env python3
"""Builds the Nostyra GameCube and Wii test DOLs: self-written, GPLv3, no devkitPPC.

Same approach as the other test ROM builders: Python 3 only, byte-for-byte
reproducible output, hand-assembled PowerPC (ppc_emitter.py). The program
(gc_test_program.py) pokes VI / SI / AI registers directly: colour bars and two
blocks in a YUY2 XFB (frame block changes every field, pad block green while A is
held), the GC pad words and a field counter mirrored into MEM1, an AI-DMA square
wave. gc_test_dol.py writes the DOL (32-byte aligned sections, as Dolphin needs).
No Nintendo code or data.

Outputs (next to this script's parent folder):
- nostyra-test.dol      GameCube executable
- nostyra-test-wii.dol  same program + one never-executed `mtspr HID4, r0` word,
                        which makes Dolphin (and Nostyra's scanner) treat it as Wii

Usage: python build-nostyra-test-gc.py [output folder]
"""
import hashlib
import pathlib
import sys

from gc_test_dol import dol
from gc_test_program import TEXT, program

OUTPUTS = (("nostyra-test.dol", False), ("nostyra-test-wii.dol", True))


def main():
    default_out = pathlib.Path(__file__).resolve().parent.parent
    out_dir = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_out
    out_dir.mkdir(parents=True, exist_ok=True)
    for name, wii in OUTPUTS:
        data = dol([(TEXT, program(wii))], entry=TEXT)
        (out_dir / name).write_bytes(data)
        print(f"wrote {out_dir / name} ({len(data)} bytes, sha1 {hashlib.sha1(data).hexdigest()})")


if __name__ == "__main__":
    main()
