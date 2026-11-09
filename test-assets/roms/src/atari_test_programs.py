"""6502 / 65C02 programs of the self-written Atari test ROMs (built by
build-nostyra-test-atari-roms.py). Each program backs the runner's real-core checks:
- Video: one full-screen colour whose luminance (2600 / 7800) steps every frame or
  whose green level (Lynx) follows a free-running counter (consecutive frames always
  differ, frames are deterministic).
- Input: fire / button (RetroPad B) switches to another hue; up and the console
  reset switch (2600 / 7800) switch to their own hues.
- Audio: a constant square tone (TIA channel 0 on 2600 / 7800; Lynx: Mikey
  channel 0 output toggled by the CPU).
- RAM: a frame counter (+1 per frame) for the RetroAchievements checks:
  2600 RIOT RAM $80 (RA address $00), 7800 $1800, Lynx $0082.
"""
from mos6502_emitter import Asm, lo, hi

# TIA (2600 and 7800 share the sound registers at $15 / $17 / $19).
VSYNC, VBLANK, WSYNC, COLUBK, INPT4 = 0x00, 0x01, 0x02, 0x09, 0x0C
AUDC0, AUDF0, AUDV0 = 0x15, 0x17, 0x19
SWCHA, SWCHB = 0x0280, 0x0282  # RIOT: joystick nibbles, console switches (active low)
HUE_PLAIN, HUE_FIRE, HUE_UP, HUE_RESET = 0x40, 0xC0, 0x80, 0x20


def _tia_tone(a):
    """Pure tone on TIA channel 0: AUDC0 = 4 (square), AUDF0 = $10, AUDV0 = 15."""
    for reg, value in ((AUDC0, 0x04), (AUDF0, 0x10), (AUDV0, 0x0F)):
        a.emit(0xA9, value)           # LDA #value
        a.emit(0x85, reg)             # STA reg


def _pick_hue(a, fire_zp):
    """X = hue for the held input (priority: reset > up > fire > none)."""
    a.emit(0xA2, HUE_PLAIN)           # LDX #plain
    a.emit(0x24, fire_zp)             # BIT INPT4     (N = bit 7, 0 = pressed)
    a.branch(0x30, "no_fire")         # BMI no_fire
    a.emit(0xA2, HUE_FIRE)            # LDX #fire
    a.label("no_fire")
    a.abs_op(0xAD, SWCHA)             # LDA SWCHA
    a.emit(0x29, 0x10)                # AND #$10      (P0 up, active low)
    a.branch(0xD0, "no_up")           # BNE no_up
    a.emit(0xA2, HUE_UP)              # LDX #up
    a.label("no_up")
    a.abs_op(0xAD, SWCHB)             # LDA SWCHB
    a.emit(0x29, 0x01)                # AND #$01      (console reset, active low)
    a.branch(0xD0, "no_reset")        # BNE no_reset
    a.emit(0xA2, HUE_RESET)           # LDX #reset
    a.label("no_reset")


def _wsync_lines(a, count, name):
    a.emit(0xA2, count)               # LDX #count
    a.label(name)
    a.emit(0x85, WSYNC)               # STA WSYNC
    a.emit(0xCA)                      # DEX
    a.branch(0xD0, name)              # BNE name


def atari2600_program(origin):
    """4K cart at $F000: NTSC frame (3 VSYNC + 37 VBLANK + 192 visible + 30 overscan)."""
    frame, lum = 0x80, 0x81           # RIOT RAM
    a = Asm(origin)
    a.label("reset")
    a.emit(0x78, 0xD8)                # SEI, CLD
    a.emit(0xA9, 0x00, 0xA2, 0x00)    # LDA #0, LDX #0
    a.label("clear")
    a.emit(0x95, 0x00)                # STA $00,X     (TIA + RAM $00..$FF)
    a.emit(0xE8)                      # INX
    a.branch(0xD0, "clear")           # BNE clear
    a.emit(0xA2, 0xFF, 0x9A)          # LDX #$FF, TXS
    _tia_tone(a)
    a.label("frame")
    a.emit(0xA9, 0x02)                # LDA #2
    a.emit(0x85, VBLANK, 0x85, VSYNC)  # STA VBLANK, STA VSYNC
    a.emit(0x85, WSYNC, 0x85, WSYNC, 0x85, WSYNC)  # 3 lines of VSYNC
    a.emit(0xA9, 0x00, 0x85, VSYNC)   # LDA #0, STA VSYNC
    _wsync_lines(a, 37, "vblank")
    a.emit(0xE6, frame)               # INC frame
    a.emit(0xA5, frame, 0x0A)         # LDA frame, ASL
    a.emit(0x29, 0x0E, 0x85, lum)     # AND #$0E, STA lum (new luminance every frame)
    _pick_hue(a, INPT4)
    a.emit(0x8A, 0x05, lum)           # TXA, ORA lum
    a.emit(0x85, COLUBK)              # STA COLUBK
    a.emit(0xA9, 0x00, 0x85, VBLANK)  # LDA #0, STA VBLANK (beam on)
    _wsync_lines(a, 192, "visible")
    a.emit(0xA9, 0x02, 0x85, VBLANK)  # LDA #2, STA VBLANK
    _wsync_lines(a, 30, "overscan")
    a.jmp("frame")                    # JMP frame
    return a.resolve(), a.labels


def atari7800_program(origin):
    """16K cart at $C000: MARIA DMA on with empty display lists, BACKGRND per frame."""
    inptctrl, backgrnd, mstat, dpph, dppl, ctrl = 0x01, 0x20, 0x28, 0x2C, 0x30, 0x3C
    frame, lum, dll, dl = 0x1800, 0x1801, 0x1810, 0x1900
    a = Asm(origin)
    a.label("reset")
    a.emit(0x78, 0xD8)                # SEI, CLD
    a.emit(0xA9, 0x07, 0x85, inptctrl)  # LDA #7, STA INPTCTRL (lock 7800 mode)
    a.emit(0xA9, 0x60, 0x85, ctrl)    # LDA #$60, STA CTRL (DMA off while building)
    a.emit(0xA2, 0xFF, 0x9A)          # LDX #$FF, TXS
    a.emit(0xA2, 0x00)                # LDX #0
    a.label("dll")                    # 16 zones x 16 lines, all -> the empty DL
    for value in (0x0F, hi(dl), lo(dl)):
        a.emit(0xA9, value)           # LDA #value
        a.abs_op(0x9D, dll)           # STA dll,X
        a.emit(0xE8)                  # INX
    a.emit(0xE0, 48)                  # CPX #48
    a.branch(0xD0, "dll")             # BNE dll
    a.emit(0xA9, 0x00)                # LDA #0
    for addr in (dl, dl + 1, frame):
        a.abs_op(0x8D, addr)          # STA (DL end marker, frame counter)
    a.emit(0xA9, hi(dll), 0x85, dpph)  # LDA #>dll, STA DPPH
    a.emit(0xA9, lo(dll), 0x85, dppl)  # LDA #<dll, STA DPPL
    _tia_tone(a)
    a.emit(0xA9, 0x40, 0x85, ctrl)    # LDA #$40, STA CTRL (DMA on)
    a.label("loop")
    a.label("in_vblank")
    a.emit(0x24, mstat)               # BIT MSTAT     (N = vertical blank)
    a.branch(0x30, "in_vblank")       # BMI in_vblank
    a.label("in_frame")
    a.emit(0x24, mstat)               # BIT MSTAT
    a.branch(0x10, "in_frame")        # BPL in_frame
    a.abs_op(0xEE, frame)             # INC frame
    a.abs_op(0xAD, frame)             # LDA frame
    a.emit(0x0A, 0x29, 0x0E)          # ASL, AND #$0E
    a.abs_op(0x8D, lum)               # STA lum
    _pick_hue(a, INPT4)               # one-button mode: B or A clears INPT4 bit 7
    a.emit(0x8A)                      # TXA
    a.abs_op(0x0D, lum)               # ORA lum
    a.emit(0x85, backgrnd)            # STA BACKGRND
    a.jmp("loop")                     # JMP loop
    a.label("nmi")
    a.emit(0x40)                      # RTI
    return a.resolve(), a.labels


def _all_pens(a, register, name):
    """A -> the 16 palette registers from `register` on (the screen RAM content is
    unknown after a cartridge boot, so every pen shows the colour)."""
    a.emit(0xA2, 0x0F)                # LDX #15
    a.label(name)
    a.abs_op(0x9D, register)          # STA register,X
    a.emit(0xCA)                      # DEX
    a.branch(0x10, name)              # BPL name


def lynx_program(origin):
    """Lynx program (65C02), at $0200 (BS93 `.o`) or $FB68 (`.lnx`, run by the boot
    loader): screen at $C000; pens 0 / 15 get a green level from a free-running
    counter, every pen red (joystick: blue) at each end-of-frame flag (timer 2,
    polled with interrupts off), which also counts the frame for RA."""
    joystick, intrst, intset, tim2ctla = 0xFCB0, 0xFD80, 0xFD81, 0xFD09
    dispadrl, dispadrh, aud0outval, green0, bluered0 = 0xFD94, 0xFD95, 0xFD22, 0xFDA0, 0xFDB0
    tone, level, frame = 0x81, 0x83, 0x82  # zero page
    a = Asm(origin)
    a.emit(0x78)                      # SEI
    a.emit(0xA9, 0x00)                # LDA #0
    a.abs_op(0x8D, dispadrl)          # STA DISPADRL
    a.emit(0xA9, 0xC0)                # LDA #$C0
    a.abs_op(0x8D, dispadrh)          # STA DISPADRH  (screen = $C000)
    # Display timing: line timer 0 = $9E, frame timer 2 linked to it, raising the
    # end-of-frame flag. 84 lines ($53) give one Lynx frame per 60 Hz Handy frame
    # (measured; the usual 105 lines repeat every 5th frame under Handy).
    for reg, value in ((0xFD00, 0x9E), (0xFD01, 0x18), (0xFD08, 0x53), (tim2ctla, 0x9F)):
        a.emit(0xA9, value)           # LDA #value
        a.abs_op(0x8D, reg)           # STA TIM0BKUP / TIM0CTLA / TIM2BKUP / TIM2CTLA
    a.emit(0xA9, 0x7F, 0x85, level)   # LDA #$7F, STA level
    a.emit(0x64, tone, 0x64, frame)   # STZ tone, STZ frame (65C02)
    a.label("wait")
    a.emit(0xC6, tone)                # DEC tone
    a.branch(0xD0, "no_toggle")       # BNE no_toggle (every 256 polls, ~460 Hz)
    a.emit(0xA5, level, 0x49, 0xFF)   # LDA level, EOR #$FF
    a.emit(0x85, level)               # STA level
    a.abs_op(0x8D, aud0outval)        # STA AUD0OUTVAL (+127 / -128)
    a.label("no_toggle")
    # Green follows the free-running poll counter (every displayed frame differs: the
    # end-of-frame flag does not tick once per 60 Hz frame under Handy's HLE boot).
    a.emit(0xA5, tone)                # LDA tone
    a.abs_op(0x8D, green0)            # STA GREEN0    (pen 0: `.o` screen RAM)
    a.abs_op(0x8D, green0 + 15)       # STA GREENF    (pen 15: erased cart RAM)
    a.abs_op(0xAD, intset)            # LDA INTSET
    a.emit(0x29, 0x04)                # AND #$04      (timer 2 = end of frame)
    a.branch(0xF0, "wait")            # BEQ wait
    a.abs_op(0x8D, intrst)            # STA INTRST    (clear it)
    a.emit(0xE6, frame)               # INC frame     (RetroAchievements counter)
    a.emit(0xA9, 0x0F)                # LDA #$0F      (red)
    a.abs_op(0xAC, joystick)          # LDY JOYSTICK  (any bit = pressed)
    a.branch(0xF0, "store")           # BEQ store
    a.emit(0xA9, 0xF0)                # LDA #$F0      (blue)
    a.label("store")
    _all_pens(a, bluered0, "bluered")
    a.emit(0x80, 0)                   # BRA wait (65C02; fixed up below)
    a.fixups.append((len(a.buf) - 1, "wait", "rel"))
    return a.resolve(), a.labels
