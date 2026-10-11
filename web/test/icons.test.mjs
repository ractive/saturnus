// The icon sprite: one file, shared/icons.svg, that both pages (the
// emulator's index.html and the transfer page's tx.html) put into the
// page through shared/components/icons.js (`loadIcons`); every icon the
// pages' scripts and markup use has its `<symbol id="ic-…">` there (a
// missing one draws nothing, silently).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join, relative } from "node:path";

const WEB = fileURLToPath(new URL("../", import.meta.url));
const sprite = readFileSync(join(WEB, "shared", "icons.svg"), "utf8");
const symbols = new Set([...sprite.matchAll(/<symbol id="ic-([a-z-]+)"/g)].map((m) => m[1]));
const PAGES = ["index.html", "tx.html"];

const js = (dir, skip = []) => readdirSync(join(WEB, dir)).filter((f) => f.endsWith(".js") && !skip.includes(f)).map((f) => join(WEB, dir, f));
const sources = [
  ...js("."),
  ...js("components"),
  ...js("shared"),
  // icons.js is the helper itself (its docs say `#ic-name`).
  ...js(join("shared", "components"), ["icons.js"]),
  ...js("tx"),
];

test("every icon used has a symbol in the sprite", () => {
  const used = new Map();
  const note = (name, where) => used.set(name, [...(used.get(name) ?? []), where]);
  for (const page of PAGES) {
    const html = readFileSync(join(WEB, page), "utf8");
    for (const m of html.matchAll(/href="#ic-([a-z-]+)"/g)) note(m[1], page);
  }
  for (const file of sources) {
    const src = readFileSync(file, "utf8");
    for (const m of src.matchAll(/\bicon(?:El)?\("([a-z-]+)"/g)) note(m[1], relative(WEB, file));
    for (const m of src.matchAll(/#ic-([a-z-]+)/g)) note(m[1], relative(WEB, file));
    for (const m of src.matchAll(/\bicon: "([a-z-]+)"/g)) note(m[1], relative(WEB, file));
  }
  assert.ok(used.size > 0, "no icon uses found");
  const missing = [...used].filter(([name]) => !symbols.has(name)).map(([name, where]) => `${name} (${[...new Set(where)].join(", ")})`);
  assert.deepEqual(missing, []);
});

test("the sprite is one file: no page carries its own, both load it", () => {
  assert.ok(symbols.size > 30);
  assert.match(sprite, /^<svg xmlns="http:\/\/www.w3.org\/2000\/svg">/);
  for (const page of PAGES) {
    assert.doesNotMatch(readFileSync(join(WEB, page), "utf8"), /<symbol /, `${page} has no inline symbols`);
  }
  assert.match(readFileSync(join(WEB, "app.js"), "utf8"), /\bloadIcons\(\)/, "the emulator loads it");
  assert.match(readFileSync(join(WEB, "tx", "app.js"), "utf8"), /\bloadIcons\(\)/, "the transfer page loads it");
});
