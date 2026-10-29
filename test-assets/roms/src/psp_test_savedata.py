"""sceUtilitySavedata parameter block of the Nostyra PSP test program (GPLv3).

The program runs one AUTOSAVE at boot (no dialog); `secureVersion 1` stores plain
data, so PPSSPP needs no game key (0 is refused with "missing key").
"""
import struct


def savedata_param(data_ptr, data_len, disc_id, title):
    """SceUtilitySavedataParam (V3, 1536 bytes): AUTOSAVE of `data_len` bytes at
    `data_ptr` to <disc_id>DATA00/DATA.BIN, unencrypted."""
    p = bytearray(1536)
    struct.pack_into("<IiiiiiiI", p, 0, 1536, 1, 1, 0x11, 0x13, 0x12, 0x10, 0)
    struct.pack_into("<iii", p, 48, 1, 0, 0)  # mode AUTOSAVE, bind, overwrite
    p[60:60 + len(disc_id)] = disc_id.encode()  # gameName
    p[76:76 + 6] = b"DATA00"  # saveName
    p[100:100 + 8] = b"DATA.BIN"  # fileName
    struct.pack_into("<III", p, 116, data_ptr, data_len, data_len)
    p[128:128 + len(title)] = title.encode()  # sfoParam.title
    p[256:256 + 9] = b"Test save"  # sfoParam.savedataTitle
    p[1408] = 1  # sfoParam.parentalLevel
    struct.pack_into("<I", p, 1516, 1)  # secureVersion 1: plain data, no game key needed
    return bytes(p)
