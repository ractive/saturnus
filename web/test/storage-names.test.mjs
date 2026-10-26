// The app's names on its origin (web/shared/storage-names.js): every
// IndexedDB database the app opens and every `saturnus.*` key it reads
// or writes is listed there and nowhere else, the service worker's cache
// prefix follows the module's rule, and the values are pinned: they are
// those of the two pages the app was made of (iteration 50d), so kept
// ROMs, states, settings and folders survive. Files' own rules at the end.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  aiSecretKey, DATABASES, FOLDER_PICKER_ID, FOLDER_STORE, KEYS, LOCAL_STORAGE_KEYS, ROM_DB, STATE_DB, TRANSFER_DB, TRANSFER_KEYS,
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
  assert.deepEqual(DATABASES, [STATE_DB, ROM_DB, TRANSFER_DB]);
});

test("the names keep the values the two pages had: the kept ROMs, states, settings and folders are found again", () => {
  assert.deepEqual(DATABASES, ["saturnus", "saturnus-roms", "saturnus-transfer"]);
  assert.equal(FOLDER_STORE, "folders");
  assert.equal(FOLDER_PICKER_ID, "saturnus-transfer");
  assert.equal(TRANSFER_OPFS_DIR, "saturnus-transfer-files");
  assert.deepEqual(KEYS, {
    model: "saturnus.model", speed: "saturnus.speed", panel: "saturnus.panel", layer: "saturnus.layer", layerTab: "saturnus.layerTab",
    panelWidth: "saturnus.panelWidth", layerWidth: "saturnus.layerWidth", treeWidth: "saturnus.treeWidth", keys: "saturnus.keys",
    storageAsk: "saturnus.storageAsk", screenLook: "saturnus.screenLook", theme: "saturnus.theme", clockOnStart: "saturnus.clockOnStart",
    sent: "saturnus.sent", tour: "saturnus.tour", view: "saturnus.view",
    aiKeepOn: "saturnus.aiKeepOn", aiRelay: "saturnus.aiRelay", aiLog: "saturnus.aiLog",
  });
  // AI access's address secret, one per relay (tab-mcp DEC-052).
  assert.equal(aiSecretKey("https://relay.ractive.ch"), "saturnus.aiSecret:https://relay.ractive.ch");
  assert.deepEqual(TRANSFER_KEYS, { link: "saturnus.transfer.link", model: "saturnus.transfer.model", port: "saturnus.transfer.port" });
});

test("the service worker's cache prefix is the module's rule, with the colon that keeps the scopes apart", () => {
  assert.equal(cachePrefix("/saturnus/app/"), "saturnus:/saturnus/app/:");
  const sw = readFileSync(join(WEB, "pwa", "sw.js"), "utf8");
  assert.match(sw, /const PREFIX = `saturnus:\$\{SCOPE\.pathname\}:`;/);
  const old = cachePrefix("/saturnus/");
  const app = cachePrefix("/saturnus/app/");
  assert.ok(!app.startsWith(old) && !old.startsWith(app), "the old worker's prefix and the app's are not prefixes of each other");
});

test("the landing page stores nothing; its one script only steps between the dialogs", () => {
  const landing = readFileSync(join(WEB, "landing", "index.html"), "utf8");
  assert.deepEqual(landing.match(/<script[^>]*>/g), ['<script type="module" src="dialogs.js">'], "one script, the dialogs'");
  const dialogs = readFileSync(join(WEB, "landing", "dialogs.js"), "utf8");
  assert.ok(!/localStorage|sessionStorage|indexedDB|document\.cookie|caches|fetch\(/.test(dialogs), "dialogs.js stores and fetches nothing");
});

// Files (web/tx/): its own keys under `saturnus.transfer.`, its own
// database (the picked folders' handles) and OPFS directory (the in-page
// "Files"), the kept ROMs read and never written. The theme is set in
// one place (Calculator's settings); Files writes only its own keys.
function txSources() {
  const files = readdirSync(join(WEB, "tx")).filter((n) => n.endsWith(".js")).map((n) => `tx/${n}`);
  return files.map((f) => [f, readFileSync(join(WEB, f), "utf8")]);
}

test("Files stores under its own names only", () => {
  const keys = Object.values(TRANSFER_KEYS);
  assert.ok(keys.length >= 2);
  assert.ok(keys.every((k) => k.startsWith("saturnus.transfer.")), "the prefix");
  assert.ok(keys.every((k) => LOCAL_STORAGE_KEYS.includes(k)), "listed with the rest");
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
    assert.ok(["TRANSFER_KEYS.link", "TRANSFER_KEYS.model", "TRANSFER_KEYS.port"].includes(m[1]), `tx/app.js sets ${m[1]}`);
  }
});

test("Files opens the ROM store only to read it, and its own database only for the folders", () => {
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
  const kept = readFileSync(join(WEB, "tx", "kept-roms.js"), "utf8");
  assert.doesNotMatch(kept, /"readwrite"|\.put\(|\.add\(|\.delete\(|\.clear\(|createObjectStore|deleteDatabase/, "read only");
  assert.match(kept, /idb\.open\(ROM_DB\)/, "opened without a version: the emulator's upgrades stay its own");
  // The emulated calculator of a ROM file keeps nothing (auto-save off);
  // that of a kept ROM is Calculator's own, in the hub (iteration 46):
  // Files writes none of the calculators' databases.
  const worker = readFileSync(join(WEB, "tx", "emulator-worker.js"), "utf8");
  assert.match(worker, /host\.setAutoSave\(false\)/);
  assert.doesNotMatch(worker, /calc-states|indexedDB/);
});
