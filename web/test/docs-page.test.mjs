// The docs' command reference page in headless Chrome (iteration 57,
// redesigned after the owner's review): built by web/docs/build.mjs and
// served as on the site, docs/ beside the app's files (app/ is web/),
// it runs the app's own reference code with no ROM and no calculator:
// the address picks the model and the command (`#48gx/hms-`), the
// entry is the app's, read-only (no Run, Type or Try it), its model
// pills lead to the same command on the other models, the search
// suggests as Search ranks (`->list` is →LIST; the ten best, then "All
// n matches"), the commands to browse are grouped by menu with every
// one a chip, choosing a chip or another model updates the address, a
// key-driven model's function shows its keys. The docs index loads with
// its model links in their colours, a model page in its own; nothing is
// wider than a phone. Skipped without Chrome or the wasm package.
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

test("the command reference page shows the addressed command read-only, suggests as Search ranks, browses by menu and keeps the address", { timeout: 90_000 }, async (t) => {
  const out = builtDocs(t);
  const s = await session(t, { dirs: { "/docs/": join(out, "docs"), "/app/": WEB } });
  if (!s) return;
  const { send, ev, port, errors } = s;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#48gx/hms-` });
  await until(ev, `document.body.dataset.ready === "true"`, 20_000, "the reference loaded");

  // The page starts with the search, not with text: one lead paragraph before it, the rest after.
  assert.equal(await ev(`document.querySelectorAll("article.docs-page > p").length`), 1, "one paragraph before the tool");
  assert.ok(await ev(`document.querySelector(".docs-after h2#reading-an-example") !== null`), "the sections after the tool");
  // The addressed command on the addressed model, as the app's entry shows it.
  assert.equal(await ev(`document.getElementById("ref-model").value`), "48gx");
  assert.equal(await ev(`document.querySelectorAll("#ref-model option").length`), 12);
  assert.equal(await ev(`document.querySelector("#ref-entry .entry-name").firstChild.textContent`), "HMS-");
  assert.equal(await ev(`document.querySelector("#ref-entry .chip.current").textContent`), "48GX");
  assert.ok(await ev(`document.querySelectorAll("#ref-entry .ex").length`) >= 1, "its examples");
  // Read-only: nothing to send to a calculator.
  assert.equal(await ev(`document.querySelectorAll("#ref-entry button").length`), 0, "no Run, Type or Try it");
  assert.match(await ev("document.title"), /^HMS- on the HP 48GX · Command reference/);
  assert.match(await ev(`document.getElementById("ref-count").textContent`), /^517 commands on the HP 48GX\.$/);
  // The entry has the app's look (style.css's rules, scoped for the docs too).
  assert.equal(await ev(`getComputedStyle(document.querySelector("#ref-entry .ex")).display`), "grid");
  // The model pills are links to the same command on the other models, the shown one marked.
  assert.deepEqual(await ev(`[...document.querySelectorAll("#ref-entry .entry-models .chip")].map((c) => [c.tagName, c.getAttribute("href"), c.getAttribute("aria-current")])`),
    [["A", "#48sx/hms-", null], ["A", "#48gx/hms-", "true"], ["A", "#49g/hms-", null], ["A", "#28s/hms-", null]]);
  // The commands to browse: a folded group per menu, every command a chip, the shown one marked; all of them A to Z last.
  assert.ok(await ev(`document.querySelectorAll("#ref-browse details.ref-group").length`) >= 16, "the 48GX's menus");
  assert.equal(await ev(`document.querySelectorAll("#ref-browse details[open]").length`), 0, "folded");
  assert.equal(await ev(`document.querySelectorAll("#ref-browse .ref-all .ref-chip").length`), 517);
  assert.equal(await ev(`document.querySelector('#ref-browse .ref-all .ref-chip[aria-current="true"]').textContent`), "HMS-");
  assert.match(await ev(`document.querySelector("#ref-browse .ref-browse-title").textContent`), /^The HP 48GX's 517 commands, by menu$/);
  assert.equal(await ev(`[...document.querySelectorAll("#ref-browse .ref-group")].find((d) => d.querySelector(".ref-group-name").textContent === "TIME (MENU 94)").querySelector(".ref-n").textContent`), "19");

  // Search suggests as the app's Search ranks: a friendly spelling finds the name first; the ten best, then all of them.
  const type = (text) => ev(`(() => { const q = document.getElementById("ref-q"); q.focus(); q.value = ${JSON.stringify(text)}; q.dispatchEvent(new Event("input")); return true; })()`);
  await type("->list");
  assert.equal(await ev(`document.querySelector("#ref-suggest .ref-name").textContent`), "→LIST");
  assert.equal(await ev(`document.getElementById("ref-q").getAttribute("aria-expanded")`), "true");
  assert.match(await ev(`document.getElementById("ref-count").textContent`), /of 517 HP 48GX commands match “->list”\.$/);
  await type("sto");
  assert.equal(await ev(`document.querySelectorAll('#ref-suggest [role="option"] a').length`), 10, "the ten best");
  assert.equal(await ev(`document.querySelector("#ref-suggest .ref-name").textContent`), "STO");
  assert.match(await ev(`document.querySelector("#ref-suggest .ref-more").textContent`), /^All \d+ matches$/);
  // The arrow keys choose a suggestion; Enter opens it at its readable address.
  const key = (k) => ev(`document.getElementById("ref-q").dispatchEvent(new KeyboardEvent("keydown", { key: ${JSON.stringify(k)}, bubbles: true, cancelable: true })), true`);
  await key("ArrowDown");
  await key("ArrowDown");
  assert.equal(await ev(`document.querySelector('#ref-suggest [aria-selected="true"] .ref-name').textContent`), "STO-");
  assert.equal(await ev(`document.getElementById("ref-q").getAttribute("aria-activedescendant")`), "ref-opt-1");
  await key("Enter");
  await until(ev, `location.hash === "#48gx/sto-"`, 5_000, "the address of STO-");
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "STO-"`, 5_000, "STO- shown");
  assert.equal(await ev(`document.getElementById("ref-suggest").hidden`), true, "the suggestions close");
  // Enter with nothing chosen opens the first.
  await type("->list");
  await key("Enter");
  await until(ev, `location.hash === "#48gx/to-list"`, 5_000, "the address of →LIST");
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "→LIST"`, 5_000, "→LIST shown");
  // "All n matches" lists every match in the browse area, best first; clearing the search brings the menus back.
  await type("sto");
  await ev(`document.querySelector("#ref-suggest .ref-more").click(), true`);
  assert.match(await ev(`document.querySelector("#ref-browse .ref-browse-title").textContent`), /^\d+ HP 48GX commands match “sto”$/);
  assert.equal(await ev(`document.querySelector("#ref-browse .ref-chip").textContent`), "STO");
  assert.equal(await ev(`document.querySelectorAll("#ref-browse .ref-group").length`), 0);
  await type("");
  assert.ok(await ev(`document.querySelectorAll("#ref-browse details.ref-group").length`) >= 16, "the menus again");
  assert.equal(await ev(`document.getElementById("ref-suggest").hidden`), true);

  // A pill: the same command on another model.
  await ev(`document.querySelector('#ref-entry .chip[href="#49g/to-list"]').click(), true`);
  await until(ev, `location.hash === "#49g/to-list"`, 5_000, "the 49G's address");
  await until(ev, `document.querySelector("#ref-entry .chip.current")?.textContent === "49G"`, 10_000, "the 49G's entry");
  assert.equal(await ev(`document.getElementById("ref-model").value`), "49g");
  // The model picker too.
  await ev(`(() => { const p = document.getElementById("ref-model"); p.value = "48sx"; p.dispatchEvent(new Event("change")); return true; })()`);
  await until(ev, `location.hash === "#48sx/to-list"`, 5_000, "the 48SX's address");
  await until(ev, `document.querySelector("#ref-entry .chip.current")?.textContent === "48SX"`, 10_000, "the 48SX's entry");

  // A chip is a link to its address.
  const dup = await ev(`(() => { const a = [...document.querySelectorAll("#ref-browse .ref-all .ref-chip")].find((x) => x.textContent === "DUP"); a.click(); return a.getAttribute("href"); })()`);
  assert.equal(dup, "#48sx/dup");
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "DUP"`, 5_000, "DUP shown");

  // A model without the command: its pill is no link.
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#48gx/fft` });
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "FFT"`, 5_000, "FFT shown");
  assert.deepEqual(await ev(`[...document.querySelectorAll("#ref-entry .entry-models .chip")].map((c) => c.tagName + (c.classList.contains("off") ? " off" : ""))`), ["SPAN off", "A", "A", "SPAN off"], "the 48SX and the 28S have no FFT");

  // A key-driven model: its function's keys, no command line; its menus to browse.
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#42s/pct` });
  await until(ev, `document.body.dataset.ready === "true" && document.querySelector("#ref-entry .entry-name")`, 20_000, "the 42S's %");
  assert.equal(await ev(`document.querySelector("#ref-entry .entry-name").firstChild.textContent`), "%");
  assert.deepEqual(await ev(`[...document.querySelectorAll("#ref-entry .entry-keys kbd")].map((k) => k.textContent)`), ["⇧", "%"]);
  assert.equal(await ev(`document.querySelectorAll("#ref-entry button").length`), 0);
  assert.match(await ev(`document.querySelector("#ref-browse .ref-browse-title").textContent`), /^The HP 42S's 277 functions, by menu$/);

  // No model in the address: the one the app last showed (same site, same storage); no entry, the menus.
  await ev(`localStorage.setItem("saturnus.model", "28s"), true`);
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/` });
  await until(ev, `document.body.dataset.ready === "true"`, 20_000, "the reference loaded");
  assert.equal(await ev(`document.getElementById("ref-model").value`), "28s");
  assert.equal(await ev(`document.getElementById("ref-entry").hidden`), true);
  assert.ok(await ev(`document.querySelectorAll("#ref-browse details.ref-group").length`) >= 20, "the 28S's menus");
  // An address the model has no command at says so.
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#28s/no-such` });
  await until(ev, `document.body.dataset.ready === "true" && document.querySelector("#ref-entry .ref-hint")`, 20_000, "the hint");
  assert.match(await ev(`document.querySelector("#ref-entry .ref-hint").textContent`), /^The HP 28S has no command at this address/);
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
      await until(ev, `document.readyState === "complete" && (!document.getElementById("ref-q") || document.body.dataset.ready === "true")`, 20_000, path);
      const over = await ev(`document.documentElement.scrollWidth - document.documentElement.clientWidth`);
      assert.equal(over, 0, `${path} at ${width} px is ${over} px too wide`);
      // The header is one row: the brand, the section links scrolling sideways, the app as its icon.
      const bar = await ev(`document.querySelector(".docs-bar").getBoundingClientRect().height`);
      assert.ok(bar <= 56, `${path} at ${width} px: the header is ${bar} px tall`);
      assert.ok(await ev(`(() => { const n = document.querySelector(".docs-nav"); return n.scrollWidth > n.clientWidth && getComputedStyle(n).overflowX === "auto"; })()`), `${path} at ${width} px: the links scroll`);
      assert.equal(await ev(`getComputedStyle(document.querySelector(".docs-app-ic")).display`), "block");
      assert.equal(await ev(`document.querySelector(".docs-app").textContent`), "Open the app", "the link keeps its name");
    }
  }
  // On a desktop the header is as it was: the links wrapping, "Open the app" as text.
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/` });
  await until(ev, `document.readyState === "complete"`, 10_000, "the index again");
  assert.equal(await ev(`getComputedStyle(document.querySelector(".docs-app-ic")).display`), "none");
  assert.deepEqual(errors, [], "no uncaught errors");
});
