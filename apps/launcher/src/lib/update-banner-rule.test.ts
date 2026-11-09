// node --test: when the update banner shows.
import assert from "node:assert/strict";
import { test } from "node:test";
import { bannerTitle, shouldShowUpdateBanner } from "./update-banner-rule.ts";

const available = { updateAvailable: true, latest: { version: "0.5.0" }, skippedVersion: null };

test("available update shows the banner", () => {
  assert.equal(shouldShowUpdateBanner(available, false), true);
});

test("skipped version hides it, a newer one shows again", () => {
  assert.equal(shouldShowUpdateBanner({ ...available, skippedVersion: "0.5.0" }, false), false);
  assert.equal(shouldShowUpdateBanner({ ...available, skippedVersion: "0.4.5" }, false), true);
});

test("Later hides it for the run", () => {
  assert.equal(shouldShowUpdateBanner(available, true), false);
});

test("no status, no update or no release shows nothing", () => {
  assert.equal(shouldShowUpdateBanner(null, false), false);
  assert.equal(shouldShowUpdateBanner(undefined, false), false);
  assert.equal(shouldShowUpdateBanner({ ...available, updateAvailable: false }, false), false);
  assert.equal(shouldShowUpdateBanner({ ...available, latest: null }, false), false);
});

test("banner title drops the repeated 'Nostyra <version>' prefix", () => {
  assert.equal(bannerTitle("Nostyra 0.4.0 — PlayStation 2, Atari", "0.4.0"), "PlayStation 2, Atari");
  assert.equal(bannerTitle("Nostyra v0.4.0: Atari", "0.4.0"), "Atari");
  assert.equal(bannerTitle("Nostyra 0.4.0 - Atari", "0.4.0"), "Atari");
  assert.equal(bannerTitle("Nostyra 0.4.0", "0.4.0"), "");
  assert.equal(bannerTitle("Big summer update", "0.4.0"), "Big summer update");
  assert.equal(bannerTitle("Nostyra 0.4.01 beta", "0.4.0"), "Nostyra 0.4.01 beta");
});
