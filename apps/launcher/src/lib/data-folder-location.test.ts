// node --test: the desktop Data folder hint for sharing with Nostyra Online.
import assert from "node:assert/strict";
import { test } from "node:test";
import { isInsideAppData } from "./data-folder-location.ts";

test("AppData folders are detected, others are not", () => {
  assert.equal(isInsideAppData("C:\\Users\\vi\\AppData\\Roaming\\Nostyra\\data"), true);
  assert.equal(isInsideAppData("C:/Users/vi/appdata/Local/Nostyra"), true);
  assert.equal(isInsideAppData("C:\\Users\\vi\\Documents\\Nostyra"), false);
  assert.equal(isInsideAppData("E:\\Nostyra-Data"), false);
  assert.equal(isInsideAppData("D:\\MyAppDataBackup\\Nostyra"), false);
});
