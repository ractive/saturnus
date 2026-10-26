// The site web/site.sh assembles: the landing page and the favicon at the
// top with the .htaccess, no emulator/ or transfer/, and the app under
// app/ (iteration 50d) as the installed page needs it:
// its service worker precaches exactly the files under app/ (but itself),
// every page file among them (Calculator's and Files'), each with its
// content's SHA-256; its build hash stays put for the same files, changes
// with any of them, and ignores the landing page; the page links the
// manifest, named as the one app; the desktop apps' file list has none of
// it; the docs under docs/ with llms.txt and sitemap.xml at the top
// (iteration 57).
// Skipped without the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, cpSync, existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const WEB = fileURLToPath(new URL("../", import.meta.url));
const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm")) && existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));

/** Every file under `dir`, relative to it, sorted. */
function filesUnder(dir, prefix = "") {
  const out = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out.push(...filesUnder(p, `${prefix}${name}/`));
    else out.push(`${prefix}${name}`);
  }
  return out.sort();
}

/** A subsite's sw.js: its BUILD and FILES' paths; every file's hash in FILES is its content's. */
function worker(dir) {
  const sw = readFileSync(join(dir, "sw.js"), "utf8");
  const build = sw.match(/^const BUILD = "([0-9a-f]{16})";$/m)?.[1];
  const list = sw.match(/^const FILES = (\[\n[\s\S]*?\n\]);$/m)?.[1];
  assert.ok(build && list, "sw.js starts with BUILD and FILES");
  const pairs = JSON.parse(list.replace(/,(\s*\])/g, "$1"));
  for (const [f, hash] of pairs) {
    assert.equal(hash, createHash("sha256").update(readFileSync(join(dir, f))).digest("hex"), `${f}'s hash in sw.js`);
  }
  return { build, files: pairs.map(([f]) => f) };
}

/** Build the site of `web` into a new directory; the app's sw.js's BUILD and FILES. */
function site(web) {
  const out = mkdtempSync(join(tmpdir(), "saturnus-site-"));
  execFileSync("sh", [join(web, "site.sh"), out], { stdio: "ignore" });
  return { out, ...worker(join(out, "app")) };
}

test("the service worker precaches exactly the app's files, and a new build is a new hash", { skip: !built && "web/pkg not built (just web)" }, () => {
  const tmp = [];
  try {
    const a = site(WEB);
    tmp.push(a.out);
    // The top: the landing page, the favicon, the .htaccess, the marker, llms.txt
    // and sitemap.xml; nothing else.
    const top = filesUnder(a.out).filter((f) => !f.includes("/") || f.startsWith("img/"));
    const landing = filesUnder(join(WEB, "landing")).filter((f) => !f.startsWith("raw/") && f !== "shots.mjs");
    assert.deepEqual(top, [".htaccess", ".saturnus-site", "favicon.svg", ...landing, "llms.txt", "sitemap.xml"].sort());
    assert.match(readFileSync(join(a.out, "sitemap.xml"), "utf8"), /^<\?xml version="1\.0" encoding="UTF-8"\?>\n<urlset xmlns="http:\/\/www\.sitemaps\.org\/schemas\/sitemap\/0\.9">/);
    // The old subsites are gone, and no worker is served at the top.
    for (const gone of ["emulator", "transfer", "sw.js"]) {
      assert.ok(!existsSync(join(a.out, gone)), `no ${gone} at the top`);
    }
    assert.deepEqual(readdirSync(a.out).filter((f) => statSync(join(a.out, f)).isDirectory()).sort(), ["app", "docs", "img"]);
    // The docs (iteration 57, web/docs/build.mjs): each page's HTML with its
    // Markdown beside it, the reference page, a command file per model;
    // outside the app, so not precached.
    const docs = filesUnder(join(a.out, "docs"));
    for (const f of ["index.html", "index.md", "48gx/index.html", "48gx.md", "commands/index.html", "commands.md", "commands/page.js", "commands/slug.js", "commands/48gx.md", "commands/19bii.md", "saturn/index.html", "transfer/index.html", "docs.css"]) {
      assert.ok(docs.includes(f), `docs/${f}`);
    }
    assert.equal(docs.filter((f) => /^commands\/[a-z0-9]+\.md$/.test(f)).length, 12, "a command file per model");
    assert.match(readFileSync(join(a.out, "docs", "commands", "index.html"), "utf8"), /http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self' 'wasm-unsafe-eval'/, "the site's CSP");
    assert.match(readFileSync(join(a.out, "llms.txt"), "utf8"), /^# saturnus\n\n> /);
    assert.ok(!existsSync(join(a.out, "app", ".htaccess")), "one .htaccess, at the top");
    const shipped = filesUnder(join(a.out, "app")).filter((f) => f !== "sw.js");
    assert.deepEqual(a.files, shipped);
    for (const f of ["index.html", "manifest.webmanifest", "pkg/saturnus_web.js", "pkg/saturnus_web_bg.wasm", "icons/icon-512.png", "icons/maskable-512.png", "icons/apple-touch-icon.png", "app.js", "tx/app.js", "tx/protocol-worker.js", "tx/emulator-worker.js", "worker.js", "main.js", "home.js", "router.js"]) {
      assert.ok(a.files.includes(f), `${f} precached`);
    }
    const pageFiles = execFileSync("sh", [join(WEB, "site.sh"), "--list"], { encoding: "utf8" }).trim().split("\n");
    for (const f of pageFiles) assert.ok(a.files.includes(f), `page file ${f} precached`);
    assert.ok(pageFiles.includes("index.html") && pageFiles.includes("tx/app.js") && pageFiles.includes("tx/style.css"), "Files is part of the app");
    assert.ok(!pageFiles.some((f) => /^tx\.html$|shared-worker|build\.js$/.test(f)), "no second page, no SharedWorker, no build id");
    assert.ok(pageFiles.includes("shared/icons.svg"), "the icon sprite is a page file");
    // The desktop apps embed the page files: no worker, no manifest.
    assert.ok(!pageFiles.some((f) => /sw\.js$|manifest|^pwa\/|^tx\/pwa\//.test(f)), "the app's list has no installable-page file");

    const index = readFileSync(join(a.out, "app", "index.html"), "utf8");
    assert.match(index, /<link rel="manifest" href="manifest.webmanifest">/);
    assert.match(index, /<link rel="apple-touch-icon" href="icons\/apple-touch-icon.png">/);
    assert.match(index, /http-equiv="Content-Security-Policy"/);
    // One document: the home page and both views, linked by their routes.
    for (const id of ["home-view", "calculator-view", "files-view"]) assert.match(index, new RegExp(`<div id="${id}"`));
    assert.match(index, /<a id="to-transfer"[^>]* href="#\/files"[^>]*>.*<span>Files<\/span><\/a>/);
    assert.match(index, /<a id="to-emulator"[^>]* href="#\/calculator"[^>]*>.*<span>Calculator<\/span><\/a>/);
    assert.match(index, /<a class="home-landing[^"]*" href="\.\.\/" data-site="landing"/, "the landing page, one up");
    assert.match(index, /<a class="home-docs[^"]*" href="\.\.\/docs\/commands\/" data-site="commands"/, "the docs' command reference, beside the app");
    assert.doesNotMatch(index, /saturnus tx/, "the views are Calculator and Files");
    const landingIndex = readFileSync(join(a.out, "index.html"), "utf8");
    assert.doesNotMatch(landingIndex, /manifest|Content-Security-Policy/, "the landing page is copied as it is");
    assert.match(landingIndex, /href="app\/"/, "the landing page links the app");
    assert.doesNotMatch(landingIndex, /href="(emulator|transfer)\/"/, "and nothing else of it");
    assert.doesNotMatch(landingIndex, /Coming soon|saturnus tx/);
    // The headline leads with saturnus; HP only in running text (owner, 2026-10-14).
    assert.match(landingIndex, /<title>saturnus: classic HP calculators in your browser<\/title>/);
    assert.match(landingIndex, /<h1>saturnus: classic HP calculators in your browser<\/h1>/);
    const manifest = JSON.parse(readFileSync(join(a.out, "app", "manifest.webmanifest"), "utf8"));
    for (const icon of manifest.icons) assert.ok(a.files.includes(icon.src), `${icon.src} shipped`);
    assert.equal(manifest.start_url, "./");
    assert.equal(manifest.short_name, "saturnus");
    assert.match(manifest.name, /^saturnus/);
    assert.match(manifest.description, /calculators/);

    // The same files, the same build; one changed byte, another.
    const b = site(WEB);
    tmp.push(b.out);
    assert.equal(b.build, a.build);
    const copy = mkdtempSync(join(tmpdir(), "saturnus-web-"));
    tmp.push(copy);
    cpSync(WEB, copy, { recursive: true, filter: (src) => !src.includes(`${join(WEB, "test")}`) });
    appendFileSync(join(copy, "style.css"), "\n");
    const c = site(copy);
    tmp.push(c.out);
    assert.notEqual(c.build, a.build);
    assert.deepEqual(c.files, a.files);
    // A landing-page change leaves the app's build alone.
    appendFileSync(join(copy, "landing", "style.css"), "\n");
    const d = site(copy);
    tmp.push(d.out);
    assert.equal(d.build, c.build);
    // Files' and the shared files are the app's too.
    appendFileSync(join(copy, "tx", "style.css"), "\n");
    const e = site(copy);
    tmp.push(e.out);
    assert.notEqual(e.build, d.build);
    appendFileSync(join(copy, "shared", "tokens.css"), "\n");
    const f = site(copy);
    tmp.push(f.out);
    assert.notEqual(f.build, e.build);
  } finally {
    for (const d of tmp) rmSync(d, { recursive: true, force: true });
  }
});

test("the .htaccess makes every file revalidate, so a deploy is never mixed with stale CSS, and redirects nothing", () => {
  const ht = readFileSync(join(WEB, "site.htaccess"), "utf8");
  assert.doesNotMatch(ht, /Redirect|RewriteRule/);
  assert.match(ht, /^Header set Cache-Control "no-cache"$/m);
  assert.match(ht, /^Header unset Expires$/m);
  // Inside the mod_headers guard, so a host without it still serves.
  const guard = ht.slice(ht.indexOf("<IfModule mod_headers.c>"), ht.indexOf("</IfModule>"));
  assert.ok(guard.includes('Header set Cache-Control "no-cache"'));
  // The docs' Markdown and llms.txt as UTF-8 text (iteration 57).
  assert.match(ht, /^AddType text\/markdown \.md$/m);
  assert.match(ht, /^AddCharset utf-8 \.md \.txt$/m);
});
