"""Atari Lynx cartridge boot loader for nostyra-test.lnx.

The 52 bytes below are the pre-encrypted first-stage loader of cc65
(libsrc/lynx/bootldr.s, Karri Kaksonen 2011; https://github.com/cc65/cc65), used
unchanged under the zlib licence of cc65:

    This software is provided 'as-is', without any express or implied warranty.
    In no event will the authors be held liable for any damages arising from
    the use of this software.

    Permission is granted to anyone to use this software for any purpose,
    including commercial applications, and to alter it and redistribute it
    freely, subject to the following restrictions:

    1. The origin of this software must not be misrepresented; you must not
    claim that you wrote the original software. If you use this software in
    a product, an acknowledgment in the product documentation would be
    appreciated but is not required.

    2. Altered source versions must be plainly marked as such, and must not
    be misrepresented as being the original software.

    3. This notice may not be removed or altered from any source distribution.

The boot ROM (or Handy's HLE boot) decrypts it to $0200 and runs it. Plain text
(from bootldr.s): Mikey mapped in, palette cleared, ComLynx / AUDIN set up, then
the next SECOND_STAGE_SIZE cartridge bytes are copied to SECOND_STAGE_ADDR and
run. cc65 puts its secondary loader there; Nostyra's test cartridge puts its own
test program there instead (no file directory needed).
"""

ENCRYPTED_LOADER = bytes([
    0xff, 0xb6, 0xbb, 0x82, 0xd5, 0x9f, 0x48, 0xcf,
    0x23, 0x37, 0x8e, 0x07, 0x38, 0xf5, 0xb6, 0x30,
    0xd6, 0x2f, 0x12, 0x29, 0x9f, 0x43, 0x5b, 0x2e,
    0xf5, 0x66, 0x5c, 0xdb, 0x93, 0x1a, 0x78, 0x55,
    0x5e, 0xc9, 0x0d, 0x72, 0x1b, 0xe9, 0xd8, 0x4d,
    0x2f, 0xe4, 0x95, 0xc0, 0x4f, 0x7f, 0x1b, 0x66,
    0x8b, 0xa7, 0xfc, 0x21,
])

SECOND_STAGE_ADDR = 0xFB68
SECOND_STAGE_SIZE = 0x97  # 151 bytes, read by the first stage
