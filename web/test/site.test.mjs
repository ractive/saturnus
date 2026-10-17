// The site web/site.sh assembles: the landing page, the favicon and the
// kill switch at the top with the .htaccess, the emulator under
// emulator/ as the installed page needs it: its service worker precaches
// exactly the files under emulator/ (but itself), every page file among
// them, each with its content's SHA-256; its build hash stays put for the
// same files, changes with any of them, and ignores the landing page; the
// page links the manifest; the desktop app's file list has none of it. The transfer page under
// transfer/ the same way, with its own marker, manifest, worker and
// build. Skipped without the wasm package (`just web`).
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

/** Build the site of `web` into a new directory; the emulator's sw.js's BUILD and FILES, and the transfer page's (`tx`). */
function site(web) {
  const out = mkdtempSync(join(tmpdir(), "saturnus-site-"));
  execFileSync("sh", [join(web, "site.sh"), out], { stdio: "ignore" });
  return { out, ...worker(join(out, "emulator")), tx: worker(join(out, "transfer")) };
}

test("the service worker precaches exactly the emulator's files, and a new build is a new hash", { skip: !built && "web/pkg not built (just web)" }, () => {
  const tmp = [];
  try {
    const a = site(WEB);
    tmp.push(a.out);
    // The top: the landing page, the favicon, the kill switch, the .htaccess and the marker; nothing else.
    const top = filesUnder(a.out).filter((f) => !f.startsWith("emulator/") && !f.startsWith("transfer/"));
    const landing = filesUnder(join(WEB, "landing")).filter((f) => !f.startsWith("raw/") && f !== "shots.mjs" && f !== "sw.js");
    assert.deepEqual(top, [".htaccess", ".saturnus-site", "favicon.svg", ...landing, "sw.js"].sort());
    assert.match(readFileSync(join(a.out, "sw.js"), "utf8"), /self\.registration\.unregister\(\)/, "the top's sw.js is the kill switch");
    assert.ok(!existsSync(join(a.out, "emulator", ".htaccess")), "one .htaccess, at the top");
    const shipped = filesUnder(join(a.out, "emulator")).filter((f) => f !== "sw.js");
    assert.deepEqual(a.files, shipped);
    for (const f of ["index.html", "manifest.webmanifest", "pkg/saturnus_web.js", "pkg/saturnus_web_bg.wasm", "icons/icon-512.png", "icons/maskable-512.png", "icons/apple-touch-icon.png"]) {
      assert.ok(a.files.includes(f), `${f} precached`);
    }
    const pageFiles = execFileSync("sh", [join(WEB, "site.sh"), "--list"], { encoding: "utf8" }).trim().split("\n");
    for (const f of pageFiles) assert.ok(a.files.includes(f), `page file ${f} precached`);
    assert.ok(!pageFiles.includes("tx.html") && !pageFiles.some((f) => f.startsWith("tx/")), "the transfer page is not the emulator's");
    assert.ok(pageFiles.includes("shared/icons.svg"), "the icon sprite is a page file");
    // The desktop app embeds the page files: no worker, no manifest.
    assert.ok(!pageFiles.some((f) => /sw\.js$|manifest|^pwa\//.test(f)), "the app's list has no installable-page file");

    const index = readFileSync(join(a.out, "emulator", "index.html"), "utf8");
    assert.match(index, /<link rel="manifest" href="manifest.webmanifest">/);
    assert.match(index, /<link rel="apple-touch-icon" href="icons\/apple-touch-icon.png">/);
    assert.match(index, /http-equiv="Content-Security-Policy"/);
    // The way to saturnus tx, its sibling (shared/site-links.js; site.sh checks it names a page).
    assert.match(index, /<a id="to-transfer"[^>]* href="\.\.\/transfer\/"[^>]*>.*<span>Files<\/span><\/a>/);
    assert.ok(existsSync(join(a.out, "emulator", "../transfer/index.html")));
    const landingIndex = readFileSync(join(a.out, "index.html"), "utf8");
    assert.doesNotMatch(landingIndex, /manifest|Content-Security-Policy/, "the landing page is copied as it is");
    assert.match(landingIndex, /href="emulator\/"/, "the landing page links the emulator");
    assert.match(landingIndex, /href="transfer\/"/, "and the transfer page");
    assert.doesNotMatch(landingIndex, /Coming soon/);
    // The headline leads with saturnus; HP only in running text (owner, 2026-10-14).
    assert.match(landingIndex, /<title>saturnus: classic HP calculators in your browser<\/title>/);
    assert.match(landingIndex, /<h1>saturnus: classic HP calculators in your browser<\/h1>/);
    const manifest = JSON.parse(readFileSync(join(a.out, "emulator", "manifest.webmanifest"), "utf8"));
    for (const icon of manifest.icons) assert.ok(a.files.includes(icon.src), `${icon.src} shipped`);
    assert.equal(manifest.start_url, "./");

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
    // A landing-page change leaves the emulator's build alone.
    appendFileSync(join(copy, "landing", "style.css"), "\n");
    const d = site(copy);
    tmp.push(d.out);
    assert.equal(d.build, c.build);
    // style.css is the emulator's alone; a shared file is both pages'.
    assert.equal(c.tx.build, a.tx.build, "the emulator's stylesheet is not the transfer page's");
    appendFileSync(join(copy, "tx", "style.css"), "\n");
    const e = site(copy);
    tmp.push(e.out);
    assert.equal(e.build, d.build, "the transfer page's change leaves the emulator's build alone");
    assert.notEqual(e.tx.build, d.tx.build);
    appendFileSync(join(copy, "shared", "tokens.css"), "\n");
    const f = site(copy);
    tmp.push(f.out);
    assert.notEqual(f.build, e.build);
    assert.notEqual(f.tx.build, e.tx.build);
  } finally {
    for (const d of tmp) rmSync(d, { recursive: true, force: true });
  }
});

test("the transfer page: its own subsite, marker, manifest, worker and file list", { skip: !built && "web/pkg not built (just web)" }, () => {
  const out = mkdtempSync(join(tmpdir(), "saturnus-site-"));
  try {
    execFileSync("sh", [join(WEB, "site.sh"), out], { stdio: "ignore" });
    const tx = join(out, "transfer");
    const { files } = worker(tx);
    assert.ok(existsSync(join(tx, ".saturnus-transfer")), "its own marker");
    assert.ok(!files.includes(".saturnus-transfer"), "not precached");
    assert.deepEqual(files, filesUnder(tx).filter((f) => f !== "sw.js" && !f.startsWith(".")));
    for (const f of ["index.html", "manifest.webmanifest", "tx/app.js", "tx/protocol-worker.js", "tx/emulator-worker.js", "pkg/saturnus_web.js", "pkg/saturnus_web_bg.wasm", "shared/icons.svg", "icons/icon-512.png", "favicon.svg"]) {
      assert.ok(files.includes(f), `${f} precached`);
    }
    assert.ok(!files.some((f) => f.startsWith("tx/pwa/") || f.endsWith(".sh") || f.startsWith("components/")), "no build files, no emulator components");
    const listed = execFileSync("sh", [join(WEB, "site.sh"), "--list", "transfer"], { encoding: "utf8" }).trim().split("\n");
    assert.ok(listed.includes("tx.html") && listed.includes("tx/app.js") && listed.includes("shared/icons.svg"));
    for (const f of listed) assert.ok(f === "tx.html" ? files.includes("index.html") : files.includes(f), `${f} shipped`);
    const index = readFileSync(join(tx, "index.html"), "utf8");
    assert.match(index, /<title>saturnus tx<\/title>/);
    // The way to the emulator, its sibling, and the module that sets the links.
    assert.match(index, /<a id="to-emulator"[^>]* href="\.\.\/emulator\/"[^>]*>.*<span>Calculator<\/span><\/a>/);
    assert.ok(files.includes("shared/site-links.js"));
    assert.match(index, /<link rel="manifest" href="manifest.webmanifest">/);
    assert.match(index, /http-equiv="Content-Security-Policy"/);
    const manifest = JSON.parse(readFileSync(join(tx, "manifest.webmanifest"), "utf8"));
    assert.equal(manifest.short_name, "saturnus tx");
    for (const icon of manifest.icons) assert.ok(files.includes(icon.src), `${icon.src} shipped`);
    // The same worker as the emulator's, behind its own constants: its caches are its scope's.
    assert.equal(readFileSync(join(tx, "sw.js"), "utf8").split("\n\n").slice(1).join("\n\n"), readFileSync(join(out, "emulator", "sw.js"), "utf8").split("\n\n").slice(1).join("\n\n"));
  } finally {
    rmSync(out, { recursive: true, force: true });
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
});
