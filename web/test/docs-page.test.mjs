// The docs' command reference page in headless Chrome (iteration 57,
// redesigned twice after the owner's reviews): built by web/docs/build.mjs
// and served as on the site, docs/ beside the app's files (app/ is web/),
// it runs the app's own reference code with no ROM and no calculator:
// the address picks the model and the command (`#48gx/hms-`), the
// entry is the app's, read-only (no Run, Type or Try it), its model
// pills lead to the same command on the other models, the search
// suggests as Search ranks (`->list` is →LIST; the ten best, then "All
// n matches"), the navigation lists the commands by menu, one folded
// group per menu with the chosen command's open, every command a row,
// choosing a row or another model updates the address, a key-driven
// model's function shows its keys. On a desktop the navigation and the
// entry are two cards down to the bottom of the screen, each scrolling
// inside itself, and choosing a command further down docks the page
// with the bar at the top; on a phone a sheet that Browse opens and a chosen
// command closes. The docs index loads with its model links in their
// colours, a model page in its own; the header's "saturnus" leads to
// the landing page, "docs" to the index; nothing is wider than a
// phone. Skipped without Chrome or the wasm package.
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

test("the command reference page: the addressed command read-only, suggestions as Search ranks, the navigation by menu beside it, the address kept", { timeout: 90_000 }, async (t) => {
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
  // Two cards on a desktop, the navigation on the left and the entry on the right, down to the
  // bottom of the screen from where they start (the bar stays at the top), each scrolling inside itself; no Browse button.
  const boxes = () => ev(`JSON.stringify({ side: document.getElementById("ref-side").getBoundingClientRect().toJSON(), entry: document.getElementById("ref-entry").getBoundingClientRect().toJSON(), bar: document.getElementById("ref-bar").getBoundingClientRect().toJSON(), lead: document.querySelector("article.docs-page > p").getBoundingClientRect().toJSON(), h: innerHeight })`).then(JSON.parse);
  let box = await boxes();
  assert.ok(box.side.right < box.entry.left, "the navigation left of the entry");
  assert.equal(box.side.top, box.entry.top, "the cards' tops aligned");
  assert.equal(box.side.height, box.entry.height, "the cards as tall as each other");
  assert.ok(Math.abs(box.side.bottom - (box.h - 16)) <= 1, `the cards end at the bottom of the screen in the first view: ${JSON.stringify(box)}`);
  assert.equal(box.side.left, box.bar.left + 16, "the cards on the bar's grid");
  assert.equal(box.entry.right, box.bar.right - 16);
  assert.equal(box.lead.left, box.side.left, "the lead on the same grid");
  assert.equal(box.lead.right, box.entry.right, "the lead the whole width");
  assert.ok(box.bar.top > 150, "the title and the lead above the bar before the page scrolls");
  for (const id of ["ref-side", "ref-entry"]) {
    assert.equal(await ev(`getComputedStyle(document.getElementById(${JSON.stringify(id)})).overflowY`), "auto", `${id} scrolls inside itself`);
    assert.equal(await ev(`getComputedStyle(document.getElementById(${JSON.stringify(id)})).backgroundColor`), "rgb(253, 252, 249)", `${id} is a card`);
  }
  assert.ok(await ev(`(() => { const s = document.getElementById("ref-side"); return s.scrollHeight > s.clientHeight; })()`), "the navigation is longer than its card");
  assert.equal(await ev(`document.getElementById("ref-nav").parentElement.id`), "ref-side");
  assert.equal(await ev(`getComputedStyle(document.getElementById("ref-browse")).display`), "none");
  assert.equal(await ev(`getComputedStyle(document.getElementById("ref-bar")).position`), "sticky", "the search bar stays at the top");
  assert.equal(await ev(`getComputedStyle(document.querySelector(".ref-nav-title")).position`), "sticky", "the navigation's title stays at the top of its card");
  // The model's pill in the field: solid, in the accent.
  assert.equal(await ev(`getComputedStyle(document.getElementById("ref-model")).backgroundColor`), "rgb(42, 127, 130)");
  assert.equal(await ev(`document.querySelectorAll("#ref-nav .ref-skeleton").length`), 0, "the skeleton gone");
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
  // The model pills are links to the same command on the other models, the shown one marked
  // (drawn again once the other models' slugs are built, after the first render).
  await until(ev, `document.body.dataset.slugs === "true"`, 10_000, "the other models' slugs");
  assert.deepEqual(await ev(`[...document.querySelectorAll("#ref-entry .entry-models .chip")].map((c) => [c.tagName, c.getAttribute("href"), c.getAttribute("aria-current")])`),
    [["A", "#48sx/hms-", null], ["A", "#48gx/hms-", "true"], ["A", "#49g/hms-", null], ["A", "#28s/hms-", null]]);
  // The navigation: a folded group per menu, every command a row with its long name, the
  // chosen one's group open and the row marked and in view; all of them A to Z last.
  assert.ok(await ev(`document.querySelectorAll("#ref-nav details.ref-group").length`) >= 16, "the 48GX's menus");
  assert.deepEqual(await ev(`[...document.querySelectorAll("#ref-nav details[open] > summary .ref-group-name")].map((n) => n.textContent)`), ["TIME (MENU 94)"], "the chosen command's menu open, the others folded");
  assert.equal(await ev(`document.querySelectorAll("#ref-nav .ref-all .ref-item").length`), 517);
  assert.deepEqual(await ev(`[...document.querySelectorAll('#ref-nav .ref-item[aria-current="true"]')].map((a) => a.closest(".ref-all") ? "all" : a.closest(".ref-group").querySelector(".ref-group-name").textContent)`), ["TIME (MENU 94)", "all"]);
  assert.equal(await ev(`document.querySelector('#ref-nav .ref-item[aria-current="true"] .ref-item-long').textContent`), "Hours-minutes-seconds minus");
  assert.ok(await ev(`(() => { const r = document.querySelector('#ref-nav .ref-item[aria-current="true"]').getBoundingClientRect(); return r.top >= 0 && r.bottom <= innerHeight; })()`), "the chosen row in view");
  assert.equal(await ev("scrollY"), 0, "the page itself did not scroll on loading");
  // Choosing a command in the first view leaves the page where it is: the cards already end at the
  // bottom of the screen.
  await ev(`[...document.querySelectorAll("#ref-nav .ref-item")].find((a) => a.dataset.name === "HMS→").click(), true`);
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "HMS→"`, 5_000, "HMS→ shown");
  await sleep(150);
  assert.equal(await ev("scrollY"), 0, "no docking when the cards fit");
  // Chosen from further down the page, it docks: the bar at the top, the cards on the screen under
  // it, their last rows reachable by scrolling inside them; the wheel at the navigation's end moves the page.
  await ev(`scrollTo(0, 3000), true`);
  await ev(`[...document.querySelectorAll("#ref-nav .ref-item")].find((a) => a.dataset.name === "HMS+").click(), true`);
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "HMS+"`, 5_000, "HMS+ shown");
  await sleep(150);
  box = await boxes();
  const dock = await ev("scrollY");
  assert.ok(dock > 150 && box.bar.top === 0, `the page docked: ${JSON.stringify({ dock, bar: box.bar })}`);
  assert.ok(box.side.bottom <= box.h && box.entry.bottom <= box.h && box.side.top > box.bar.bottom, `the cards on the screen under the bar: ${JSON.stringify(box)}`);
  await until(ev, `document.getElementById("ref-bar").classList.contains("stuck")`, 2_000, "the bar marked stuck");
  const fit = await ev(`(() => { const s = document.getElementById("ref-side"); s.scrollTop = s.scrollHeight; const r = s.getBoundingClientRect(); const last = [...s.querySelectorAll("summary, .ref-item")].filter((x) => x.checkVisibility()).at(-1).getBoundingClientRect(); return JSON.stringify({ bottom: Math.round(r.bottom), last: Math.round(last.bottom) }); })()`).then(JSON.parse);
  assert.ok(fit.last <= fit.bottom, `the navigation's last row reachable inside its card: ${JSON.stringify(fit)}`);
  for (let i = 0; i < 10 && !(await ev("scrollY") > dock); i++) {
    await send("Input.dispatchMouseEvent", { type: "mouseWheel", x: 180, y: 500, deltaX: 0, deltaY: 300 });
    await sleep(200);
  }
  assert.ok(await ev("scrollY") > dock, "the wheel at the navigation's end scrolls the page");
  // Chosen again from below the cards (the search bar is still there): the page docks again.
  await ev(`scrollTo(0, 3000), true`);
  await ev(`[...document.querySelectorAll("#ref-nav .ref-item")].find((a) => a.dataset.name === "HMS-").click(), true`);
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "HMS-"`, 5_000, "HMS- shown again");
  await sleep(150);
  assert.equal(await ev("scrollY"), dock, "docked again");
  // Cmd-K (or Ctrl-K) starts a search from anywhere on the page.
  await ev(`document.activeElement.blur(), document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "k", metaKey: true, bubbles: true, cancelable: true })), true`);
  assert.equal(await ev(`document.activeElement.id`), "ref-q", "Cmd-K focuses the search");
  await ev(`document.activeElement.blur(), document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "K", ctrlKey: true, bubbles: true, cancelable: true })), true`);
  assert.equal(await ev(`document.activeElement.id`), "ref-q", "Ctrl-K focuses the search");
  await ev(`document.activeElement.blur(), true`);
  await ev(`scrollTo(0, 0), true`);
  await ev(`document.getElementById("ref-side").scrollTop = 0, true`);
  await sleep(150);
  assert.match(await ev(`document.querySelector("#ref-nav .ref-nav-title").textContent`), /^The HP 48GX's 517 commands, by menu$/);
  assert.equal(await ev(`[...document.querySelectorAll("#ref-nav .ref-group")].find((d) => d.querySelector(".ref-group-name").textContent === "TIME (MENU 94)").querySelector(".ref-n").textContent`), "19");
  // The arrow keys walk the navigation's rows and groups.
  await ev(`document.querySelector('#ref-nav .ref-item[aria-current="true"]').focus(), true`);
  const navKey = (k) => ev(`document.activeElement.dispatchEvent(new KeyboardEvent("keydown", { key: ${JSON.stringify(k)}, bubbles: true, cancelable: true })), true`);
  await navKey("ArrowDown");
  assert.equal(await ev(`document.activeElement.querySelector(".ref-item-name")?.textContent`), "HMS→");
  await navKey("ArrowUp");
  await navKey("ArrowUp");
  assert.equal(await ev(`document.activeElement.querySelector(".ref-item-name")?.textContent`), "DDAYS");
  await navKey("Home");
  assert.equal(await ev(`document.activeElement.tagName + " " + document.activeElement.querySelector(".ref-group-name").textContent`), "SUMMARY MTH");

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
  assert.equal(await ev(`document.getElementById("ref-q").hasAttribute("aria-activedescendant")`), false, "nothing active yet");
  // Nothing found: the list stays closed, the count line says so.
  await type("zzzzqq");
  assert.equal(await ev(`document.getElementById("ref-suggest").hidden`), true);
  assert.equal(await ev(`document.getElementById("ref-q").getAttribute("aria-expanded")`), "false");
  assert.match(await ev(`document.getElementById("ref-count").textContent`), /^0 of 517 HP 48GX commands match “zzzzqq”\.$/);
  await type("sto");
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
  // The navigation follows: STO-'s menu opens too (TIME stays open), the marked row moves.
  assert.deepEqual(await ev(`[...document.querySelectorAll('#ref-nav .ref-item[aria-current="true"] .ref-item-name')].map((n) => n.textContent)`), ["STO-", "STO-"]);
  assert.ok(await ev(`[...document.querySelectorAll("#ref-nav details[open] > summary .ref-group-name")].map((n) => n.textContent).includes("TIME (MENU 94)")`), "an open group stays open");
  // Enter with nothing chosen opens the first.
  await type("->list");
  await key("Enter");
  await until(ev, `location.hash === "#48gx/to-list"`, 5_000, "the address of →LIST");
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "→LIST"`, 5_000, "→LIST shown");
  // "All n matches" lists every match in the navigation, best first; clearing the search brings the menus back.
  await type("sto");
  await ev(`document.querySelector("#ref-suggest .ref-more").click(), true`);
  assert.match(await ev(`document.querySelector("#ref-nav .ref-nav-title").textContent`), /^\d+ HP 48GX commands match “sto”$/);
  assert.equal(await ev(`document.querySelector("#ref-nav .ref-item .ref-item-name").textContent`), "STO");
  assert.equal(await ev(`document.querySelectorAll("#ref-nav .ref-group").length`), 0);
  const all = await ev(`document.querySelectorAll("#ref-nav .ref-item").length`);
  assert.equal(await ev(`document.getElementById("ref-count").textContent`), `${all} of 517 HP 48GX commands match “sto”.`, "the count after All n matches");
  // Another model while the matches show: its matches, its count.
  await ev(`(() => { const p = document.getElementById("ref-model"); p.value = "49g"; p.dispatchEvent(new Event("change")); return true; })()`);
  await until(ev, `document.getElementById("ref-model").value === "49g" && /HP 49G commands match/.test(document.querySelector("#ref-nav .ref-nav-title").textContent)`, 10_000, "the 49G's matches");
  assert.match(await ev(`document.getElementById("ref-count").textContent`), /^[1-9]\d* of \d+ HP 49G commands match “sto”\.$/);
  await ev(`(() => { const p = document.getElementById("ref-model"); p.value = "48gx"; p.dispatchEvent(new Event("change")); return true; })()`);
  await until(ev, `document.getElementById("ref-model").value === "48gx" && /HP 48GX commands match/.test(document.querySelector("#ref-nav .ref-nav-title").textContent)`, 10_000, "the 48GX's matches again");
  await type("");
  assert.ok(await ev(`document.querySelectorAll("#ref-nav details.ref-group").length`) >= 16, "the menus again");
  assert.equal(await ev(`document.getElementById("ref-suggest").hidden`), true);
  const marked = await ev(`[...document.querySelectorAll('#ref-nav .ref-item[aria-current="true"] .ref-item-name')].map((n) => n.textContent)`);
  assert.ok(marked.length >= 2 && marked.every((n) => n === "→LIST"), `the chosen command marked again, in each of its menus: ${marked}`);

  // A pill: the same command on another model.
  await ev(`document.querySelector('#ref-entry .chip[href="#49g/to-list"]').click(), true`);
  await until(ev, `location.hash === "#49g/to-list"`, 5_000, "the 49G's address");
  await until(ev, `document.querySelector("#ref-entry .chip.current")?.textContent === "49G"`, 10_000, "the 49G's entry");
  assert.equal(await ev(`document.getElementById("ref-model").value`), "49g");
  // The model picker too.
  await ev(`(() => { const p = document.getElementById("ref-model"); p.value = "48sx"; p.dispatchEvent(new Event("change")); return true; })()`);
  await until(ev, `location.hash === "#48sx/to-list"`, 5_000, "the 48SX's address");
  await until(ev, `document.querySelector("#ref-entry .chip.current")?.textContent === "48SX"`, 10_000, "the 48SX's entry");

  // A row is a link to its address.
  const dup = await ev(`(() => { const a = [...document.querySelectorAll("#ref-nav .ref-all .ref-item")].find((x) => x.dataset.name === "DUP"); a.click(); return a.getAttribute("href"); })()`);
  assert.equal(dup, "#48sx/dup");
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "DUP"`, 5_000, "DUP shown");
  // The same model: the groups stay, the marked row moves.
  assert.deepEqual(await ev(`[...document.querySelectorAll('#ref-nav .ref-all .ref-item[aria-current="true"]')].map((a) => a.dataset.name)`), ["DUP"]);
  assert.equal(await ev(`document.querySelectorAll('#ref-nav .ref-item[aria-current]').length`), await ev(`document.querySelectorAll('#ref-nav .ref-item[aria-current="true"]').length`), "aria-current is true, never empty");
  // The entry's card scrolled into a long entry: a new command shows from its top, the card docked under the bar.
  await ev(`document.getElementById("ref-entry").scrollTop = 300, true`);
  await ev(`scrollTo(0, 600), true`);
  await ev(`[...document.querySelectorAll("#ref-nav .ref-all .ref-item")].find((x) => x.dataset.name === "DROP").click(), true`);
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "DROP"`, 5_000, "DROP shown");
  await sleep(200);
  assert.equal(await ev(`document.getElementById("ref-entry").scrollTop`), 0, "the entry from its top");
  assert.ok(await ev(`(() => { const top = document.getElementById("ref-entry").getBoundingClientRect().top; const bar = document.getElementById("ref-bar").getBoundingClientRect().bottom; return top >= bar && top < bar + 40; })()`), "the entry's top under the bar");

  // Where a command is, one rule for every model (BAR): the Menu row where the data has menus, the Key
  // row the manual's key alone, no "Group" row, the manuals' pages only under "In the manuals", tagged
  // with their models where those are not the shown one; an arrow alone as text, not a box; the
  // examples' heading says they are results, since nothing runs here.
  const rows = () => ev(`[...document.querySelectorAll("#ref-entry .entry-where dt")].map((dt) => dt.textContent + ": " + dt.nextElementSibling.textContent)`);
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#48sx/bar` });
  await until(ev, `document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "BAR" && document.querySelector("#ref-entry .chip.current")?.textContent === "48SX"`, 10_000, "BAR on the 48SX");
  assert.deepEqual(await rows(), ["Menu: PLOT PTYPE", "Key: PLOT"]);
  assert.equal(await ev(`document.querySelectorAll("#ref-entry .entry-where a").length`), 0, "no manual link in the rows");
  assert.deepEqual(await ev(`[...document.querySelectorAll("#ref-entry .entry-manuals li")].map((li) => li.textContent)`), ["HP 48G Series Advanced User's Reference Manual, p. 198"]);
  assert.equal(await ev(`document.querySelector("#ref-entry .entry-stack-none").textContent`), "Takes nothing, returns nothing.");
  assert.equal(await ev(`document.querySelectorAll("#ref-entry .entry-stack").length`), 0, "no box for an arrow alone");
  assert.equal(await ev(`document.querySelector("#ref-entry .entry h4").textContent`), "Examples · results from saturnus's emulated HP 48SX");
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#49g/bar` });
  await until(ev, `document.querySelector("#ref-entry .chip.current")?.textContent === "49G"`, 10_000, "BAR on the 49G");
  assert.deepEqual(await rows(), [], "no Group row: our grouping is not a place on the calculator");
  assert.deepEqual(await ev(`[...document.querySelectorAll("#ref-entry .entry-manuals li")].map((li) => li.textContent)`), ["HP 48G Series Advanced User's Reference Manual, p. 198 (48SX, 48GX)"]);
  assert.equal(await ev(`document.querySelector("#ref-entry .entry-stack-none").textContent`), "Takes nothing, returns nothing.");

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
  assert.match(await ev(`document.querySelector("#ref-nav .ref-nav-title").textContent`), /^The HP 42S's 277 functions, by menu$/);
  assert.match(await ev(`document.getElementById("ref-count").textContent`), /^277 functions on the HP 42S\.$/);

  // No model in the address: the one the app last showed (same site, same storage); no entry, a line in its place, the menus.
  await ev(`localStorage.setItem("saturnus.model", "28s"), true`);
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/` });
  await until(ev, `document.body.dataset.ready === "true"`, 20_000, "the reference loaded");
  assert.equal(await ev(`document.getElementById("ref-model").value`), "28s");
  assert.equal(await ev(`document.querySelector("#ref-entry .ref-empty-title").textContent`), "Pick a command");
  assert.equal(await ev(`document.querySelectorAll("#ref-entry .entry").length`), 0);
  assert.equal(await ev(`document.querySelectorAll('#ref-nav [aria-current]').length`), 0, "nothing marked");
  assert.ok(await ev(`document.querySelectorAll("#ref-nav details.ref-group").length`) >= 20, "the 28S's menus");
  // An address the model has no command at says so.
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#28s/no-such` });
  await until(ev, `document.body.dataset.ready === "true" && document.querySelector("#ref-entry .ref-hint")`, 20_000, "the hint");
  assert.match(await ev(`document.querySelector("#ref-entry .ref-hint").textContent`), /^The HP 28S has no command at this address/);
  assert.deepEqual(errors, [], "no uncaught errors");
});

test("on a phone the command reference is one column: Browse opens the navigation as a sheet, a chosen command closes it", { timeout: 90_000 }, async (t) => {
  const out = builtDocs(t);
  const s = await session(t, { dirs: { "/docs/": join(out, "docs"), "/app/": WEB } });
  if (!s) return;
  const { send, ev, port, errors } = s;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/docs/commands/#48gx/hms-` });
  await until(ev, `document.body.dataset.ready === "true"`, 20_000, "the reference loaded");
  // One column: the navigation's column is not shown, the Browse button is.
  assert.equal(await ev(`getComputedStyle(document.getElementById("ref-side")).display`), "none");
  assert.notEqual(await ev(`getComputedStyle(document.getElementById("ref-browse")).display`), "none");
  assert.equal(await ev(`document.getElementById("ref-browse").textContent`), "Browse", "the button keeps its name");
  assert.equal(await ev(`document.getElementById("ref-sheet").open`), false);
  assert.equal(await ev(`document.querySelector("#ref-entry .entry-name").firstChild.textContent`), "HMS-");
  // Browse: the sheet with the navigation, the chosen command's group open and its row focused.
  await ev(`document.getElementById("ref-browse").click(), true`);
  await until(ev, `document.getElementById("ref-sheet").open`, 5_000, "the sheet open");
  assert.equal(await ev(`document.getElementById("ref-nav").closest("dialog")?.id`), "ref-sheet", "the navigation in the sheet");
  assert.equal(await ev(`document.activeElement.dataset.name`), "HMS-", "the chosen row focused");
  assert.deepEqual(await ev(`[...document.querySelectorAll("#ref-nav details[open] > summary .ref-group-name")].map((n) => n.textContent)`), ["TIME (MENU 94)"]);
  assert.ok(await ev(`(() => { const r = document.activeElement.getBoundingClientRect(); const d = document.querySelector(".ref-sheet-body").getBoundingClientRect(); return r.top >= d.top && r.bottom <= d.bottom; })()`), "the row in view");
  // Escape closes it; the navigation is back in its column.
  await ev(`document.getElementById("ref-sheet").dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true })), true`);
  await ev(`document.getElementById("ref-sheet").close(), true`);
  await until(ev, `!document.getElementById("ref-sheet").open && document.getElementById("ref-nav").parentElement.id === "ref-side"`, 5_000, "the sheet closed");
  // A command chosen in the sheet: its entry, the sheet closed, the entry in view.
  await ev(`scrollTo(0, 2000), true`);
  await ev(`document.getElementById("ref-browse").click(), true`);
  await until(ev, `document.getElementById("ref-sheet").open`, 5_000, "the sheet open again");
  await ev(`[...document.querySelectorAll("#ref-nav .ref-item")].find((a) => a.dataset.name === "TICKS").click(), true`);
  await until(ev, `location.hash === "#48gx/ticks" && document.querySelector("#ref-entry .entry-name")?.firstChild.textContent === "TICKS"`, 5_000, "TICKS shown");
  await until(ev, `!document.getElementById("ref-sheet").open && document.getElementById("ref-nav").parentElement.id === "ref-side"`, 5_000, "the sheet closed by the choice, the navigation back in its column");
  await sleep(200);
  assert.ok(await ev(`(() => { const top = document.getElementById("ref-entry").getBoundingClientRect().top; return top >= 0 && top < innerHeight * 0.6; })()`), "the entry in view");
  // "All n matches" opens the sheet with the matches.
  await ev(`(() => { const q = document.getElementById("ref-q"); q.focus(); q.value = "sto"; q.dispatchEvent(new Event("input")); return true; })()`);
  await ev(`document.querySelector("#ref-suggest .ref-more").click(), true`);
  await until(ev, `document.getElementById("ref-sheet").open`, 5_000, "the sheet with the matches");
  assert.match(await ev(`document.querySelector("#ref-nav .ref-nav-title").textContent`), /^\d+ HP 48GX commands match “sto”$/);
  await ev(`document.querySelector(".ref-sheet-close").click(), true`);
  await until(ev, `!document.getElementById("ref-sheet").open`, 5_000, "the sheet closed by its button");
  // Nothing wider than the phone, the sheet included.
  assert.equal(await ev(`document.documentElement.scrollWidth - document.documentElement.clientWidth`), 0);
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
  // The header: "saturnus" leads to the landing page, "docs" to the docs' index, "Open the app" to the app.
  assert.deepEqual(await ev(`[...document.querySelectorAll(".docs-bar .brand, .docs-bar .docs-brand-sub, .docs-bar .docs-app")].map((a) => [a.textContent.trim(), new URL(a.href).pathname])`), [["saturnus", "/"], ["docs", "/docs/"], ["Open the app", "/app/"]]);
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
