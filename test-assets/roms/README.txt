nostyra-test.sfc — Nostyra SNES test ROM
=========================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE). Redistribution in this
         repository and in CI is permitted by that license.
Source:  ./src/build-nostyra-test-rom.py (Python 3, no external toolchain;
         output is byte-for-byte reproducible)
Rebuild: python test-assets/roms/src/build-nostyra-test-rom.py

What it does (each feature backs a runner test):
- Video: backdrop color ramps with a frame counter (deterministic frames).
- Input: holding any joypad button sets the blue channel (unpressed: blue = 0).
- Audio: SPC700 plays a ~440 Hz square wave on voice 0.
- SRAM: 2 KiB battery RAM; byte 0 is a boot counter (+1 per reset), so a
  correctly persisted .srm increments across runs.

Format: 32 KiB LoROM, NTSC, header title "NOSTYRA TEST ROM", no copier header.
Note: Snes9x logs "LoROM: Corrupt" for this ROM. That label only means the
size is below 1 Mbit (Snes9x prints sizes in Mbit); the checksum is reported
OK and the ROM runs correctly.

Why not the 240p Test Suite (first choice in the M0 plan)?
- Its code is GPLv2-or-later (compatible), but the SNES build embeds artwork
  taken from commercial games (e.g. sonicback.bmp, fireshark.bmp, kiki.bmp),
  which the GPL notice cannot license. Project rule: no copyrighted assets in
  the repo. No prebuilt SNES ROM is published in its git repo either.
- Plan fallback applied: self-written minimal ROM. Developers may still use
  the 240p Test Suite locally for manual video/lag tests (do not commit it).

nostyra-test.nes — Nostyra NES test ROM
=======================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-nes-rom.py (Python 3, no external toolchain;
         output is byte-for-byte reproducible, the script prints its SHA1)
Rebuild: python test-assets/roms/src/build-nostyra-test-nes-rom.py

What it does (each feature backs a runner test on the mesen core):
- Video: the backdrop colour cycles through 12 hues, one step per frame.
- Input: holding any joypad-1 button switches to the brighter colour row.
- Audio: APU pulse 1 plays a ~440 Hz square wave.
- SRAM: 8 KiB battery PRG-RAM; byte 0 is a boot counter (+1 per reset),
  byte 1 a constant marker ($A5); nothing else is written.

Format: iNES, mapper 0 (NROM-128), 16 KiB PRG + 8 KiB zero-filled CHR (no
artwork), battery, vertical mirroring. 24 592 bytes.

nostyra-test.gb / nostyra-test.gbc — Nostyra Game Boy / Game Boy Color test ROMs
==================================================================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-gb-rom.py (Python 3, no external toolchain;
         output is byte-for-byte reproducible, the script prints both SHA1s)
Rebuild: python test-assets/roms/src/build-nostyra-test-gb-rom.py
SHA1:    nostyra-test.gb  91e9250a51dc70052effac29faf180dfaa5285b6
         nostyra-test.gbc 7da657f63d47836df9acfed01f385a685b8f0ada

What they do (each feature backs a runner test on the mgba core):
- Video: the background colour changes every frame (DMG: BGP register; CGB:
  BG palette 0 colour 0).
- Input: holding any joypad button switches palette.
- Audio: channel 1 plays a ~440 Hz square wave.
- SRAM: 8 KiB battery RAM; byte 0 is a boot counter (+1 per reset), byte 1 a
  marker ($A5); a fresh cart (all $FF) is initialised on first boot.
- Clock: the .gb ROM is MBC3+TIMER+RAM+BATTERY and latches the clock every
  frame (mGBA keeps it in RETRO_MEMORY_RTC, saved by the runner as rtc.bin);
  the .gbc ROM is MBC1+RAM+BATTERY (no clock).

Format: 32 KiB, title "NOSTYRA TEST", CGB flag $00 (.gb) / $80 (.gbc). Only
the first 4 bytes of the Nintendo logo are present (what mGBA checks).

nostyra-test.gba — Nostyra Game Boy Advance test ROM
====================================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-gba-rom.py (Python 3, no external toolchain;
         output is byte-for-byte reproducible, the script prints its SHA1)
Rebuild: python test-assets/roms/src/build-nostyra-test-gba-rom.py
SHA1:    0d3d359e9339c6b522794abe14736de476f1e313

What it does (each feature backs a runner test on the mgba core):
- Video: the backdrop colour takes the frame counter every VBlank.
- Input: holding any key sets the blue bits.
- Audio: DMG channel 1 plays a ~440 Hz square wave (PSG at 100 %).
- SRAM: battery SRAM at 0x0E000000; byte 0 is a boot counter, byte 1 a marker
  (0xA5). The game code "ZNOS" has no mGBA override, so mGBA detects SRAM on
  the first access (its save buffer is reported as 128 KiB until then).

Format: 64 KiB, ARM branch at the entry point, header byte 0xB2 = 0x96 and
complement checksum; the Nintendo logo area is zero.

Note (all three): with a user boot ROM / BIOS in the data folder's system/
directory (gb_bios.bin, gbc_bios.bin, gba_bios.bin; mGBA's "use BIOS" option
defaults to ON), the boot ROM checks the full logo and these ROMs hang. The
runner tests use empty temporary system directories.

nostyra-test-psx.cue/.bin, nostyra-test-psx-disc2.cue/.bin, nostyra-test-psx.m3u
====================================================================================
Nostyra PlayStation test discs

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-psx-disc.py (Python 3, no external toolchain;
         hand-assembled MIPS R3000; output is byte-for-byte reproducible, the
         script prints both SHA1s)
Rebuild: python test-assets/roms/src/build-nostyra-test-psx-disc.py
SHA1:    nostyra-test-psx.bin       a8757ddc911ec9ca13c6885a66700d86125a6ab8
         nostyra-test-psx-disc2.bin 65d41179d626e2d8b1d539aebd4475f973a8b11a

What they do (each feature backs a runner test on PCSX ReARMed, HLE BIOS):
- Video: 320x240 NTSC; every VBlank a full-screen fill takes 8 x the frame
  counter as red (consecutive frames differ); disc 2 adds a blue base.
- Input: pad 1 is read straight from the serial port each frame: Cross sets
  green, L2 sets blue.
- Audio: SPU voice 0 loops one ADPCM block (a loud square wave).
- Multi-disc: the .m3u lists both discs (serials NSTY-00001 / NSTY-00002).

Format: ISO9660 data track, MODE2/2352 Form 1 sectors (EDC/ECC zero), 64
sectors, SYSTEM.CNF + PS-X EXE. Sector 4 holds our own text "Nostyra test
disc - not licensed by Sony Computer Entertainment America", which Nostyra's
PlayStation check and region check look for (PCSX ReARMed logs "missing lic
sector?" and boots anyway). No Sony code or data is used. The runner tests
force the HLE BIOS (pcsx_rearmed_bios=HLE); a real BIOS only adds its boot
logo when enabled in the core.

nostyra-test.gen — Nostyra Mega Drive / Genesis test ROM
========================================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-mega-drive-rom.py (Python 3, no external toolchain;
         hand-assembled 68000; output is byte-for-byte reproducible, the script
         prints its SHA1)
Rebuild: python test-assets/roms/src/build-nostyra-test-mega-drive-rom.py
SHA1:    d9f6a72d1da7250037a2ccc389eb9da61f623a3f

What it does (each feature backs a runner test on Genesis Plus GX):
- Video: CRAM colour 0 (backdrop and every pixel of the all-zero planes) steps
  through an 8-colour ramp, one step per VBlank.
- Input: holding Up/Down/Left/Right/B/C on pad 1 adds blue (the runner tests
  press RetroPad B = Mega Drive B).
- Audio: PSG channel 0 plays a ~440 Hz square wave at full volume.
- SRAM: header "RA" battery RAM at 0x200001-0x203FFF; byte 0x200000 is a boot
  counter (+1 per reset), byte 0x200001 a marker (0xA5); a fresh cart (all 0xFF)
  is initialised on the first boot. Genesis Plus GX saves cart RAM up to its
  last non-0xFF byte, so save.srm is 2 bytes.

Format: 128 KiB raw image (No-Intro layout, "SEGA MEGA DRIVE " at 0x100), 3-button
pad, header checksum filled in, no BIOS needed. Committed as .gen so Markdown
tools never treat it as a document; the runner tests copy it to .md, .bin and
.smd (copier layout, made by system_catalog::raw_to_smd) in temp folders.

nostyra-test.sms / nostyra-test.gg / nostyra-test.sg — Nostyra Master System,
Game Gear and SG-1000 test ROMs
==============================================================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-sega-8bit-rom.py (Python 3, no external toolchain;
         hand-assembled Z80, one program for three targets; output is byte-for-byte
         reproducible, the script prints the SHA1s)
Rebuild: python test-assets/roms/src/build-nostyra-test-sega-8bit-rom.py
SHA1:    nostyra-test.sms bef1cd44322425a6ed41a4d5c8d606f1894598aa
         nostyra-test.gg  d8337b9eb2fd9aeaf49804aaf9b175c3bcad5239
         nostyra-test.sg  a823891f75a0e1ed5f3d8e7ce715b3cf11237c3c

What they do (each feature backs a runner test on Genesis Plus GX):
- Video: a colour that changes every frame. SMS / GG (mode 4): written to CRAM
  entry 0 (every pixel of the all-zero name table) and entry 16 (backdrop).
  SG-1000 (TMS9918): the register 7 backdrop, colours 8-15 only (colour 0 is
  transparent and looks like colour 1, black).
- Input: holding Up/Down/Left/Right/1/2 on pad 1 changes the colour (the runner
  tests press RetroPad B = button 1).
- Audio: PSG channel 0 plays a ~440 Hz square wave at full volume.
- Cart RAM (SMS / GG): the Sega mapper ($FFFC bit 3) maps battery RAM at $8000;
  byte 0 is a boot counter (+1 per reset), byte 1 a marker (0xA5); save.srm is 2
  bytes (Genesis Plus GX saves up to the last non-0xFF byte). A 32 KiB ROM gets
  no cart RAM from the core, so these ROMs are 64 KiB. The SG-1000 ROM (32 KiB)
  has no battery RAM: no save.srm is written.

Format: no copier header; SMS / GG carry a "TMR SEGA" header at 0x7FF0 (not needed
without a BIOS). Genesis Plus GX picks the system by file extension: the same
bytes renamed .bin boot as a Mega Drive game (black screen), so 8-bit files must
keep .sms / .gg / .sg.

nostyra-test.z64 — Nostyra Nintendo 64 test ROM
===============================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-n64-rom.py (Python 3, no external toolchain;
         hand-assembled MIPS R4300i with our own 0x40-0x1000 loader; output is
         byte-for-byte reproducible, the script prints its SHA1)
Rebuild: python test-assets/roms/src/build-nostyra-test-n64-rom.py
SHA1:    b0eea59fc49f27ec763c4fd3d4554cdbb520042e

What it does (each feature backs a runner test on Mupen64Plus-Next, with the
GLideN64 OpenGL renderer and with the Angrylion software renderer):
- Video: VI 320x240 RGBA5551, two framebuffers swapped every VI. An RDP command
  list sent straight to DPC_START/END fills the top half red and the bottom half
  blue whose green channel is the frame counter (mod 32): consecutive frames
  differ and an upside-down picture shows.
- Input: controller 1 is read through the PIF every frame: A held turns the top
  half green (the runner tests press RetroPad B = N64 A); stick X > +40 turns the
  bottom half white.
- Audio: AI DMA of a ~500 Hz square wave at 32 kHz.
- Save: EEPROM 4K (Advanced Homebrew Header "ED", save type 1). Block 0 byte 0 is
  a boot counter (+1 per boot), byte 1 a marker (0xA5); a blank EEPROM (all 0xFF)
  counts from 0, so the first boot writes 01 A5. Mupen64Plus-Next keeps EEPROM at
  offset 0 of its 296 960-byte save.srm.

Format: 16 KiB big-endian (.z64) image, entry 0x80000400, title "NOSTYRA TEST",
game code NEDE, no real CRC. Mupen64Plus HLE-boots the PIF and does not verify
the IPL3, so our 15-instruction loader at 0x40 PI-DMAs the payload (ROM 0x1000)
to RDRAM 0x400. It never runs on real hardware. libdragon was not used: its IPL3
(r5+, and an r4 splice) hangs Mupen's dynarec (mupen64plus-core issue #1185). The
runner tests make .v64 / .n64 copies (system_catalog::n64_to_order) in temp folders.

nostyra-test-dc.elf, nostyra-test-dc.gdi/.cue (+ "(Track 1-3)" files),
nostyra-test-dc-disc2.gdi (+ tracks), nostyra-test-dc.m3u — Nostyra Dreamcast test
program and discs
=================================================================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-dreamcast.py with ./src/dreamcast_test_sh4.py
         (SH-4 emitter, ELF writer) and ./src/dreamcast_test_disc.py (IP.BIN,
         ISO9660, GDI and cue writers); Python 3, no toolchain; output is
         byte-for-byte reproducible, the script prints every SHA1
Rebuild: python test-assets/roms/src/build-nostyra-test-dreamcast.py
SHA1:    nostyra-test-dc.elf            5f7631ab06e2fa0060a0c7c4354c27e02ca4ac2d
         nostyra-test-dc.gdi            7d16e1364c214e98db54327f6ea4095be0b34bd0
         nostyra-test-dc.cue            d243534e2a22abbe501cb25bbe6a5100b9e40dbf
         nostyra-test-dc (Track 3).bin  7c461f66799dac686cc29791d54431717d6f38cb
         nostyra-test-dc-disc2.gdi      1d1b8887856fb80b78804fad3df5b711d12bec0e
         nostyra-test-dc.m3u            26544f8b4d4ee5f271c49002f63633d77d4203a5

What it does (each feature backs a runner test on Flycast, OpenGL, HLE BIOS):
- Video: PVR framebuffer 640x480 RGB565 in VGA timing (no TA / 3D). After each
  frame the CPU fills the top half red and the bottom half blue whose green
  channel is the frame counter (mod 32): consecutive frames differ and an
  upside-down picture shows. Disc 2's program draws a magenta bottom.
- Input: the port-A controller is read every frame by Maple DMA (GET_CONDITION):
  A held turns the top half green (RetroPad B = Dreamcast A), the right trigger
  past half turns it yellow, stick X right turns the bottom half white.
- Audio: none written; Flycast still delivers 44.1 kHz silence (~735 frames per
  video frame).

Format: the ELF has one PT_LOAD at 0x8C010000 (Flycast's HLE BIOS copies it and
jumps there). The GDI is a TOSEC-style 3-track GD-ROM: track 1 data (2 KiB
sectors), track 2 audio, track 3 at LBA 45000 holding IP.BIN (hardware id SEGA
SEGAKATANA, product NSTY-00001 / NSTY-00002, area JUE, boot file 1ST_READ.BIN;
the bootstrap area stays zero: no Sega code or data) and a tiny ISO9660 volume
whose 1ST_READ.BIN is the program. The .cue lists the same tracks Redump-style
(density-area REMs); the .m3u lists both GDIs. They never run on real hardware.

nostyra-test-psp.elf, nostyra-test-psp.iso, nostyra-test-psp.cso,
nostyra-test-psp/EBOOT.PBP — Nostyra PSP test program and its containers
=================================================================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-psp.py with ./src/psp_test_allegrex.py
         (Allegrex/MIPS emitter, import stubs, ELF writer), ./src/psp_test_disc.py
         (PARAM.SFO, ISO9660, CSO, EBOOT.PBP writers) and
         ./src/psp_test_savedata.py (sceUtilitySavedata parameters); Python 3, no
         PSPSDK; output is byte-for-byte reproducible, the script prints every SHA1
Rebuild: python test-assets/roms/src/build-nostyra-test-psp.py
SHA1:    nostyra-test-psp.elf           591d2c9c31523641e98e2869b0c7e3f4f7900e04
         nostyra-test-psp.iso           fe820f87a06eac3be11396e0b0651193d6342f0a
         nostyra-test-psp.cso           d87314e0eb5973222e71a5cc0c6b063d3a9af83d
         nostyra-test-psp/EBOOT.PBP     cdcf24eaac8960573a579b6b24b7c0d7ae568e12

What it does (each feature backs a runner test on PPSSPP, software and OpenGL):
- Video: sceDisplaySetMode(0, 480, 272) + SetFrameBuf(VRAM, 512, 8888). Every
  frame the CPU fills the top half red and the bottom half blue whose green
  channel is the frame counter: consecutive frames differ and an upside-down
  picture shows.
- Input: sceCtrlPeekBufferPositive (analog sampling): Cross held turns the top
  half green (RetroPad B), stick X right turns the bottom half yellow.
- Audio: one stereo channel, a 512-frame square wave (~689 Hz) per frame.
- Save: at boot an AUTOSAVE through sceUtilitySavedata (no dialog, secureVersion
  1) writes PSP/SAVEDATA/NSTY00001DATA00/DATA.BIN + PARAM.SFO on the memory stick
  (Nostyra: the game's save folder).

Format: the ELF has one PT_LOAD at 0x08804000, no section headers (module info
found through p_paddr = its file offset), 5-word import stubs (sceDisplay,
sceCtrl, sceAudio, sceUtility). The ISO is a UMD-style ISO9660 volume (system id
"PSP GAME", UMD_DATA.BIN "NSTY-00001|…", PSP_GAME/PARAM.SFO category UG, title
"Nostyra Test PSP", PSP_GAME/SYSDIR/EBOOT.BIN = the plain ELF); the CSO is CISO v1
of the ISO (2 KiB blocks, raw deflate); EBOOT.PBP is a homebrew eboot (category
MG, DATA.PSP = the ELF, empty PSAR). Tests also build a DVD CHD of the ISO in code.
No Sony code or data; HLE boot only (no firmware).

nostyra-test.nds, nostyra-test-save.nds — Nostyra Nintendo DS test ROMs
========================================================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-nds.py with ./src/nds_test_arm9.py and
         ./src/nds_test_arm7.py (the two CPU programs) and ./src/arm_emitter.py
         (ARM assembler shared with the GBA builder); Python 3, no devkitARM;
         output is byte-for-byte reproducible, the script prints both SHA1s
Rebuild: python test-assets/roms/src/build-nostyra-test-nds.py
SHA1:    nostyra-test.nds      7e856d85444107df741ff3363fa4527123c14ee0
         nostyra-test-save.nds 367dee0d206481c9b195131055b100aa8624e75e

What they do (each feature backs a runner test on melonDS DS, built-in BIOS and
firmware, direct boot):
- Video: top screen red, bottom screen blue, the frame counter (3 bits) in the
  blue / red channel, a white 3D triangle (one vertex moving) in the top-left of
  the top screen (exercises the threaded 3D renderer).
- Input: any key held turns the top screen green. While the pen is down the
  ARM7 reads the touch-screen controller and the bottom screen shows the touch
  position (red = x / 8, green = y / 8): each quadrant gets its own tint.
- Audio: PSG channel 8 plays a ~440 Hz square wave.
- Save (nostyra-test-save.nds only): 8 KiB EEPROM (game code ZNSB is not in
  melonDS's ROM list); byte 0 is a boot counter, byte 1 a marker (0xA5), read and
  written by the ARM7 over AUXSPI.

Format: nostyra-test.nds is homebrew-style (8 KiB, ARM9 at ROM offset 0x200,
game code ZNSA). nostyra-test-save.nds is retail-like (32 KiB, ARM9 at 0x4000
starting with the decrypted secure-area marker 0xE7FFDEFF twice, entry at RAM +
0x20, logo CRC field 0xCF56): the layout of a decrypted cartridge dump, so it
passes the runner's encrypted-dump check and melonDS's own. The Nintendo logo
area is zero in both (no Nintendo code or data); they never run on hardware.

nostyra-test.dol, nostyra-test-wii.dol — Nostyra GameCube and Wii test programs
===============================================================================

Author:  Nostyra contributors (self-written, 2026)
License: GPL-3.0-or-later (full text in ./LICENSE).
Source:  ./src/build-nostyra-test-gc.py with ./src/gc_test_program.py (the program),
         ./src/gc_test_video.py (VI setup, XFB drawing), ./src/gc_test_dol.py (DOL
         writer) and ./src/ppc_emitter.py (PowerPC assembler); Python 3, no
         devkitPPC / libogc; output is byte-for-byte reproducible, the script
         prints both SHA1s
Rebuild: python test-assets/roms/src/build-nostyra-test-gc.py
SHA1:    nostyra-test.dol     ddc0c3e3ee379be42e8b836481ec0dbb7ce4e8d6
         nostyra-test-wii.dol 2bf099fe233d688c4c208e9f3b216f2886020400

What they do (each feature backs a runner test on Dolphin, HLE boot, Null and
OpenGL renderers):
- Video: VI NTSC 480i, interlaced 640x480 YUY2 XFB written by the CPU: 8 colour
  bars, a frame block that turns white / blue on alternate VI fields (Dolphin
  presents only changed XFBs, so the picture changes every field) and a pad block,
  red, green while GameCube A is held.
- Input: SI channel 0 polls a GameCube pad every field; the raw IN_HI / IN_LO words
  (buttons, sticks, analog triggers) are mirrored into MEM1.
- Audio: AI DMA loop of a 48 kHz square wave (+-0x2000) from the text section (no DSP
  microcode).
- Reset: the PI reset-button state is latched into MEM1 (Dolphin's retro_reset taps
  the reset button; the program has no reboot handler).
- MEM1 debug block at 0x80200000 (read by the tests through RETRO_MEMORY_SYSTEM_RAM):
  "NSTY", VI field counter, SI words, HID4 as set at boot (non-zero in Wii mode),
  PI interrupt cause, reset-seen flag. Offsets: gc_test_program.py and the runner
  suite apps/runner/src/gamecube_wii_real_core/mod.rs.

Format: DOL with one text section at 0x80003100 (entry), 32-byte aligned (Dolphin
refuses unaligned sections). The Wii file is the same program plus one
never-executed `mtspr HID4, r0` word, which makes Dolphin and Nostyra's scanner
treat it as a Wii executable. The GC DOL boots with a PAL/EUR default region in
Dolphin (reports 50 fps while its VI runs NTSC). The Wii DOL never drives the
Bluetooth stack, so the emulated Wii Remote is never polled. No Nintendo code or
data; they never run on real hardware.

Frame counters in RAM (RetroAchievements tests, M15)
=====================================================

Every RA-capable test program keeps a frame counter at a fixed RAM address, which
the fake achievement sets in ../achievements (generate-fake-sets.py) read at its
RetroAchievements address: NES zero page $01 (hue 0..11), SNES WRAM $7E0000, GB/GBC
HRAM $FF80 (since the first builds); copied from the CPU register every frame since
M15 phase 07: Mega Drive $FF0000 and $FF0001, SMS/GG/SG-1000 $C000, GBA IWRAM
0x03000000, PS1 0x800F0000, N64 RDRAM 0xF0000, Dreamcast 0x8C0F0000, PSP the word
after the tone buffer (0x08805F50), NDS main RAM 0x023F0008.
