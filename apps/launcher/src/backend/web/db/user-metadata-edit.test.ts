import { test } from "node:test";
import assert from "node:assert/strict";
import { InvalidMetadataInput, cleanUserMetadata } from "./user-metadata-edit.ts";

test("values are trimmed, blanks dropped and the title search key lower-cased", () => {
  const clean = cleanUserMetadata({ title: "  Chrono TRIGGER ", genre: "   ", releaseYear: 1995, players: 1 });
  assert.equal(clean.title, "Chrono TRIGGER");
  assert.equal(clean.titleSearch, "chrono trigger");
  assert.equal(clean.genre, null);
  assert.equal(clean.releaseYear, 1995);
});

test("long text is capped by characters", () => {
  assert.equal(cleanUserMetadata({ title: "é".repeat(250) }).title?.length, 200);
});

test("out-of-range numbers are rejected with the desktop message", () => {
  assert.throws(() => cleanUserMetadata({ releaseYear: 1979 }), new InvalidMetadataInput("year must be between 1980 and 2030"));
  assert.throws(() => cleanUserMetadata({ players: 9 }), /players must be between 1 and 8/);
});
