// The icon sprite: every icon the page's scripts and markup use has its
// `<symbol id="ic-…">` in index.html (a missing one draws nothing, silently).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join, relative } from "node:path";

const WEB = fileURLToPath(new URL("../", import.meta.url));
const html = readFileSync(join(WEB, "index.html"), "utf8");
const symbols = new Set([...html.matchAll(/<symbol id="ic-([a-z-]+)"/g)].map((m) => m[1]));

const sources = [
  ...readdirSync(WEB).filter((f) => f.endsWith(".js")).map((f) => join(WEB, f)),
  // icons.js is the helper itself (its docs say `#ic-name`).
  ...readdirSync(join(WEB, "components")).filter((f) => f.endsWith(".js") && f !== "icons.js").map((f) => join(WEB, "components", f)),
];

test("every icon used has a symbol in the sprite", () => {
  const used = new Map();
  const note = (name, where) => used.set(name, [...(used.get(name) ?? []), where]);
  for (const m of html.matchAll(/href="#ic-([a-z-]+)"/g)) note(m[1], "index.html");
  for (const file of sources) {
    const src = readFileSync(file, "utf8");
    for (const m of src.matchAll(/\bicon(?:El)?\("([a-z-]+)"/g)) note(m[1], relative(WEB, file));
    for (const m of src.matchAll(/#ic-([a-z-]+)/g)) note(m[1], relative(WEB, file));
  }
  assert.ok(used.size > 0, "no icon uses found");
  const missing = [...used].filter(([name]) => !symbols.has(name)).map(([name, where]) => `${name} (${[...new Set(where)].join(", ")})`);
  assert.deepEqual(missing, []);
});
