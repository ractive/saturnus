#!/usr/bin/env node
// Tells IndexNow (https://www.indexnow.org/documentation; Bing, Yandex,
// Seznam, Naver and others share the submissions) which addresses of
// /saturnus/ changed in a deploy, from the sitemap before and after it:
//
//   node scripts/indexnow.mjs OLD_SITEMAP NEW_SITEMAP
//
// An address is sent when it is new, its lastmod differs (web/docs/
// build.mjs writes them from git) or it left the sitemap (a deleted page,
// which then answers 404: IndexNow takes those too). A missing or unreadable OLD_SITEMAP
// (the first deploy, ractive.ch down) counts as empty: every address is
// sent. The key's file is served at the top of ractive.ch by that site's
// own repository (keyLocation below); saturnus publishes none.
//
// Run by .github/workflows/pages.yml after the FTP deploy. It never
// fails a deploy: nothing to send, an error answer or no network all
// end with a log line and exit status 0. Only bad arguments exit 1.
// Dependency-free (Node's fetch); the pure functions are tested in
// web/test/indexnow.test.mjs.

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

export const ENDPOINT = "https://api.indexnow.org/indexnow";
export const KEY = "3d159b02864628aba0cf93708a000930";
export const KEY_LOCATION = `https://ractive.ch/${KEY}.txt`;

/** The sitemap `xml`'s addresses: a Map from each <loc> to its <lastmod> (null without one). */
export function parseSitemap(xml) {
  const out = new Map();
  const text = (block, tag) => block.match(new RegExp(`<${tag}>\\s*([^<]*?)\\s*</${tag}>`))?.[1] ?? null;
  for (const [, block] of String(xml ?? "").matchAll(/<url>([\s\S]*?)<\/url>/g)) {
    const loc = text(block, "loc");
    if (loc) out.set(unescapeXml(loc), text(block, "lastmod"));
  }
  return out;
}

function unescapeXml(s) {
  return s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, "&");
}

/**
 * The addresses of `newXml` that `oldXml` lacks or lists with another
 * lastmod, in `newXml`'s order, then those of `oldXml` that `newXml` lacks.
 */
export function changedUrls(oldXml, newXml) {
  const before = parseSitemap(oldXml);
  const after = parseSitemap(newXml);
  const changed = [...after].filter(([loc, lastmod]) => !before.has(loc) || before.get(loc) !== lastmod).map(([loc]) => loc);
  return [...changed, ...[...before.keys()].filter((loc) => !after.has(loc))];
}

/** The IndexNow request body for `urls` (all on one host). */
export function payload(urls) {
  return { host: new URL(urls[0]).host, key: KEY, keyLocation: KEY_LOCATION, urlList: urls };
}

function readOr(file, fallback) {
  try {
    return readFileSync(file, "utf8");
  } catch {
    return fallback;
  }
}

async function main(argv) {
  if (argv.length !== 2) {
    console.error("usage: node scripts/indexnow.mjs OLD_SITEMAP NEW_SITEMAP");
    return 1;
  }
  const [oldFile, newFile] = argv;
  const newXml = readOr(newFile, null);
  if (newXml === null) {
    console.warn(`indexnow: cannot read ${newFile}; nothing sent`);
    return 0;
  }
  const oldXml = readOr(oldFile, "");
  if (!parseSitemap(oldXml).size) console.log(`indexnow: no earlier sitemap in ${oldFile}; every address counts as new`);
  const urls = changedUrls(oldXml, newXml);
  if (!urls.length) {
    console.log("indexnow: no new, changed or removed address; nothing sent");
    return 0;
  }
  console.log(`indexnow: sending ${urls.length} address(es):\n${urls.map((u) => `  ${u}`).join("\n")}`);
  try {
    const res = await fetch(ENDPOINT, {
      method: "POST",
      headers: { "Content-Type": "application/json; charset=utf-8" },
      body: JSON.stringify(payload(urls)),
      signal: AbortSignal.timeout(30_000),
    });
    if (res.ok) console.log(`indexnow: accepted (HTTP ${res.status})`);
    else console.warn(`::warning::indexnow: HTTP ${res.status} ${res.statusText}: ${(await res.text().catch(() => "")).slice(0, 500)}`);
  } catch (err) {
    console.warn(`::warning::indexnow: request failed: ${err?.message ?? err}`);
  }
  return 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await main(process.argv.slice(2));
}
