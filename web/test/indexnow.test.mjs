// scripts/indexnow.mjs: which addresses of the sitemap after a deploy
// are new or changed against the one before, and what goes to IndexNow.
// No network: the script's run is checked only for "nothing to send".
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { KEY, KEY_LOCATION, changedUrls, parseSitemap, payload } from "../../scripts/indexnow.mjs";

const SCRIPT = fileURLToPath(new URL("../../scripts/indexnow.mjs", import.meta.url));
const B = "https://ractive.ch/saturnus/";

/** A sitemap of [path, lastmod or null] entries, as web/docs/build.mjs writes it. */
const sitemap = (entries) =>
  `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${entries
    .map(([p, d]) => `  <url>\n    <loc>${B}${p}</loc>\n${d ? `    <lastmod>${d}</lastmod>\n` : ""}  </url>`)
    .join("\n")}\n</urlset>\n`;

test("parseSitemap: each loc with its lastmod or null", () => {
  const m = parseSitemap(sitemap([["", "2026-10-22"], ["app/", null], ["docs/a&amp;b/", "2026-10-23"]]));
  assert.deepEqual([...m], [[B, "2026-10-22"], [`${B}app/`, null], [`${B}docs/a&b/`, "2026-10-23"]]);
  assert.equal(parseSitemap("").size, 0);
  assert.equal(parseSitemap(null).size, 0);
  assert.equal(parseSitemap("<html>not found</html>").size, 0);
});

test("changedUrls: new addresses and changed lastmods in the new sitemap's order, then removed ones", () => {
  const before = sitemap([["", "2026-10-20"], ["app/", "2026-10-21"], ["docs/", "2026-10-22"], ["docs/gone/", "2026-10-01"], ["docs/x/", null]]);
  const after = sitemap([["", "2026-10-20"], ["app/", "2026-10-23"], ["docs/", "2026-10-22"], ["docs/new/", "2026-10-23"], ["docs/x/", "2026-10-23"]]);
  assert.deepEqual(changedUrls(before, after), [`${B}app/`, `${B}docs/new/`, `${B}docs/x/`, `${B}docs/gone/`]);
  assert.deepEqual(changedUrls(after, after), [], "nothing changed");
  assert.deepEqual(changedUrls("", after), [B, `${B}app/`, `${B}docs/`, `${B}docs/new/`, `${B}docs/x/`], "no earlier sitemap: everything");
  // A lastmod dropped (no git history in that build) is a change too; the addresses gone follow.
  assert.deepEqual(changedUrls(after, sitemap([["", null]])), [B, `${B}app/`, `${B}docs/`, `${B}docs/new/`, `${B}docs/x/`]);
});

test("payload: host, key, its location on ractive.ch and the addresses", () => {
  assert.deepEqual(payload([`${B}app/`, `${B}docs/`]), {
    host: "ractive.ch",
    key: KEY,
    keyLocation: `https://ractive.ch/${KEY}.txt`,
    urlList: [`${B}app/`, `${B}docs/`],
  });
  assert.equal(KEY_LOCATION, "https://ractive.ch/3d159b02864628aba0cf93708a000930.txt");
});

test("the script: nothing to send exits 0 without a request; bad arguments exit 1", () => {
  const dir = mkdtempSync(join(tmpdir(), "indexnow-"));
  try {
    const file = join(dir, "sitemap.xml");
    writeFileSync(file, sitemap([["", "2026-10-20"]]));
    const same = spawnSync(process.execPath, [SCRIPT, file, file], { encoding: "utf8" });
    assert.equal(same.status, 0, same.stderr);
    assert.match(same.stdout, /nothing sent/);
    const unreadable = spawnSync(process.execPath, [SCRIPT, file, join(dir, "missing.xml")], { encoding: "utf8" });
    assert.equal(unreadable.status, 0, unreadable.stderr);
    assert.match(unreadable.stderr, /cannot read/);
    assert.equal(spawnSync(process.execPath, [SCRIPT, file], { encoding: "utf8" }).status, 1);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
