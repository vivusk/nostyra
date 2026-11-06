import { test } from "node:test";
import assert from "node:assert/strict";
import { listGamesSql, searchPattern } from "./game-list-sql.ts";

// Same cases as storage-db game_queries.rs `search_pattern_escapes_wildcards_and_lowercases`.
test("search pattern escapes wildcards and lower-cases", () => {
  assert.equal(searchPattern(undefined), null);
  assert.equal(searchPattern("  "), null);
  assert.equal(searchPattern("Pokémon"), "%pokémon%");
  assert.equal(searchPattern("100%_a"), "%100\\%\\_a%");
});

test("the window parameter appears only for recently played", () => {
  assert.ok(listGamesSql("recently-played", "title").includes("?5"));
  assert.ok(!listGamesSql("all", "title").includes("?5"));
  assert.ok(listGamesSql("favorites", "play-time").includes("AND g.favorite = 1"));
  assert.ok(listGamesSql("all", "recently-added").includes("ORDER BY g.added_at DESC"));
});
