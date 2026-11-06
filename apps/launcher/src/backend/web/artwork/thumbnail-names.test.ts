// Desktop examples from crates/artwork-cache (thumbnail_name_sanitizer.rs,
// thumbnail_source_urls.rs, tests/thumbnail_name_tests.rs) must give the same names.
import { test } from "node:test";
import assert from "node:assert/strict";
import { fallbackNames, resolveNames, sanitizeThumbnailName, thumbnailUrl } from "./thumbnail-names.ts";

test("forbidden characters become underscores", () => {
  assert.equal(sanitizeThumbnailName('A&B*C/D:E`F<G>H?I\\J|K"L'), "A_B_C_D_E_F_G_H_I_J_K_L");
  assert.equal(sanitizeThumbnailName("Pokémon - Red (USA)"), "Pokémon - Red (USA)");
});

test("fallbacks drop trailing tags but keep the region", () => {
  assert.deepEqual(fallbackNames("Super Mario World (USA) (Rev 1)"), ["Super Mario World (USA)"]);
  assert.deepEqual(fallbackNames("X (Japan) (En,Ja) (Rev 2)"), ["X (Japan) (En,Ja)", "X (Japan)"]);
  assert.deepEqual(fallbackNames("Chrono Trigger (USA)"), []);
  assert.deepEqual(fallbackNames("(Only)"), []);
  assert.deepEqual(fallbackNames("No Tags"), []);
});

test("resolved names are sanitised, best first, without duplicates", () => {
  assert.deepEqual(resolveNames("  Mario & Luigi (USA) (Rev 1)  "), ["Mario _ Luigi (USA) (Rev 1)", "Mario _ Luigi (USA)"]);
});

// Same table as crates/artwork-cache/tests/sanitizer_and_fallback_tests.rs.
test("desktop table cases", () => {
  assert.equal(sanitizeThumbnailName("Q*bert's Qubes (USA)"), "Q_bert's Qubes (USA)");
  assert.equal(sanitizeThumbnailName("Pop'n TwinBee, Dr. Mario - Rev. (Japan)"), "Pop'n TwinBee, Dr. Mario - Rev. (Japan)");
  assert.equal(sanitizeThumbnailName("Pokémon’s (Japan)"), "Pokémon’s (Japan)");
  assert.deepEqual(fallbackNames("X (USA) (Rev 1) (Beta)"), ["X (USA) (Rev 1)", "X (USA)"]);
  assert.deepEqual(fallbackNames("X (Japan) (En)"), ["X (Japan)"]);
  assert.deepEqual(resolveNames("Q*bert's Qubes (USA) (Rev 1)"), ["Q_bert's Qubes (USA) (Rev 1)", "Q_bert's Qubes (USA)"]);
  assert.deepEqual(resolveNames("  Chrono Trigger (USA) "), ["Chrono Trigger (USA)"]);
  assert.deepEqual(resolveNames("Mr. Do! (USA)"), ["Mr. Do! (USA)"]);
  assert.deepEqual(resolveNames("Super Bomberman 5 (Japan) (Rev 1)."), ["Super Bomberman 5 (Japan) (Rev 1)."]);
  assert.deepEqual(fallbackNames("Game (The) - Part 2 (Europe) (Rev 2)"), ["Game (The) - Part 2 (Europe)"]);
});

test("raw URLs escape like the desktop mirror URL", () => {
  assert.equal(
    thumbnailUrl("Nintendo_-_Super_Nintendo_Entertainment_System", "Named_Boxarts", "Chrono Trigger (USA)"),
    "https://raw.githubusercontent.com/libretro-thumbnails/Nintendo_-_Super_Nintendo_Entertainment_System/master/Named_Boxarts/Chrono%20Trigger%20(USA).png",
  );
  assert.ok(
    thumbnailUrl("R", "Named_Snaps", "Pop’n #1 + 50% (Japan)").endsWith(
      "/Named_Snaps/Pop%E2%80%99n%20%231%20%2B%2050%25%20(Japan).png",
    ),
  );
});
