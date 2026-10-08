// The site web/site.sh assembles, as the installed page needs it: the
// service worker precaches exactly the files the site ships (but itself,
// the .htaccess and the marker), every page file among them; its build
// hash stays put for the same files and changes with any of them; the
// page links the manifest; the desktop app's file list has none of it.
// Skipped without the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFileSync, cpSync, existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const WEB = fileURLToPath(new URL("../", import.meta.url));
const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));

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

/** Build the site of `web` into a new directory; its sw.js's BUILD and FILES. */
function site(web) {
  const out = mkdtempSync(join(tmpdir(), "saturnus-site-"));
  execFileSync("sh", [join(web, "site.sh"), out], { stdio: "ignore" });
  const sw = readFileSync(join(out, "sw.js"), "utf8");
  const build = sw.match(/^const BUILD = "([0-9a-f]{16})";$/m)?.[1];
  const list = sw.match(/^const FILES = (\[[^\]]*\]);$/m)?.[1];
  assert.ok(build && list, "sw.js starts with BUILD and FILES");
  return { out, build, files: JSON.parse(list.replace(/,\s*\]$/, "]")) };
}

test("the service worker precaches exactly the site's files, and a new build is a new hash", { skip: !built && "web/pkg not built (just web)" }, () => {
  const tmp = [];
  try {
    const a = site(WEB);
    tmp.push(a.out);
    const shipped = filesUnder(a.out).filter((f) => ![".htaccess", ".saturnus-site", "sw.js"].includes(f));
    assert.deepEqual(a.files, shipped);
    for (const f of ["index.html", "manifest.webmanifest", "pkg/saturnus_web.js", "pkg/saturnus_web_bg.wasm", "icons/icon-512.png", "icons/maskable-512.png", "icons/apple-touch-icon.png"]) {
      assert.ok(a.files.includes(f), `${f} precached`);
    }
    const pageFiles = execFileSync("sh", [join(WEB, "site.sh"), "--list"], { encoding: "utf8" }).trim().split("\n");
    for (const f of pageFiles) assert.ok(a.files.includes(f), `page file ${f} precached`);
    // The desktop app embeds the page files: no worker, no manifest.
    assert.ok(!pageFiles.some((f) => /sw\.js$|manifest|^pwa\//.test(f)), "the app's list has no installable-page file");

    const index = readFileSync(join(a.out, "index.html"), "utf8");
    assert.match(index, /<link rel="manifest" href="manifest.webmanifest">/);
    assert.match(index, /<link rel="apple-touch-icon" href="icons\/apple-touch-icon.png">/);
    const manifest = JSON.parse(readFileSync(join(a.out, "manifest.webmanifest"), "utf8"));
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
  } finally {
    for (const d of tmp) rmSync(d, { recursive: true, force: true });
  }
});
