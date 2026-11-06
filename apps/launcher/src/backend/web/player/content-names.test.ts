// node --test: patch chain lookup (rom_patch::find_patches_for_rom), cue / m3u
// reading and sibling paths of the web player's content preparation.
import assert from "node:assert/strict";
import { test } from "node:test";
import { cueFiles, firstPlaylistEntry, patchChain, siblingPath, stemOf } from "./content-names.ts";

test("patch chain: bps, ups, ips, each plain then numbered until a gap; any case", () => {
  const siblings = ["Game.sfc", "Game.IPS", "Game.ips1", "Game.ips3", "game.bps", "Game.ups2", "Other.ips", "Game.sfc.ips"];
  assert.deepEqual(patchChain("Game.sfc", siblings), ["game.bps", "Game.IPS", "Game.ips1"]);
  assert.deepEqual(patchChain("Game.zip", ["Game.zip", "Game.bps1", "Game.bps2"]), ["Game.bps1", "Game.bps2"]);
  assert.deepEqual(patchChain("Game.sfc", ["Game.sfc"]), []);
});

test("cue sheets list their files once, quoted or not", () => {
  const cue = [
    'FILE "Game (Track 1).bin" BINARY',
    "  TRACK 01 MODE2/2352",
    "FILE track2.bin BINARY",
    "  TRACK 02 AUDIO",
    'file "Game (Track 1).bin" BINARY',
  ].join("\n");
  assert.deepEqual(cueFiles(cue), ["Game (Track 1).bin", "track2.bin"]);
});

test("m3u: first disc, comments and blank lines skipped", () => {
  assert.equal(firstPlaylistEntry("#EXTM3U\r\n\r\n  Disc 1.cue \r\nDisc 2.cue"), "Disc 1.cue");
  assert.equal(firstPlaylistEntry("# nothing\n"), null);
});

test("sibling paths and stems", () => {
  assert.equal(siblingPath("PS1/Game/Game.cue", "Game.bin"), "PS1/Game/Game.bin");
  assert.equal(siblingPath("Game.m3u", "discs\\Disc 1.cue"), "discs/Disc 1.cue");
  assert.equal(stemOf("Game.v1.sfc"), "Game.v1");
});
