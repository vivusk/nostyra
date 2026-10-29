#!/usr/bin/env python3
"""Writes the fake RetroAchievements game sets of the Nostyra test ROMs (M15 phase 07).

One folder per RA system (`<system>/`), read by the runner tests:
- `achievementsets.json`: the fake server's answer for the ROM (FakeTransport::from_dir,
  also `NOSTYRA_RA_FAKE_SERVER=<folder>`): one achievement `counter == unlock` and one
  leaderboard (start at `lb_start`, submit at `lb_submit`, value = the counter) on the
  ROM's RAM frame counter at its RA address. Shape per rcheevos v12.5.0 tests (same as
  `achievements::fake_game_set_json`). Every other request kind keeps the fake's default.
- `game-hash.txt`: the RA hash of the ROM (= crates/achievements/src/content_hash_tests.rs);
  the fake answers "unknown game" for any other hash, so a loaded set proves identification.
- `case.json`: the same facts for the real-core suite (ids, values, address, ROM).

Counters: NES hue $0001 (0..11), SNES WRAM $7E0000, GB/GBC HRAM $FF80; the other builders
copy their register counter into RAM (test-assets/roms/src, M15 D12). 8-bit reads only:
the 32-bit counters are little-endian in the cores' buffers (low byte first).

Output is byte-for-byte reproducible (CI checks `git diff` after a run).
Usage: python -I generate-fake-sets.py [output folder]
"""
import json
import pathlib
import sys

# system: (RA console, ROM in test-assets/roms, counter RA address, unlock, lb start,
#          lb submit, RA hash)
SYSTEMS = {
    "nes": (7, "nostyra-test.nes", 0x0001, 5, 3, 9, "56479a1eae301c86c803c0caeab8935f"),
    "snes": (3, "nostyra-test.sfc", 0x0000, 20, 10, 30, "7a0c0dc01bc47346e71c8a797fc09ee4"),
    "gb": (4, "nostyra-test.gb", 0xFF80, 20, 10, 30, "9a4cf57700a59e384d4e58a7c54e82d6"),
    "gbc": (6, "nostyra-test.gbc", 0xFF80, 20, 10, 30, "06858f875e98f9ebf692a4df939f81d1"),
    "gba": (5, "nostyra-test.gba", 0x000000, 20, 10, 30, "7bae559c191e9664b0c1008d2add6533"),
    "genesis": (1, "nostyra-test.gen", 0x0000, 20, 10, 30, "848d85d8f4ff37f0831b9dfe26d33b05"),
    "sms": (11, "nostyra-test.sms", 0x0000, 20, 10, 30, "756102f9eff77cf6ba653a8077e20349"),
    "gg": (15, "nostyra-test.gg", 0x0000, 20, 10, 30, "599d451aadb27125b3a691dbe4462548"),
    "sg1000": (33, "nostyra-test.sg", 0x0000, 20, 10, 30, "a3f6fcf859a53c037c41fccf5742cba4"),
    "psx": (12, "nostyra-test-psx.cue", 0x0F0000, 20, 10, 30, "1a6bda47ed75db642b078ca628b17c14"),
    "n64": (2, "nostyra-test.z64", 0x0F0000, 20, 10, 30, "29e45e317ae6d40f7e665048da86508a"),
    "dreamcast": (40, "nostyra-test-dc.gdi", 0x0F0000, 20, 10, 30,
                  "e409cbc65e2c90b7e37218815787e0b4"),
    # 0x08805F50 (word after the PSP builder's tone buffer) - 0x08000000.
    "psp": (41, "nostyra-test-psp.iso", 0x805F50, 20, 10, 30, "12401395fbb18b25dfd26ead32cb6999"),
    # Main RAM 0x023F0008 (nds_test_arm9.py FRAME_COUNTER_OFFSET).
    "nds": (18, "nostyra-test.nds", 0x3F0008, 20, 10, 30, "bb170d92aa660083da1f4a3ce714e0bb"),
}


def game_set(system, game_id, ach_id, lb_id, counter, unlock, lb_start, lb_submit):
    icon = f"http://server/Images/{game_id}.png"
    achievement = {
        "ID": ach_id, "Title": f"Frame {unlock}", "Description": f"Counter reaches {unlock}",
        "Flags": 3, "Points": 5, "MemAddr": f"{counter}={unlock}", "Author": "nostyra",
        "BadgeName": f"{ach_id:05}", "Created": 1367266583, "Modified": 1376929305,
    }
    leaderboard = {
        "ID": lb_id, "Title": "Frame counter", "Description": "Counter at submit",
        "Mem": f"STA:{counter}={lb_start}::CAN:0=1::SUB:{counter}={lb_submit}::VAL:{counter}",
        "Format": "SCORE",
    }
    return {
        "Success": True, "GameId": game_id, "Title": f"Nostyra Test {system}",
        "ConsoleId": SYSTEMS[system][0], "ImageIconUrl": icon, "RichPresenceGameId": game_id,
        "RichPresencePatch": "",
        "Sets": [{"AchievementSetId": game_id, "GameId": game_id, "Title": None, "Type": "core",
                  "ImageIconUrl": icon, "Achievements": [achievement],
                  "Leaderboards": [leaderboard]}],
    }


def write(path, text):
    path.write_bytes(text.encode("utf-8"))


def main():
    root = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).parent
    for system, (console, rom, address, unlock, lb_start, lb_submit, hash_) in SYSTEMS.items():
        game_id = 9000 + console
        ach_id, lb_id = game_id * 10 + 1, game_id * 10 + 2
        counter = f"0xH{address:06x}"
        folder = root / system
        folder.mkdir(parents=True, exist_ok=True)
        answer = game_set(system, game_id, ach_id, lb_id, counter, unlock, lb_start, lb_submit)
        write(folder / "achievementsets.json", json.dumps(answer, indent=1) + "\n")
        write(folder / "game-hash.txt", hash_ + "\n")
        case = {
            "system": system, "rom": rom, "console_id": console, "game_id": game_id,
            "achievement_id": ach_id, "leaderboard_id": lb_id, "counter_address": address,
            "unlock_value": unlock, "lb_start_value": lb_start, "lb_submit_value": lb_submit,
        }
        write(folder / "case.json", json.dumps(case, indent=1) + "\n")
        print(f"{system}: game {game_id}, counter {counter}, hash {hash_}")


if __name__ == "__main__":
    main()
