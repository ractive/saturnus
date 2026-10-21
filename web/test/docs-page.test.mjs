// The docs' command reference page in headless Chrome (iteration 57):
// built by web/docs/build.mjs and served as on the site, docs/ beside
// the app's files (app/ is web/), it runs the app's own reference code
// with no ROM and no calculator: the address picks the model and the
// command (`#48gx/hms-`), the entry is the app's, read-only (no Run,
// Type or Try it), the search ranks as Search does (`->list` is →LIST),
// choosing a row or another model updates the address, a key-driven
// model's function shows its keys. The docs index loads with its model
// links in their colours, a model page in its own; nothing is wider than
// a phone. Skipped without Chrome or the wasm package.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { WEB, session, sleep } from "./chrome.mjs";
import { build } from "../docs/build.mjs";

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

/** The site's docs/ built into a folder of its own, removed after `t`; served with web/ as app/. */
function builtDocs(t) {
  const out = mkdtempSync(join(tmpdir(), "saturnus-docs-page-"));
  t.after(() => rmSync(out, { recursive: true, force: true }));
  build(out);
  return out;
}

test("the command reference page shows the addressed command read-only, searches as Search does and keeps the address", { timeout: 90_000 }, async (t) => {
  const out = builtDocs(t);
  const s = await session(t, { dirs: { "/docs/": join(out, "docs"), "/app/": WEB } });
  if (!s) return;
  const { send, ev, port, errors } = s;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#48gx/hms-` });
  await until(ev, `document.body.dataset.ready === "true"`, 20_000, "the reference loaded");

  // The addressed command on the addressed model, as the app's entry shows it.
  assert.equal(await ev(`document.getElementById("ref-model").value`), "48gx");
  assert.equal(await ev(`document.querySelectorAll("#ref-model option").length`), 12);
  assert.equal(await ev(`document.querySelector("#ref-entry .entry-name").firstChild.textContent`), "HMS-");
  assert.equal(await ev(`document.querySelector("#ref-entry .chip.current").textContent`), "48GX");
  assert.ok(await ev(`document.querySelectorAll("#ref-entry .ex").length`) >= 1, "its examples");
  // Read-only: nothing to send to a calculator.
  assert.equal(await ev(`document.querySelectorAll("#ref-entry button").length`), 0, "no Run, Type or Try it");
  assert.equal(await ev(`document.querySelector('#ref-list [aria-current="true"] .ref-name').textContent`), "HMS-");
  assert.match(await ev("document.title"), /^HMS- on the HP 48GX · Command reference/);
  assert.match(await ev(`document.getElementById("ref-count").textContent`), /^517 commands on the HP 48GX\.$/);
  // The entry has the app's look (style.css's rules, scoped for the docs too).
  assert.equal(await ev(`getComputedStyle(document.querySelector("#ref-entry .ex")).display`), "grid");

  // Search ranks as the app's Search: a friendly spelling finds the name first.
  await ev(`(() => { const q = document.getElementById("ref-q"); q.value = "->list"; q.dispatchEvent(new Event("input")); return true; })()`);
  assert.equal(await ev(`document.querySelector("#ref-list .ref-name").textContent`), "→LIST");
  assert.match(await ev(`document.getElementById("ref-count").textContent`), /of 517 HP 48GX commands match “->list”\.$/);
  // Enter opens the first one: its readable address.
  await ev(`document.getElementById("ref-q").dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })), true`);
  await until(ev, `location.hash === "#48gx/to-list"`, 5_000, "the address of →LIST");
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "→LIST"`, 5_000, "→LIST shown");

  // Another model keeps the command where it has it.
  await ev(`(() => { const p = document.getElementById("ref-model"); p.value = "49g"; p.dispatchEvent(new Event("change")); return true; })()`);
  await until(ev, `location.hash === "#49g/to-list"`, 5_000, "the 49G's address");
  await until(ev, `document.querySelector("#ref-entry .chip.current")?.textContent === "49G"`, 10_000, "the 49G's entry");

  // A row is a link to its address.
  await ev(`(() => { const q = document.getElementById("ref-q"); q.value = ""; q.dispatchEvent(new Event("input")); return true; })()`);
  const dup = await ev(`(() => { const a = [...document.querySelectorAll("#ref-list a.ref-row")].find((x) => x.querySelector(".ref-name").textContent === "DUP"); a.click(); return a.getAttribute("href"); })()`);
  assert.equal(dup, "#49g/dup");
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "DUP"`, 5_000, "DUP shown");

  // A key-driven model: its function's keys, no command line.
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#42s/pct` });
  await until(ev, `document.body.dataset.ready === "true" && document.querySelector("#ref-entry .entry-name")`, 20_000, "the 42S's %");
  assert.equal(await ev(`document.querySelector("#ref-entry .entry-name").firstChild.textContent`), "%");
  assert.deepEqual(await ev(`[...document.querySelectorAll("#ref-entry .entry-keys kbd")].map((k) => k.textContent)`), ["⇧", "%"]);
  assert.equal(await ev(`document.querySelectorAll("#ref-entry button").length`), 0);

  // No model in the address: the one the app last showed (same site, same storage).
  await ev(`localStorage.setItem("saturnus.model", "28s"), true`);
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/` });
  await until(ev, `document.body.dataset.ready === "true"`, 20_000, "the reference loaded");
  assert.equal(await ev(`document.getElementById("ref-model").value`), "28s");
  assert.match(await ev(`document.querySelector("#ref-entry .ref-hint").textContent`), /^Choose a command/);
  assert.deepEqual(errors, [], "no uncaught errors");
});

test("the docs index: the sections, the models with their colours, the Markdown beside it", { timeout: 60_000 }, async (t) => {
  const out = builtDocs(t);
  const s = await session(t, { dirs: { "/docs/": join(out, "docs"), "/app/": WEB } });
  if (!s) return;
  const { send, ev, port, errors } = s;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/` });
  await until(ev, `document.readyState === "complete"`, 10_000, "the index loaded");
  assert.equal(await ev("document.title"), "saturnus docs");
  assert.deepEqual(await ev(`[...document.querySelectorAll(".docs-nav a")].map((a) => a.textContent)`), ["Models", "Command reference", "How the Saturn works", "Moving files"]);
  assert.deepEqual(await ev(`[...document.querySelectorAll(".docs-page h2")].map((h) => h.textContent)`), ["Models", "Command reference", "How the Saturn works", "Moving files to a real calculator", "For AI apps"]);
  assert.equal(await ev(`document.querySelectorAll(".docs-model-link").length`), 12);
  // Each model's logo in its colours (style.css's table), the header's in the plain logo's.
  const fills = await ev(`[...document.querySelectorAll(".docs-model-link .planet")].map((p) => getComputedStyle(p).fill)`);
  assert.equal(new Set(fills).size >= 10, true, `the models' own colours: ${fills}`);
  assert.equal(await ev(`getComputedStyle(document.querySelector(".docs-bar .planet")).fill`), "rgb(217, 135, 74)");
  assert.equal(await ev(`document.querySelector('link[rel="alternate"]').getAttribute("href")`), "index.md");
  const md = await ev(`fetch("index.md").then((r) => r.text())`);
  assert.match(md, /^---\ntitle: saturnus docs\n/);
  // A model page: the model's colours.
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/48gx/` });
  await until(ev, `document.readyState === "complete"`, 10_000, "the 48GX page loaded");
  assert.equal(await ev(`document.documentElement.dataset.logo`), "48gx");
  assert.equal(await ev(`getComputedStyle(document.querySelector(".docs-page h1 .planet")).fill`), "rgb(140, 134, 204)");
  // On a phone nothing is wider than the window: the index, a model page, the reference.
  for (const width of [360, 390]) {
    await send("Emulation.setDeviceMetricsOverride", { width, height: 800, deviceScaleFactor: 1, mobile: true });
    for (const path of ["docs/", "docs/48gx/", "docs/commands/#49g/to-list"]) {
      await send("Page.navigate", { url: `http://127.0.0.1:${port}/${path}` });
      await until(ev, `document.readyState === "complete" && (!document.getElementById("ref-list") || document.body.dataset.ready === "true")`, 20_000, path);
      const over = await ev(`document.documentElement.scrollWidth - document.documentElement.clientWidth`);
      assert.equal(over, 0, `${path} at ${width} px is ${over} px too wide`);
    }
  }
  assert.deepEqual(errors, [], "no uncaught errors");
});
