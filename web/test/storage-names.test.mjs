// The emulator's names on its origin (web/storage-names.js): every
// IndexedDB database the page opens and every `saturnus.*` key it reads
// or writes is listed there and nowhere else, the service worker's cache
// prefix follows the module's rule, and no other page of /saturnus/ (the
// landing page, the kill switch's old prefix, satx later) shares a name.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { DATABASES, EMULATOR, KEYS, LOCAL_STORAGE_KEYS, OTHER_PAGES, ROM_DB, STATE_DB, cachePrefix } from "../storage-names.js";

const WEB = fileURLToPath(new URL("../", import.meta.url));

/** The page's own modules and scripts (not the tests, the landing page or the workers' templates). */
function pageSources() {
  const files = [];
  for (const name of readdirSync(WEB)) if (name.endsWith(".js")) files.push(name);
  for (const name of readdirSync(join(WEB, "components"))) if (name.endsWith(".js")) files.push(`components/${name}`);
  return files.map((f) => [f, readFileSync(join(WEB, f), "utf8")]);
}

test("every saturnus.* key literal in the page is one of the listed keys, and lives in storage-names.js or theme-boot.js", () => {
  const listed = new Set(LOCAL_STORAGE_KEYS);
  for (const [file, text] of pageSources()) {
    for (const m of text.matchAll(/["'](saturnus\.[A-Za-z]+)["']/g)) {
      assert.ok(listed.has(m[1]), `${file}: ${m[1]} is not in storage-names.js`);
      // theme-boot.js is a classic script and cannot import; everything else imports.
      assert.ok(["storage-names.js", "theme-boot.js"].includes(file), `${file} spells ${m[1]} instead of importing it`);
    }
  }
  assert.equal(new Set(LOCAL_STORAGE_KEYS).size, LOCAL_STORAGE_KEYS.length, "no key twice");
  assert.ok(LOCAL_STORAGE_KEYS.every((k) => k.startsWith("saturnus.")));
  assert.equal(KEYS.theme, "saturnus.theme");
});

test("every IndexedDB database the page opens is listed", () => {
  const opened = new Set();
  for (const [file, text] of pageSources()) {
    for (const m of text.matchAll(/\b(?:indexedDB|idb)\.open\(\s*([^,)]+)/g)) {
      const arg = m[1].trim();
      assert.ok(/^(DB_NAME|ROM_DB|STATE_DB)$/.test(arg), `${file}: indexedDB.open(${arg}) names its database by a literal`);
      opened.add(arg);
    }
  }
  assert.ok(opened.size >= 2, "the states and the ROMs");
  assert.deepEqual(DATABASES, [STATE_DB, ROM_DB]);
  assert.deepEqual(DATABASES, ["saturnus", "saturnus-roms"]);
});

test("the service worker's cache prefix is the module's rule, with the colon that keeps the scopes apart", () => {
  assert.equal(cachePrefix("/saturnus/emulator/"), "saturnus:/saturnus/emulator/:");
  const sw = readFileSync(join(WEB, "pwa", "sw.js"), "utf8");
  assert.match(sw, /const PREFIX = `saturnus:\$\{SCOPE\.pathname\}:`;/);
  const kill = readFileSync(join(WEB, "landing", "sw.js"), "utf8");
  assert.match(kill, /const PREFIX = `saturnus:\$\{SCOPE\.pathname\}:`;/);
  assert.equal(EMULATOR.cachePrefix, "saturnus:/saturnus/emulator/:");
  const old = cachePrefix("/saturnus/");
  assert.ok(!EMULATOR.cachePrefix.startsWith(old) && !old.startsWith(EMULATOR.cachePrefix), "the old worker's prefix and the emulator's are not prefixes of each other");
});

test("no other page of /saturnus/ shares a database, a key or a cache prefix with the emulator", () => {
  assert.ok(Object.keys(OTHER_PAGES).includes("landing"));
  for (const [name, page] of Object.entries(OTHER_PAGES)) {
    for (const db of page.databases) assert.ok(!EMULATOR.databases.includes(db), `${name} shares the database ${db}`);
    for (const key of page.localStorage) assert.ok(!EMULATOR.localStorage.includes(key), `${name} shares the key ${key}`);
    assert.notEqual(page.cachePrefix, EMULATOR.cachePrefix);
    assert.ok(!page.cachePrefix.startsWith(EMULATOR.cachePrefix) && !EMULATOR.cachePrefix.startsWith(page.cachePrefix), `${name}'s cache prefix overlaps the emulator's`);
  }
  // The landing page stores nothing and runs no script.
  assert.deepEqual(OTHER_PAGES.landing.databases, []);
  assert.deepEqual(OTHER_PAGES.landing.localStorage, []);
  const landing = readFileSync(join(WEB, "landing", "index.html"), "utf8");
  assert.ok(!/<script/.test(landing), "the landing page has no script");
});
