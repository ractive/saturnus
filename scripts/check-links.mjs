#!/usr/bin/env node
// Checks the links to other sites that people read, before a release
// (kb/docs/releasing.md):
//
//   node scripts/check-links.mjs [--verbose]
//
// It collects the external addresses of the docs pages (web/docs/pages/:
// Markdown links, <autolinks> and the site names the docs build turns
// into links, web/shared/ext-links.js), the landing page
// (web/landing/index.html, its model dialogs included), web/about.json,
// the manuals of web/commands.json (Search's links) and the app's own
// texts (web/*.js, web/components/, web/shared/, web/tx/, web/index.html).
// Each address is asked for once (a #fragment dropped): HEAD, then GET
// when HEAD is refused or fails, following redirects, with a browser's
// User-Agent, a few at a time and one at a time per host with a pause
// between them. Reported:
//
// - dead: an error status (404, 410, 500…) or no answer (DNS, TLS,
//   timeout), each with the files that link it; exit status 1;
// - blocked, check by hand: sites that refuse scripts (Cloudflare's
//   challenge, a 403 from hpcalc.org, hpmuseum.org, columbia.edu or
//   codeberg.org, which answer a browser fine);
// - web.archive.org's 429 (rate limit) is retried slowly; still limited
//   at the end it counts as blocked, as does no answer at all from those
//   sites or the Wayback Machine (they drop a script's connections when
//   it asks too often). Any other address without an answer is asked
//   once more before it counts as dead.
//
// Needs the network, so it is not part of CI. Dependency-free (Node 20's
// fetch; curl, where installed, for a second opinion on a site that
// turns Node away).

import { execFile } from "node:child_process";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { isExternal, siteNameAt, siteUrl } from "../web/shared/ext-links.js";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const WEB = join(ROOT, "web");

/** A desktop browser's User-Agent: some sites answer scripts' own with 403. */
const UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36";
/** Sites known to turn scripts away: their 403 and 503 mean "check by hand". */
const BLOCKING = ["hpcalc.org", "hpmuseum.org", "columbia.edu", "codeberg.org"];
/** Addresses that are no pages: XML namespaces, the AI relay's endpoint, examples. */
const SKIP = [/^https?:\/\/www\.w3\.org\//, /^https:\/\/tab-mcp\.ractive\.ch\/?$/, /^https?:\/\/(localhost|127\.0\.0\.1)/, /example\.(org|com)/];
const PARALLEL = 6;
const TIMEOUT = 25_000;

/** The files of `dir` (recursively) whose names pass `keep`. */
function files(dir, keep) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out.push(...files(p, keep));
    else if (keep(name)) out.push(p);
  }
  return out;
}

/** Addresses written in free text: up to whitespace, a quote or an angle bracket, without a closing punctuation mark. */
function textUrls(text) {
  const out = [];
  for (const [u] of text.matchAll(/https?:\/\/[^\s"'<>`\\]+/g)) {
    let url = u.replace(/[.,;:!?]+$/, "");
    // A closing parenthesis belongs to the address only when it opened one.
    while (url.endsWith(")") && (url.match(/\(/g) ?? []).length < (url.match(/\)/g) ?? []).length) url = url.slice(0, -1);
    if (!url.includes("${")) out.push(url);
  }
  return out;
}

/**
 * The external addresses of a Markdown page: `[text](url)` (the url may
 * hold balanced parentheses, a Wikimedia file name), `<url>`, and the
 * site names in its text outside code (as the docs build links them).
 */
export function markdownUrls(md) {
  const out = [];
  const prose = md.replace(/^```[\s\S]*?^```/gm, "").replace(/`[^`\n]*`/g, "");
  for (let i = prose.indexOf("]("); i >= 0; i = prose.indexOf("](", i + 2)) {
    let depth = 1;
    let j = i + 2;
    for (; j < prose.length && depth; j++) {
      if (prose[j] === "(") depth++;
      else if (prose[j] === ")") depth--;
      else if (/\s/.test(prose[j])) break;
    }
    if (depth === 0) out.push(prose.slice(i + 2, j - 1));
  }
  for (const [, u] of prose.matchAll(/<(https?:\/\/[^\s<>]+)>/g)) out.push(u);
  // Site names: only in text, not inside a link's address.
  const text = prose.replace(/\]\([^\s]*\)/g, "]").replace(/<https?:\/\/[^\s<>]+>/g, "");
  for (let i = 0; i < text.length; i++) {
    const name = /[a-z0-9]/i.test(text[i]) ? siteNameAt(text, i) : null;
    if (!name) continue;
    const url = siteUrl(name);
    if (url) out.push(url);
    i += name.length - 1;
  }
  return out.filter(isExternal);
}

/** Every external address to check: a Map from the address (no fragment) to the files that link it. */
export function collect() {
  const found = new Map();
  const add = (url, file) => {
    const u = url.replace(/#.*$/, "");
    if (!isExternal(u) || SKIP.some((re) => re.test(u))) return;
    if (!found.has(u)) found.set(u, new Set());
    found.get(u).add(relative(ROOT, file));
  };
  for (const f of files(join(WEB, "docs", "pages"), (n) => n.endsWith(".md"))) for (const u of markdownUrls(readFileSync(f, "utf8"))) add(u, f);
  const landing = join(WEB, "landing", "index.html");
  for (const [, u] of readFileSync(landing, "utf8").matchAll(/href="(https?:\/\/[^"]+)"/g)) add(u.replace(/&amp;/g, "&"), landing);
  const about = join(WEB, "about.json");
  for (const u of textUrls(readFileSync(about, "utf8"))) add(u, about);
  const commands = join(WEB, "commands.json");
  for (const m of Object.values(JSON.parse(readFileSync(commands, "utf8")).manuals ?? {})) add(m.url, commands);
  const app = [
    join(WEB, "index.html"),
    ...files(WEB, (n) => n.endsWith(".js")).filter((f) => !/[/\\](vendor|test|docs|landing|pkg)[/\\]/.test(relative(WEB, f))),
  ];
  for (const f of app) {
    const text = readFileSync(f, "utf8");
    // Comments are for developers; only the texts' addresses and site names.
    const code = text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:"'`])\/\/.*$/gm, "$1");
    for (const u of textUrls(code)) add(u, f);
    for (const [, s] of code.matchAll(/"([^"\n]*)"|`([^`]*)`/g)) {
      for (let i = 0; s && i < s.length; i++) {
        const name = /[a-z0-9]/i.test(s[i]) ? siteNameAt(s, i) : null;
        if (!name) continue;
        const url = siteUrl(name);
        if (url) add(url, f);
        i += name.length - 1;
      }
    }
  }
  return found;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const hostOf = (url) => new URL(url).hostname.replace(/^www\./, "");
const blocking = (host) => BLOCKING.some((b) => host === b || host.endsWith(`.${b}`));

/** One request: `{status, challenge}` or `{error}`. */
async function request(url, method) {
  try {
    const res = await fetch(url, { method, redirect: "follow", signal: AbortSignal.timeout(TIMEOUT), headers: { "User-Agent": UA, Accept: "text/html,application/xhtml+xml,application/pdf,*/*;q=0.8", "Accept-Language": "en" } });
    const challenge = res.headers.get("cf-mitigated") === "challenge" || (res.status === 403 && /cloudflare/i.test(res.headers.get("server") ?? ""));
    await res.body?.cancel().catch(() => {});
    return { status: res.status, challenge };
  } catch (err) {
    return { error: err?.cause?.code ?? err?.name ?? String(err) };
  }
}

/** The verdict on `url`: "ok", "blocked" or "dead", with what was seen. */
export async function check(url) {
  const host = hostOf(url);
  let r = await request(url, "HEAD");
  if (r.error || r.status >= 400) r = await request(url, "GET");
  // The Wayback Machine's rate limit: wait and ask again, slowly.
  for (let wait = 10_000; r.status === 429 && wait <= 40_000; wait *= 2) {
    await sleep(wait);
    r = await request(url, "GET");
  }
  // No answer: once more after a pause (a busy server, a dropped connection).
  if (r.error) {
    await sleep(5000);
    r = await request(url, "GET");
  }
  // The sites that turn scripts away and the Wayback Machine also stop
  // answering a script that asks too often: that is no dead link.
  const throttles = blocking(host) || host.endsWith("archive.org");
  if (r.error) return throttles ? { verdict: "blocked", seen: `${r.error} (no answer)` } : { verdict: "dead", seen: r.error };
  if (r.status < 400) return { verdict: "ok", seen: String(r.status) };
  if (r.challenge || r.status === 429 || (blocking(host) && [403, 503].includes(r.status))) return viaCurl(url, `${r.status}${r.challenge ? " (Cloudflare challenge)" : ""}`);
  return { verdict: "dead", seen: String(r.status) };
}

/**
 * A second opinion from curl for an address that turned Node away:
 * hpcalc.org and codeberg.org answer curl (a different TLS handshake),
 * hpmuseum.org and columbia.edu do not. A 404 or 410 is dead, a success
 * fine, anything else (no curl either) stays "check by hand". hpcalc.org
 * answers 200 for a /details/ number it does not have: a success there
 * says the site is up, not that the entry exists.
 */
function viaCurl(url, seen) {
  return new Promise((resolve) => {
    execFile("curl", ["-s", "-L", "-o", "/dev/null", "-m", "30", "-A", "Mozilla/5.0", "-w", "%{http_code}", url], (err, out) => {
      const code = Number(String(out ?? "").trim());
      if (code >= 200 && code < 400) resolve({ verdict: "ok", seen: `${code} (curl)` });
      else if (code === 404 || code === 410) resolve({ verdict: "dead", seen: `${code} (curl)` });
      else resolve({ verdict: "blocked", seen: err && !code ? seen : `${seen}, curl ${code}` });
    });
  });
}

/** Check every address, a few at a time, one per host at a time with a pause (longer for the Wayback Machine). */
async function checkAll(urls, log) {
  const byHost = new Map();
  for (const u of urls) {
    const h = hostOf(u);
    if (!byHost.has(h)) byHost.set(h, []);
    byHost.get(h).push(u);
  }
  const queues = [...byHost.entries()].sort((a, b) => b[1].length - a[1].length);
  const results = new Map();
  let next = 0;
  const worker = async () => {
    while (next < queues.length) {
      const [host, list] = queues[next++];
      const pause = host.endsWith("archive.org") ? 3000 : 700;
      for (const u of list) {
        const r = await check(u);
        results.set(u, r);
        log(`${r.verdict.padEnd(7)} ${r.seen.padEnd(5)} ${u}`);
        await sleep(pause);
      }
    }
  };
  await Promise.all(Array.from({ length: PARALLEL }, worker));
  return results;
}

async function main() {
  const verbose = process.argv.includes("--verbose");
  const found = collect();
  const urls = [...found.keys()].sort();
  console.log(`check-links: ${urls.length} addresses on ${new Set(urls.map(hostOf)).size} sites`);
  const results = await checkAll(urls, (line) => verbose && console.log(line));
  const of = (v) => urls.filter((u) => results.get(u).verdict === v);
  const where = (u) => [...found.get(u)].join(", ");
  const dead = of("dead");
  const blocked = of("blocked");
  console.log(`\nok: ${of("ok").length}`);
  if (blocked.length) {
    console.log(`\nblocked, check by hand (${blocked.length}):`);
    for (const u of blocked) console.log(`  ${results.get(u).seen}  ${u}`);
  }
  if (dead.length) {
    console.log(`\ndead (${dead.length}):`);
    for (const u of dead) console.log(`  ${results.get(u).seen}  ${u}\n      in ${where(u)}`);
  } else {
    console.log("\nno dead links");
  }
  process.exitCode = dead.length ? 1 : 0;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) await main();
