import { test } from "node:test";
import assert from "node:assert/strict";
import { splitDatPending } from "./scan-dat-pending.ts";

test("problems with DAT candidates become pending, the rest stay problems", () => {
  const problems = [
    { relative: "Atari/pitfall.bin", reason: "not a PlayStation disc" },
    { relative: "Bad/rom.nes", reason: "missing iNES header" },
    { relative: "Zips/odd.zip", reason: "missing Mega Drive header" },
  ];
  const candidatesOf = (relative: string) =>
    relative.endsWith(".bin") || relative.endsWith(".zip") ? ["atari2600", "atari7800"] : [];
  const split = splitDatPending(problems, candidatesOf, (relative) => relative.endsWith(".zip"));
  assert.deepEqual(split.pending, [
    { relative: "Atari/pitfall.bin", reason: "not a PlayStation disc", candidates: ["atari2600", "atari7800"] },
  ]);
  assert.deepEqual(
    split.problems.map((p) => p.relative),
    ["Bad/rom.nes", "Zips/odd.zip"],
    "archives are never pending",
  );
});

test("nothing pending when no candidate fits (disc tracks were hidden before)", () => {
  const split = splitDatPending([{ relative: "x.bin", reason: "r" }], () => [], () => false);
  assert.equal(split.pending.length, 0);
  assert.equal(split.problems.length, 1);
});
