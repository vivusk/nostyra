"""ARM7 program of the Nostyra DS test ROMs (see build-nostyra-test-nds.py).

PSG channel 8 plays a ~440 Hz square; every loop it reads EXTKEYIN pen-down (bit 6)
and the touch-screen controller (SPI device 2, cmds 0xD0 = X, 0x90 = Y, 7 high
bits) -> shared word SHARED = 0x8000 | y7 << 7 | x7 (0 while the pen is up).
ROM B also keeps a boot counter in EEPROM bytes 0 (counter) / 1 (0xA5 marker) over
AUXSPI (slot 1 belongs to the ARM7 after direct boot, EXMEMCNT bit 11).
"""
from arm_emitter import (COND_AL, COND_NE, OP_ADD, OP_AND, OP_CMP, OP_MOV, OP_ORR, OP_TST,
                         SHIFT_LSL, Arm, dpi, dpr, ldrh, str_, strh)
from nds_test_arm9 import IO, SHARED, wait_vcount

ARM7_RAM = 0x037F8000


def spi_xfer(a, base_reg, cnt, data_reg=None, data=None):
    """One SPI byte: CNT = cnt, DATA = data (reg or imm), wait busy, r2 = reply."""
    a.ldr_const(12, cnt)
    a.emit(strh(12, base_reg))
    if data_reg is None:
        a.emit(dpi(OP_MOV, 12, 0, data))
        data_reg = 12
    a.emit(strh(data_reg, base_reg, 2))
    name = f"busy{len(a.words)}"
    a.label(name)
    a.emit(ldrh(2, base_reg))
    a.emit(dpi(OP_TST, 0, 2, 0x80, s=1))
    a.b(COND_NE, name)
    a.emit(ldrh(2, base_reg, 2))


def tsc_read(a, cmd, out):
    """TSC channel read on SPI (r1 = SPICNT): out = 7 high bits of the 12-bit value."""
    spi_xfer(a, 1, 0x8A01, data=cmd)              # device 2 (TSC), hold, 2 MHz
    spi_xfer(a, 1, 0x8A01, data=0)
    a.emit(dpi(OP_AND, out, 2, 0x7F))
    spi_xfer(a, 1, 0x8201, data=0)                # last byte releases chip select


def eeprom_boot_counter(a):
    """r7 = EEPROM[0] + 1 (0 when EEPROM[1] != 0xA5), written back with the marker."""
    for _ in range(3):                            # let the frontend SRAM install first
        wait_vcount(a, True)
        wait_vcount(a, False)
    a.ldr_const(1, IO + 0x1A0)                    # AUXSPICNT (ARM7 owns slot 1)
    hold, last = 0xA040, 0xA000                   # enable + SPI mode (+ CS hold)
    for byte in (0x03, 0x00, 0x00):               # READ @0
        spi_xfer(a, 1, hold, data=byte)
    spi_xfer(a, 1, hold, data=0)
    a.emit(dpr(OP_MOV, 7, 0, 2))                  # r7 = byte 0
    spi_xfer(a, 1, last, data=0)                  # r2 = byte 1, CS released
    a.emit(dpi(OP_CMP, 0, 2, 0xA5, s=1))
    a.emit(dpi(OP_MOV, 7, 0, 0, cond=COND_NE))
    a.emit(dpi(OP_ADD, 7, 7, 1))
    a.emit(dpi(OP_AND, 7, 7, 0xFF))
    spi_xfer(a, 1, last, data=0x06)               # WREN
    for byte in (0x02, 0x00, 0x00):               # WRITE @0
        spi_xfer(a, 1, hold, data=byte)
    spi_xfer(a, 1, hold, data_reg=7)
    spi_xfer(a, 1, last, data=0xA5)               # release -> core save write
    a.emit(str_(7, 11, 4))


def arm7_program(with_save):
    """The ARM7 program (with the EEPROM boot counter for ROM B)."""
    a = Arm(ARM7_RAM)
    a.ldr_const(0, IO)
    a.ldr_const(11, SHARED)
    a.ldr_const(1, IO + 0x500)
    a.ldr_const(3, 0x807F)
    a.emit(strh(3, 1))                            # SOUNDCNT: master on, volume 127
    a.ldr_const(1, IO + 0x480)                    # channel 8 (PSG)
    a.ldr_const(3, 0x10000 - 16756991 // (8 * 440))
    a.emit(strh(3, 1, 0x08))                      # SOUND8TMR ~440 Hz
    a.ldr_const(3, 0xE340007F)
    a.emit(str_(3, 1))                            # start, PSG, duty 4/8, pan 64, vol 127
    if with_save:
        eeprom_boot_counter(a)
    a.ldr_const(10, IO + 0x136)                   # EXTKEYIN
    a.ldr_const(1, IO + 0x1C0)                    # SPICNT
    a.label("loop")
    a.emit(ldrh(3, 10))
    a.emit(dpi(OP_TST, 0, 3, 0x40, s=1))          # bit 6 = 1: pen up
    a.b(COND_NE, "up")
    tsc_read(a, 0xD0, 4)                          # X
    tsc_read(a, 0x90, 5)                          # Y
    a.ldr_const(3, 0x8000)
    a.emit(dpr(OP_ORR, 3, 3, 4))
    a.emit(dpr(OP_ORR, 3, 3, 5, SHIFT_LSL, 7))
    a.emit(str_(3, 11))
    a.b(COND_AL, "loop")
    a.label("up")
    a.emit(dpi(OP_MOV, 3, 0, 0))
    a.emit(str_(3, 11))
    a.b(COND_AL, "loop")
    return a.resolve()
