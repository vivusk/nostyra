"""Minimal 6502 byte emitter shared by the self-written test-ROM builders (NES,
Atari 2600 / 7800, Lynx). No external toolchain: hand-assembled opcodes with the
mnemonics in the callers' comments, output byte-for-byte reproducible.

65C02-only opcodes (Lynx: BRA, STZ) are emitted by the caller with `emit`.
"""


class Asm:
    """Minimal byte emitter with labels, 8-bit relative branches and absolute jumps."""

    def __init__(self, origin):
        self.origin = origin
        self.buf = bytearray()
        self.labels = {}
        self.fixups = []  # (offset, label, kind) with kind "rel" or "abs"

    @property
    def pc(self):
        return self.origin + len(self.buf)

    def label(self, name):
        self.labels[name] = self.pc

    def emit(self, *data):
        self.buf.extend(data)

    def abs_op(self, opcode, addr):
        self.emit(opcode, lo(addr), hi(addr))

    def branch(self, opcode, name):
        self.emit(opcode, 0)
        self.fixups.append((len(self.buf) - 1, name, "rel"))

    def jmp(self, name):
        self.emit(0x4C, 0, 0)
        self.fixups.append((len(self.buf) - 2, name, "abs"))

    def resolve(self):
        for off, name, kind in self.fixups:
            target = self.labels[name]
            if kind == "abs":
                self.buf[off:off + 2] = bytes([lo(target), hi(target)])
                continue
            rel = target - (self.origin + off + 1)
            if not -128 <= rel <= 127:
                raise ValueError(f"branch to {name} out of range")
            self.buf[off] = rel & 0xFF
        return bytes(self.buf)


def lo(v):
    return v & 0xFF


def hi(v):
    return (v >> 8) & 0xFF
