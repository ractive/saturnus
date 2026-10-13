// The emulator's names on its origin (web/shared/storage-names.js): every
// IndexedDB database the page opens and every `saturnus.*` key it reads
// or writes is listed there and nowhere else, the service worker's cache
// prefix follows the module's rule, and no other page of /saturnus/ (the
// landing page, the kill switch's old prefix, the transfer page) shares a
// name; the transfer page's own rules at the end.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  DATABASES, EMULATOR, FOLDER_KEY, FOLDER_STORE, KEYS, LOCAL_STORAGE_KEYS, OTHER_PAGES, ROM_DB, STATE_DB, TRANSFER_DB, TRANSFER_KEYS,
  TRANSFER_OPFS_DIR, cachePrefix,
} from "../shared/storage-names.js";

const WEB = fileURLToPath(new URL("../", import.meta.url));

/** The page's own modules and scripts (not the tests, the landing page or the workers' templates). */
function pageSources() {
  const files = [];
  for (const name of readdirSync(WEB)) if (name.endsWith(".js")) files.push(name);
  for (const dir of ["components", "shared", "shared/components"]) {
    for (const name of readdirSync(join(WEB, dir))) if (name.endsWith(".js")) files.push(`${dir}/${name}`);
  }
  return files.map((f) => [f, readFileSync(join(WEB, f), "utf8")]);
}

test("every saturnus.* key literal in the page is one of the listed keys, and lives in storage-names.js or theme-boot.js", () => {
  const listed = new Set(LOCAL_STORAGE_KEYS);
  for (const [file, text] of pageSources()) {
    for (const m of text.matchAll(/["'](saturnus\.[A-Za-z]+)["']/g)) {
      assert.ok(listed.has(m[1]), `${file}: ${m[1]} is not in storage-names.js`);
      // theme-boot.js is a classic script and cannot import; everything else imports.
      assert.ok(["shared/storage-names.js", "shared/theme-boot.js"].includes(file), `${file} spells ${m[1]} instead of importing it`);
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
    for (const dir of page.opfs) assert.ok(!EMULATOR.opfs.includes(dir), `${name} shares the OPFS directory ${dir}`);
    assert.notEqual(page.cachePrefix, EMULATOR.cachePrefix);
    assert.ok(!page.cachePrefix.startsWith(EMULATOR.cachePrefix) && !EMULATOR.cachePrefix.startsWith(page.cachePrefix), `${name}'s cache prefix overlaps the emulator's`);
  }
  // The landing page stores nothing and runs no script.
  assert.deepEqual(OTHER_PAGES.landing.databases, []);
  assert.deepEqual(OTHER_PAGES.landing.localStorage, []);
  const landing = readFileSync(join(WEB, "landing", "index.html"), "utf8");
  assert.ok(!/<script/.test(landing), "the landing page has no script");
});

// The transfer page (web/tx.html, web/tx/): its own keys under
// `saturnus.transfer.`, its own database (the picked folder's handle) and
// OPFS directory (the in-page "Files"), the emulator's kept ROMs read and
// never written, the theme the one key both pages share.
function txSources() {
  const files = readdirSync(join(WEB, "tx")).filter((n) => n.endsWith(".js")).map((n) => `tx/${n}`);
  return [...files, "tx.html"].map((f) => [f, readFileSync(join(WEB, f), "utf8")]);
}

test("the transfer page stores under its own names only", () => {
  const keys = Object.values(TRANSFER_KEYS);
  assert.ok(keys.length >= 2);
  assert.ok(keys.every((k) => k.startsWith("saturnus.transfer.")), "the prefix");
  assert.deepEqual(OTHER_PAGES.transfer.localStorage, keys);
  assert.deepEqual(OTHER_PAGES.transfer.databases, [TRANSFER_DB]);
  assert.deepEqual(OTHER_PAGES.transfer.opfs, [TRANSFER_OPFS_DIR]);
  assert.ok(TRANSFER_DB.startsWith("saturnus-transfer") && TRANSFER_OPFS_DIR.startsWith("saturnus-transfer"), "named after the page");
  assert.ok(!DATABASES.includes(TRANSFER_DB), "not one of the emulator's databases");
  assert.deepEqual(EMULATOR.opfs, [], "the emulator keeps nothing in OPFS");
  assert.equal(OTHER_PAGES.transfer.cachePrefix, "saturnus:/saturnus/transfer/:");
  for (const [file, text] of txSources()) {
    // No key spelled out: every one comes from storage-names.js.
    assert.doesNotMatch(text, /["'`]saturnus\.[A-Za-z.]+["'`]/, `${file} spells a storage key`);
    // localStorage only through the page's store, with the listed names.
    for (const m of text.matchAll(/localStorage\.(setItem|removeItem|clear)\(/g)) {
      assert.equal(file, "tx/app.js", `${file} writes localStorage (${m[0]})`);
    }
  }
  const app = readFileSync(join(WEB, "tx", "app.js"), "utf8");
  for (const m of app.matchAll(/store\.set\(([A-Za-z_.]+),/g)) {
    assert.ok(["TRANSFER_KEYS.link", "TRANSFER_KEYS.model", "TRANSFER_KEYS.port", "KEYS.theme"].includes(m[1]), `tx/app.js sets ${m[1]}`);
  }
});

test("the transfer page opens the emulator's ROM store only to read it, and its own database only for the folder", () => {
  const opens = { "tx/kept-roms.js": "ROM_DB", "tx/folder.js": "TRANSFER_DB" };
  for (const [file, text] of txSources()) {
    for (const m of text.matchAll(/\bindexedDB\b|\bidb\.open\(\s*([^,)]+)/g)) {
      if (m[1] !== undefined) assert.equal(m[1].trim(), opens[file], `${file} opens ${m[1]}`);
    }
    if (!(file in opens)) assert.doesNotMatch(text, /\bidb\.open\(|indexedDB\.open\(/, `${file} opens a database`);
    // OPFS only through folder.js, under its own directory.
    if (file !== "tx/folder.js") assert.doesNotMatch(text, /storage\.getDirectory|getDirectory\(\)/, `${file} opens OPFS`);
  }
  const folder = readFileSync(join(WEB, "tx", "folder.js"), "utf8");
  assert.match(folder, /idb\.open\(TRANSFER_DB, 1\)/);
  assert.match(folder, /createObjectStore\(FOLDER_STORE\)/);
  assert.match(folder, /getDirectoryHandle\(TRANSFER_OPFS_DIR, \{ create: true \}\)/, "OPFS: one directory of the page's own");
  assert.doesNotMatch(folder, /["'`]saturnus[-.][a-z.-]+["'`]/, "no name spelled out: they come from storage-names.js");
  assert.equal(FOLDER_STORE, "folders");
  assert.equal(FOLDER_KEY, "folder");
  const kept = readFileSync(join(WEB, "tx", "kept-roms.js"), "utf8");
  assert.doesNotMatch(kept, /"readwrite"|\.put\(|\.add\(|\.delete\(|\.clear\(|createObjectStore|deleteDatabase/, "read only");
  assert.match(kept, /idb\.open\(ROM_DB\)/, "opened without a version: the emulator's upgrades stay its own");
  // The emulated calculator of a ROM file keeps nothing (auto-save off);
  // that of a kept ROM is the emulator's own, in its SharedWorker
  // (iteration 46): the transfer page writes none of the emulator's databases.
  const worker = readFileSync(join(WEB, "tx", "emulator-worker.js"), "utf8");
  assert.match(worker, /host\.setAutoSave\(false\)/);
  assert.doesNotMatch(worker, /calc-states|indexedDB/);
  assert.equal(OTHER_PAGES.transfer.shares, undefined, "no database of the emulator's is written by the transfer page");
});
