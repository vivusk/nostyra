"""DOL writer for the GameCube / Wii test programs.

A DOL is a 0x100-byte big-endian header (7 text + 11 data section slots: file
offsets at 0x00/0x1C, load addresses at 0x48/0x64, sizes at 0x90/0xAC; BSS address
0xD8, BSS size 0xDC, entry 0xE0) followed by the section bytes. Dolphin's DolReader
refuses a section whose file offset or size is not 32-byte aligned, so every
section here starts and ends on a 32-byte boundary (zero padding).
"""
import struct

HEADER_SIZE = 0x100
TEXT_SLOTS, DATA_SLOTS = 7, 11
ALIGN = 32


def _pad(data):
    return data + b"\0" * (-len(data) % ALIGN)


def dol(texts, entry, datas=(), bss=(0, 0)):
    """`texts` / `datas`: lists of (load address, bytes); `bss`: (address, size)."""
    assert 1 <= len(texts) <= TEXT_SLOTS and len(datas) <= DATA_SLOTS
    header = bytearray(HEADER_SIZE)
    body = bytearray()
    sections = [(0x00, 0x48, 0x90, i, s) for i, s in enumerate(texts)]
    sections += [(0x1C, 0x64, 0xAC, i, s) for i, s in enumerate(datas)]
    for off_at, addr_at, size_at, index, (addr, data) in sections:
        assert addr % ALIGN == 0, f"section load address {addr:#x} not 32-byte aligned"
        data = _pad(data)
        struct.pack_into(">I", header, off_at + 4 * index, HEADER_SIZE + len(body))
        struct.pack_into(">I", header, addr_at + 4 * index, addr)
        struct.pack_into(">I", header, size_at + 4 * index, len(data))
        body += data
    struct.pack_into(">III", header, 0xD8, bss[0], bss[1], entry)
    return bytes(header) + bytes(body)
