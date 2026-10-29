#!/usr/bin/env python3
"""Builds the Nostyra PSP test content: self-written, GPLv3, no PSPSDK toolchain.

Same approach as the other test ROM builders: Python 3 only, byte-for-byte
reproducible output, hand-assembled Allegrex (MIPS32 little-endian) with mnemonics
in the comments. PPSSPP HLE-boots it (no firmware).

What the program does (each feature backs a runner test on PPSSPP):
- Video: sceDisplaySetMode(0, 480, 272) + SetFrameBuf(VRAM, 512, 8888). Every frame
  the CPU fills the top half red and the bottom half blue whose green channel is the
  frame counter (mod 64), so consecutive frames differ and an upside-down image shows.
- RAM: the frame counter word after the tone buffer (RA address = address - 0x08000000).
- Input: sceCtrlPeekBufferPositive every frame (analog sampling): Cross held -> top
  half green; stick X > 192 -> bottom half yellow.
- Audio: one stereo channel, a 512-frame square wave (~689 Hz) queued every frame.
- Save: at boot an AUTOSAVE through sceUtilitySavedata (no dialog) writes
  PSP/SAVEDATA/NSTY00001DATA00/DATA.BIN (16 bytes) on the memory stick; the frame
  loop drives the utility (Update while running, Shutdown when finished).

Outputs (next to this script's parent folder): nostyra-test-psp.elf,
nostyra-test-psp.iso, nostyra-test-psp.cso, nostyra-test-psp/EBOOT.PBP.

Usage: python build-nostyra-test-psp.py [output folder]
"""
import hashlib
import pathlib
import struct
import sys

from psp_test_allegrex import (A0, A1, A2, A3, BASE, S0, S1, S2, S3, S4, S5, S6, T0,
                               T1, T2, T3, T4, T5, T6, T7, V0, ZERO, Allegrex, Imports,
                               elf, module_info)
from psp_test_disc import cso_image, eboot_pbp, game_sfo, umd_iso
from psp_test_savedata import savedata_param

DATA = BASE + 0x1000  # imports, module info and buffers live after the code
VRAM = 0x04000000
VRAM_UNCACHED = 0x44000000
WIDTH, HEIGHT, STRIDE = 480, 272, 512
# 8888 pixels are 0xAABBGGRR words.
RED, GREEN, BLUE, YELLOW = 0xFF0000FF, 0xFF00FF00, 0xFFFF0000, 0xFF00FFFF
CROSS = 0x4000
STICK_THRESHOLD = 192
AUDIO_FRAMES = 512  # per sceAudioOutputBlocking (multiple of 64)
SERIAL, DISC_ID, TITLE = "NSTY-00001", "NSTY00001", "Nostyra Test PSP"


def imports():
    imp = Imports()
    imp.stub("sceDisplay", "SetMode", 0x0E20F177)
    imp.stub("sceDisplay", "SetFrameBuf", 0x289D82FE)
    imp.stub("sceDisplay", "WaitVblankStart", 0x984C27E7)
    imp.stub("sceCtrl", "SetSamplingMode", 0x1F4011E6)
    imp.stub("sceCtrl", "PeekBufferPositive", 0x3A622550)
    imp.stub("sceAudio", "ChReserve", 0x5EC81C55)
    imp.stub("sceAudio", "OutputBlocking", 0x136CAF51)
    imp.stub("sceUtility", "SavedataInitStart", 0x50C4CD57)
    imp.stub("sceUtility", "SavedataUpdate", 0xD4B95FFB)
    imp.stub("sceUtility", "SavedataGetStatus", 0x8874DBE0)
    imp.stub("sceUtility", "SavedataShutdownStart", 0x9790B33C)
    return imp


SAVE_DATA = b"NOSTYRA TEST SAV"  # 16 bytes written to DATA.BIN


def tone():
    """AUDIO_FRAMES stereo s16 frames of a square wave, period 64 frames."""
    return b"".join(struct.pack("<hh", v, v)
                    for v in (6000 if (n // 32) % 2 else -6000 for n in range(AUDIO_FRAMES)))


def program(stubs, pad_buf, tone_buf, save_param, counter_buf):
    m = Allegrex(BASE)
    m.labels.update(stubs)
    m.move(A0, ZERO)
    m.li(A1, WIDTH)
    m.li(A2, HEIGHT)
    m.jal("SetMode")  # sceDisplaySetMode(0, 480, 272)
    m.li(A0, VRAM)
    m.li(A1, STRIDE)
    m.li(A2, 3)  # PSP_DISPLAY_PIXEL_FORMAT_8888
    m.li(A3, 1)  # PSP_DISPLAY_SETBUF_NEXTFRAME
    m.jal("SetFrameBuf")
    m.li(A0, 1)
    m.jal("SetSamplingMode")  # analog
    m.addiu(A0, ZERO, -1)  # next free channel
    m.li(A1, AUDIO_FRAMES)
    m.move(A2, ZERO)  # stereo
    m.jal("ChReserve")
    m.move(S3, V0)  # s3 = audio channel
    m.li(A0, save_param)
    m.jal("SavedataInitStart")  # AUTOSAVE, runs over the next frames
    m.move(S0, ZERO)  # s0 = frame counter
    m.label("frame")
    m.li(A0, pad_buf)
    m.li(A1, 1)
    m.jal("PeekBufferPositive")
    m.li(T0, pad_buf)
    m.lw(S1, 4, T0)  # buttons
    m.lbu(S2, 8, T0)  # stick X
    m.li(S4, RED)
    m.andi(T1, S1, CROSS)
    m.beq(T1, ZERO, "top_done")
    m.li(S4, GREEN)
    m.label("top_done")
    m.andi(T1, S0, 0x3F)
    m.sll(T1, T1, 8)
    m.li(S5, BLUE)
    m.or_(S5, S5, T1)  # blue | counter << 8
    m.sltiu(T1, S2, STICK_THRESHOLD + 1)
    m.bne(T1, ZERO, "bottom_done")
    m.li(S5, YELLOW)
    m.label("bottom_done")
    m.li(T2, VRAM_UNCACHED)  # t2 = row start
    m.move(T3, ZERO)  # t3 = y
    m.label("row")
    m.move(T4, S4)
    m.slti(T5, T3, HEIGHT // 2)
    m.bne(T5, ZERO, "row_colour")
    m.move(T4, S5)
    m.label("row_colour")
    m.li(T6, WIDTH)
    m.move(T7, T2)
    m.label("pixel")
    m.sw(T4, 0, T7)
    m.addiu(T7, T7, 4)
    m.addiu(T6, T6, -1)
    m.bne(T6, ZERO, "pixel")
    m.addiu(T2, T2, STRIDE * 4)
    m.addiu(T3, T3, 1)
    m.slti(T5, T3, HEIGHT)
    m.bne(T5, ZERO, "row")
    m.move(A0, S3)
    m.li(A1, 0x8000)  # max volume
    m.li(A2, tone_buf)
    m.jal("OutputBlocking")
    m.jal("SavedataGetStatus")
    m.move(S6, V0)  # 2 = running, 3 = finished
    m.li(T0, 2)
    m.bne(S6, T0, "save_not_running")
    m.li(A0, 1)
    m.jal("SavedataUpdate")
    m.label("save_not_running")
    m.li(T0, 3)
    m.bne(S6, T0, "save_done")
    m.jal("SavedataShutdownStart")
    m.label("save_done")
    m.jal("WaitVblankStart")
    m.addiu(S0, S0, 1)
    m.li(T0, counter_buf)
    m.sw(S0, 0, T0)  # frame counter word in RAM (RetroAchievements: address - 0x08000000)
    m.j("frame")
    return m.code()


def build_elf():
    imp_blob, stubs, stub_range = imports().layout(DATA)
    modinfo_at = len(imp_blob) + (-len(imp_blob) % 4)
    modinfo = module_info("NostyraTestPsp", stub_range)
    buffers_at = DATA + modinfo_at + len(modinfo)
    pad_buf = buffers_at + (-buffers_at % 16)
    save_buf = pad_buf + 16
    save_param = save_buf + 16
    tone_buf = save_param + 1536
    counter_buf = tone_buf + 4 * AUDIO_FRAMES  # one zeroed word after the tone
    code = program(stubs, pad_buf, tone_buf, save_param, counter_buf)
    assert len(code) <= DATA - BASE, "code overlaps the data area"
    data = imp_blob.ljust(modinfo_at, b"\0") + modinfo
    data = data.ljust(save_buf - DATA, b"\0") + SAVE_DATA
    data += savedata_param(save_buf, len(SAVE_DATA), DISC_ID, TITLE)
    data = data.ljust(tone_buf - DATA, b"\0") + tone() + bytes(16)
    segment = code.ljust(DATA - BASE, b"\0") + data
    return elf(segment, BASE, DATA - BASE + modinfo_at)


def sfo(category):
    return game_sfo(category, DISC_ID, TITLE)


def main():
    out_dir = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else
                           pathlib.Path(__file__).resolve().parent.parent)
    program_elf = build_elf()
    image = umd_iso(SERIAL, sfo("UG"), program_elf)
    outputs = {
        "nostyra-test-psp.elf": program_elf,
        "nostyra-test-psp.iso": image,
        "nostyra-test-psp.cso": cso_image(image),
        "nostyra-test-psp/EBOOT.PBP": eboot_pbp(sfo("MG"), program_elf),
    }
    for name, data in outputs.items():
        path = out_dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        print(f"{hashlib.sha1(data).hexdigest()}  {name}")


if __name__ == "__main__":
    main()
