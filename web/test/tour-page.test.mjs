// The tour (kb iteration 35) in the real page in headless Chrome: the
// quiet offer on a first visit and its dismissal kept; starting from
// Search and from About; Back, Next, Skip and Esc, the count, Done and
// the next chapter; the focus back where it was; the views put back; the
// calculator never touched; the bubble as a sheet at the bottom of a
// phone. And the guard against a UI change breaking a step: every step's
// anchor is seen, in light and dark, at 1280 and 390 px, in the browser
// and a stubbed app. No ROM: the memory view's reads are answered by the
// test. Skipped without Chrome (`SATURNUS_CHROME` names one) or the wasm
// package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

/** Poll `expression` in the page until it is truthy, at most `ms`. */
async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

// A 48GX "running": the memory reads answered here (HOME with MYDIR and
// X, two stack levels), and every command the page sends the host, or a
// write it asks the fake for, kept in `window.__sent`.
const FAKE = `(() => {
  const b = window.saturnus.backend;
  window.__sent = [];
  for (const m of ["request", "send"]) {
    const f = b[m].bind(b);
    b[m] = (cmd, ...rest) => { window.__sent.push(cmd); return f(cmd, ...rest); };
  }
  const v = (name, type, extra = {}) => ({ name, type, size: 16, checksum: 0x5B55, address: 0x7A000 + name.length, ...extra });
  const tree = { path: ["HOME"], variables: [v("MYDIR", "Directory", { variables: [v("A", "Real Number")] }), v("X", "Real Number")] };
  b.watchMemory = async () => ({ supported: true });
  b.memoryTree = async () => structuredClone(tree);
  b.stack = async () => [{ type: "real", text: "42" }, { type: "real", text: "7" }];
  b.flags = async () => ({ system: [], user: [], set: [] });
  b.objectAt = async () => ({ type: "real", text: "42" });
  for (const w of ["createDir", "changeDir", "setFlag", "purge", "rename", "storeFile", "keyDown", "keyUp", "insert", "replace", "reset", "pause"]) {
    b[w] = async () => { window.__sent.push(w); return { emulatedMs: 1, keys: false }; };
  }
  window.saturnus.store.set({ booted: "48gx", model: "48gx" });
  return true;
})()`;

/** The commands sent that only read or pace (none presses a key or writes). */
const READS = new Set(["watchMemory", "memoryTree", "stack", "flags", "objectAt", "commandLine", "stackTop", "visibility", "stats", "speed", "setSpeed", "skin"]);

/**
 * The page in headless Chrome, or null when the test skipped. `fresh`:
 * localStorage cleared first (a first visit); `fake`: the 48GX above.
 */
async function page(t, { width = 1280, height = 860, mobile = false, fake = true } = {}) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  const size = async (w, h, m) => {
    await send("Emulation.setTouchEmulationEnabled", { enabled: m, maxTouchPoints: m ? 5 : 1 });
    await send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 1, mobile: m });
  };
  await size(width, height, mobile);
  const load = async () => {
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    await until(ev, "!!window.saturnus", 15_000, "the page started");
    await ev("window.saturnus.started");
    await sleep(100);
    if (fake) await ev(FAKE);
  };
  await load();
  /** A named key, as the keyboard sends it. */
  const key = async (k, code = k, vk = 0) => {
    await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: k, code, windowsVirtualKeyCode: vk });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: vk });
    await sleep(150);
  };
  /** A real mouse click in the middle of `selector`'s element. */
  const click = async (selector) => {
    const [x, y] = await ev(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
    await sleep(150);
  };
  /** The bubble's state: open, step, count, title, Back shown, Next's word, the next chapter offered. */
  const bubble = () => ev(`(() => {
    const t = document.querySelector("sat-tour");
    const b = t.querySelector(".tour-bubble");
    if (!b.open) return null;
    return { step: t.dataset.step, count: b.querySelector(".tour-step").textContent, title: b.querySelector(".tour-title").textContent,
      back: !b.querySelector(".tour-back").hidden, next: b.querySelector(".tour-next").textContent,
      goOn: b.querySelector(".tour-next-chapter").hidden ? null : b.querySelector(".tour-go-on").textContent,
      focus: b.contains(document.activeElement) };
  })()`);
  /** Wait for the tour to settle on a step (or to have ended: null). */
  const settled = async () => {
    await ev("document.querySelector('sat-tour').busy");
    await sleep(50);
    return bubble();
  };
  return { ...c, size, load, key, click, bubble, settled };
}

test("the offer: a quiet line on a first visit; × dismisses it for good; taking the tour hides it", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { fake: false });
  if (!p) return;
  const shown = `!document.querySelector(".tour-offer").hidden`;
  assert.equal(await p.ev(shown), true, "offered on a first visit");
  assert.equal(await p.ev(`document.querySelector(".tour-offer").textContent.replace(/\\s+/g, " ").trim()`), "New here? Take a 2-minute tour.");
  assert.equal(await p.ev(`document.querySelector(".tour-offer").parentElement.id`), "stage", "on the stage, over the case");
  await p.click(".tour-offer-close");
  assert.equal(await p.ev(shown), false);
  assert.equal(await p.ev(`localStorage.getItem("saturnus.tour")`), "dismissed");
  await p.load();
  assert.equal(await p.ev(shown), false, "dismissed: not again");
  // Back to a first visit: the line starts the tour, and is not offered again.
  await p.ev(`localStorage.removeItem("saturnus.tour"), true`);
  await p.load();
  assert.equal(await p.ev(shown), true);
  await p.click(".tour-offer-go");
  const b = await p.settled();
  assert.equal(b.count, "1 of 7");
  assert.equal(b.step, "panel");
  assert.equal(await p.ev(shown), false);
  assert.equal(await p.ev(`localStorage.getItem("saturnus.tour")`), "taken");
  await p.key("Escape", "Escape", 27);
  await p.load();
  assert.equal(await p.ev(shown), false, "taken: not offered again");
});

test("on a phone the offer sits over the case, under the memory view lying over it", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 390, height: 844, mobile: true, fake: false });
  if (!p) return;
  const top = `(() => { const r = document.querySelector(".tour-offer-close").getBoundingClientRect(); return document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)?.closest(".tour-offer, sat-explorer")?.localName ?? null; })()`;
  assert.equal(await p.ev(top), "div", "the offer is on top of the calculator");
  await p.ev(`window.saturnus.setLayer(true).then(() => true)`);
  await sleep(300);
  assert.equal(await p.ev(top), "sat-explorer", "the memory view covers it");
});

test("Show me around: from Search and from About; Esc ends it and the focus goes back", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  // Search: the action is listed before anything is typed; the chapters are found by a search.
  const rows = (q) => p.ev(`(async () => {
    const pal = window.saturnus.palette;
    if (!pal.isOpen()) await pal.open();
    const i = pal.querySelector("input");
    i.value = ${JSON.stringify(q)};
    i.dispatchEvent(new Event("input"));
    await new Promise((r) => setTimeout(r, 150));
    return [...document.querySelectorAll(".palette .prow-action .prow-name")].map((e) => e.textContent);
  })()`);
  assert.ok((await rows("")).includes("Show me around"));
  assert.ok(!(await rows("")).some((n) => n.startsWith("Tour:")), "the chapters only for a search");
  assert.deepEqual((await rows("tour")).filter((n) => n.startsWith("Tour:")), ["Tour: Getting started", "Tour: The calculator", "Tour: The memory view"]);
  await rows("show me around");
  await p.ev(`(() => {
    const m = window.saturnus.palette.model;
    m.select(m.rows.findIndex((r) => r.kind === "action" && r.action.id === "tour"));
    document.querySelector(".palette input").dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    return true;
  })()`);
  await until(p.ev, `window.saturnus.tour.isOpen()`, 3_000, "the tour started");
  let b = await p.settled();
  assert.equal(await p.ev(`window.saturnus.palette.isOpen()`), false, "Search closed first");
  assert.equal(b.step, "panel");
  assert.ok(b.focus, "the bubble has the focus");
  assert.equal(await p.ev(`document.querySelector(".tour-bubble").getAttribute("aria-labelledby")`), "tour-title");
  await p.key("Escape", "Escape", 27);
  assert.equal(await p.bubble(), null, "Esc ends it");
  assert.ok(!(await p.ev(`window.__sent.includes("keyDown")`)), "Esc was not the calculator's ON");

  // About: its button closes About and starts the tour.
  await p.ev(`document.querySelector("#about").scrollIntoView({ block: "center" }), true`);
  await p.click("#about");
  await until(p.ev, `document.querySelector("sat-about dialog").open`, 3_000, "About opened");
  await p.ev(`document.querySelector("sat-about .tour-start").click(), true`);
  await until(p.ev, `window.saturnus.tour.isOpen()`, 3_000, "the tour started from About");
  b = await p.settled();
  assert.equal(await p.ev(`document.querySelector("sat-about dialog").open`), false, "About closed");
  assert.equal(b.step, "panel");
  await p.key("Escape", "Escape", 27);
  assert.equal(await p.bubble(), null);
  assert.equal(await p.ev(`document.querySelector("sat-tour").contains(document.activeElement)`), false, "the focus left the closed bubble");
});

test("Back, Next, Skip: the count, Done and the next chapter; the views put back; the calculator never touched", { timeout: 180_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const views = `[document.body.classList.contains("panel-hidden"), document.getElementById("roms").open, window.saturnus.store.state.layer, window.saturnus.explorer.tab]`;
  // The panel hidden and the memory view closed on the Stack tab, before the tour.
  await p.ev(`document.querySelector("#panel-hide").click(), window.saturnus.explorer.setTab("stack"), true`);
  const before = await p.ev(views);
  assert.deepEqual(before, [true, false, false, "stack"]);
  await p.ev(`document.querySelector("#palette-show").focus(), true`);
  await p.ev(`window.saturnus.tour.start("start")`);
  let b = await p.settled();
  assert.deepEqual([b.step, b.count, b.back, b.next], ["panel", "1 of 7", false, "Next"]);
  assert.equal(await p.ev(`document.body.classList.contains("panel-hidden")`), false, "the panel shown for its step");
  await p.click(".tour-next");
  b = await p.settled();
  assert.deepEqual([b.step, b.count, b.back], ["roms", "2 of 7", true]);
  assert.equal(await p.ev(`document.getElementById("roms").open`), true, "the ROM list opened for its step");
  await p.click(".tour-back");
  b = await p.settled();
  assert.deepEqual([b.step, b.count, b.back], ["panel", "1 of 7", false]);
  assert.ok(b.focus, "the focus stays in the bubble when Back goes away");
  for (let i = 0; i < 6; i++) await p.click(".tour-next");
  b = await p.settled();
  assert.deepEqual([b.step, b.count, b.next, b.goOn], ["again", "7 of 7", "Done", "Next: The calculator"]);
  // The next chapter from the last step; then the memory view's.
  await p.click(".tour-go-on");
  b = await p.settled();
  assert.deepEqual([b.step, b.count], ["keys", "1 of 7"]);
  for (let i = 0; i < 6; i++) await p.click(".tour-next");
  b = await p.settled();
  assert.deepEqual([b.step, b.next, b.goOn], ["memory", "Done", "Next: The memory view"]);
  await p.click(".tour-go-on");
  b = await p.settled();
  assert.deepEqual([b.step, b.count], ["vars", "1 of 7"]);
  assert.equal(await p.ev(`window.saturnus.store.state.layer`), true, "the memory view opened");
  for (let i = 0; i < 6; i++) await p.click(".tour-next");
  b = await p.settled();
  assert.deepEqual([b.step, b.count, b.next, b.goOn], ["keys-here", "7 of 7", "Done", null]);
  await p.click(".tour-next");
  assert.equal(await p.settled(), null, "Done ends it");
  assert.deepEqual(await p.ev(views), before, "the views as they were");
  assert.equal(await p.ev(`document.activeElement.id`), "palette-show", "the focus back where the tour began, across chapters");
  const sent = await p.ev(`window.__sent`);
  assert.deepEqual(sent.filter((c) => !READS.has(c)), [], `only reads: ${[...new Set(sent)]}`);

  // Skip tour ends it at once, with the same care.
  await p.ev(`window.saturnus.tour.start("memory")`);
  b = await p.settled();
  assert.equal(b.step, "vars");
  await p.click(".tour-skip");
  assert.equal(await p.settled(), null);
  assert.deepEqual(await p.ev(views), before);
});

test("a step whose element is gone is skipped and leaves the count, never shown floating", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  // Every ROM kept: no Download link in the list.
  await p.ev(`(() => { const s = window.saturnus.store; const r = s.state.roms; s.set({ roms: { ...r, slots: r.slots.map((x) => ({ ...x, fileName: "rom", state: "ready" })) } }); return true; })()`);
  await p.ev(`window.saturnus.tour.start("start")`);
  await p.settled();
  const steps = [];
  for (let b = await p.bubble(); b; b = await p.settled()) {
    steps.push(`${b.step} ${b.count}`);
    await p.ev(`window.saturnus.tour.step(1)`);
  }
  assert.deepEqual(steps, ["panel 1 of 7", "roms 2 of 7", "choose 3 of 6", "search 4 of 6", "shortcuts 5 of 6", "again 6 of 6"]);
});

test("on a phone the bubble is a sheet at the bottom, the ring on the element above it", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 390, height: 844, mobile: true });
  if (!p) return;
  for (const id of ["start", "calculator", "memory"]) {
    await p.ev(`window.saturnus.tour.start(${JSON.stringify(id)})`);
    for (let b = await p.settled(); b; b = await p.settled()) {
      const g = await p.ev(`(() => {
        const b = document.querySelector(".tour-bubble").getBoundingClientRect();
        const r = document.querySelector(".tour-ring").getBoundingClientRect();
        return { left: b.left, right: b.right, bottom: b.bottom, top: b.top, vw: document.documentElement.clientWidth, vh: innerHeight, ring: r.top, sheet: document.querySelector(".tour-bubble").classList.contains("sheet") };
      })()`);
      assert.ok(g.sheet, `${id}/${b.step}: a sheet`);
      assert.deepEqual([Math.round(g.left), Math.round(g.right), Math.round(g.bottom)], [0, g.vw, g.vh], `${id}/${b.step}: along the bottom, edge to edge`);
      assert.ok(g.ring < g.top, `${id}/${b.step}: the element shows above the sheet`);
      await p.ev(`window.saturnus.tour.step(1)`);
    }
  }
  assert.equal(await p.ev(`document.documentElement.scrollWidth <= document.documentElement.clientWidth`), true, "no horizontal overflow");
});

test("every step's element is seen: light and dark, 1280 and 390 px, the browser and the app", { timeout: 240_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const failures = [];
  let checked = 0;
  for (const host of ["browser", "app"]) {
    for (const [w, h, mobile] of [[1280, 860, false], [390, 844, true]]) {
      await p.size(w, h, mobile);
      await p.load();
      if (host === "app") {
        // The desktop app's ROM list: Download… buttons, the tour told it runs there.
        await p.ev(`(() => {
          window.saturnus.backend.romSource = "dialog";
          window.saturnus.tour.host = "app";
          const s = window.saturnus.store;
          const r = s.state.roms;
          s.set({ roms: { ...r, slots: r.slots.map((x) => ({ ...x })) } });
          return true;
        })()`);
      }
      for (const theme of ["light", "dark"]) {
        await p.ev(`document.documentElement.dataset.theme = ${JSON.stringify(theme)}, true`);
        for (const id of ["start", "calculator", "memory"]) {
          // The app differs only in the ROM steps.
          if (host === "app" && id !== "start") continue;
          const steps = await p.ev(`window.saturnus.tour.probe(${JSON.stringify(id)})`);
          assert.ok(steps.length >= 5, `${id}: ${steps.length} steps`);
          for (const s of steps) {
            checked++;
            if (!s.seen) failures.push(`${host} ${w}px ${theme} ${id}/${s.id} (${s.anchor})`);
          }
        }
      }
    }
  }
  assert.deepEqual(failures, [], `${failures.length} of ${checked} anchors not seen`);
});
