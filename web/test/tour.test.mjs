// The tour's steps and its run (web/tour.js): every step well formed,
// which steps and chapters apply where, the words with their keys, and
// a run that drops a step it cannot show, counts what is left, stays on
// Back from the first and ends past the last. The page's side is
// web/test/tour-page.test.mjs. `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { CHAPTERS, SETUPS, TourRun, animationsDone, applies, chapter, chaptersFor, context, fillText, nextChapter, offerShown } from "../tour.js";

const WHEN = { host: ["browser", "app"], pointer: ["fine", "coarse"], phone: [true, false], mac: [true, false] };

/** Every place the tour can run: host × pointer × phone × Mac. */
function places(booted = "48gx") {
  const out = [];
  for (const host of WHEN.host) for (const pointer of WHEN.pointer) for (const phone of WHEN.phone) for (const mac of WHEN.mac) {
    out.push(context({ host, pointer, phone, mac, booted, keys: (id) => ({ palette: "⌘K", edit: "⌘E", layerFocus: "⌥M", fullscreen: "⌥↩" })[id] ?? "" }));
  }
  return out;
}

test("every step is well formed: an anchor, a title, short words, a known setup and condition", () => {
  const ids = new Set();
  for (const c of CHAPTERS) {
    for (const s of c.steps) {
      const key = `${c.id}/${s.id}`;
      assert.ok(!ids.has(key), `${key} is unique`);
      ids.add(key);
      assert.equal(typeof s.anchor, "string", key);
      assert.ok(s.anchor.length, key);
      assert.ok(s.title && s.text, key);
      assert.ok(SETUPS.includes(s.setup), `${key}: setup ${s.setup}`);
      for (const [k, v] of Object.entries(s.when ?? {})) assert.ok(WHEN[k]?.includes(v), `${key}: when ${k}=${v}`);
      // One or two plain sentences, no exclamation marks.
      const sentences = s.text.split(/(?<=[.?])\s+(?=[A-Z"“])/).filter(Boolean);
      assert.ok(sentences.length >= 1 && sentences.length <= 2, `${key}: ${sentences.length} sentences`);
      assert.ok(!s.text.includes("!"), `${key}: no exclamation mark`);
      assert.ok(s.text.endsWith("."), `${key}: ends with a full stop`);
    }
  }
});

test("each chapter has 5 to 8 steps wherever it runs", () => {
  for (const ctx of places()) {
    for (const c of chaptersFor(ctx)) {
      const n = new TourRun(c, ctx).total;
      assert.ok(n >= 5 && n <= 8, `${c.id} at ${ctx.host}/${ctx.pointer}/${ctx.phone ? "phone" : "wide"}: ${n} steps`);
    }
  }
});

test("a step applies when every condition of its `when` holds", () => {
  const ctx = context({ host: "app", pointer: "coarse", phone: true });
  assert.ok(applies({ when: {} }, ctx));
  assert.ok(applies({}, ctx));
  assert.ok(applies({ when: { host: "app", phone: true } }, ctx));
  assert.ok(!applies({ when: { host: "app", phone: false } }, ctx));
  assert.ok(!applies({ when: { pointer: "fine" } }, ctx));
  // The host's own Download: the link in the browser, the button in the app.
  const start = chapter("start");
  const ids = (c) => new TourRun(start, c).steps.map((s) => s.id);
  assert.ok(ids(context({ host: "browser" })).includes("download"));
  assert.ok(!ids(context({ host: "browser" })).includes("download-app"));
  assert.ok(ids(context({ host: "app" })).includes("download-app"));
  assert.ok(!ids(context({ host: "app" })).includes("download"));
  // No keys to change by touch.
  assert.ok(!ids(context({ pointer: "coarse", phone: true })).includes("shortcuts"));
});

test("the memory view's chapter only while a 48SX, 48GX or 49G runs; the next chapter", () => {
  const titles = (booted) => chaptersFor(context({ booted })).map((c) => c.id);
  assert.deepEqual(titles(null), ["start", "calculator"]);
  assert.deepEqual(titles("38g"), ["start", "calculator"]);
  assert.deepEqual(titles("42s"), ["start", "calculator"]);
  for (const m of ["48sx", "48gx", "49g"]) assert.deepEqual(titles(m), ["start", "calculator", "memory"]);
  assert.equal(nextChapter("start", context()).id, "calculator");
  assert.equal(nextChapter("calculator", context()), null, "no memory view without a model that has one");
  assert.equal(nextChapter("calculator", context({ booted: "48sx" })).id, "memory");
  assert.equal(nextChapter("memory", context({ booted: "48sx" })), null);
});

test("the words: a binding's key in parentheses, none by touch or without one; ⌘ or Ctrl, Option or Alt", () => {
  const keys = (id) => (id === "palette" ? "⌘K" : "");
  assert.equal(fillText("Search{key:palette} finds.", context({ keys })), "Search (⌘K) finds.");
  assert.equal(fillText("Search{key:palette} finds.", context({ keys, pointer: "coarse" })), "Search finds.");
  assert.equal(fillText("Edit{key:edit} opens.", context({ keys })), "Edit opens.");
  assert.equal(fillText("Paste ({mod}V), {alt}+click.", context({ mac: true })), "Paste (⌘V), Option+click.");
  assert.equal(fillText("Paste ({mod}V), {alt}+click.", context({ mac: false })), "Paste (Ctrl+V), Alt+click.");
  // Every step's words come out without a placeholder left.
  for (const ctx of places()) for (const c of CHAPTERS) for (const s of c.steps) assert.ok(!/[{}]/.test(fillText(s.text, ctx)), `${c.id}/${s.id}`);
});

const fake = (id, n) => ({ id, steps: Array.from({ length: n }, (_, i) => ({ id: `s${i}`, anchor: `#s${i}`, setup: "panel" })) });

test("a run: forward, back, the count, done past the last", async () => {
  const run = new TourRun(fake("x", 3), context());
  const shown = [];
  const show = async (s) => (shown.push(s.id), true);
  assert.equal((await run.go(1, show)).id, "s0");
  assert.equal(run.counter, "1 of 3");
  assert.ok(run.first);
  assert.equal((await run.go(1, show)).id, "s1");
  assert.equal((await run.go(-1, show)).id, "s0");
  assert.equal((await run.go(1, show)).id, "s1");
  assert.equal((await run.go(1, show)).id, "s2");
  assert.ok(run.last);
  assert.equal(run.counter, "3 of 3");
  assert.equal(await run.go(1, show), null, "past the last: done");
  assert.deepEqual(shown, ["s0", "s1", "s0", "s1", "s2"]);
});

test("a step whose anchor is not shown is dropped and leaves the count", async () => {
  const run = new TourRun(fake("x", 5), context());
  const missing = new Set(["s1", "s3"]);
  const show = async (s) => !missing.has(s.id);
  assert.equal((await run.go(1, show)).id, "s0");
  assert.equal(run.counter, "1 of 5");
  assert.equal((await run.go(1, show)).id, "s2");
  assert.equal(run.counter, "2 of 4");
  assert.equal((await run.go(1, show)).id, "s4");
  assert.equal(run.counter, "3 of 3");
  assert.ok(run.last);
  assert.deepEqual(run.skipped, ["s1", "s3"]);
  // Back past the dropped ones.
  assert.equal((await run.go(-1, show)).id, "s2");
  assert.equal(run.counter, "2 of 3");
});

test("Back from the first step stays on it; a chapter with nothing shown ends at once", async () => {
  const run = new TourRun(fake("x", 3), context());
  const missing = new Set(["s0"]);
  const show = async (s) => !missing.has(s.id);
  assert.equal((await run.go(1, show)).id, "s1");
  assert.equal(run.counter, "1 of 2");
  assert.equal((await run.go(-1, show)).id, "s1", "stays");
  assert.equal(run.counter, "1 of 2");
  const none = new TourRun(fake("y", 3), context());
  assert.equal(await none.go(1, async () => false), null);
  assert.equal(none.total, 0);
});

test("the offer shows until it is dismissed or the tour taken", () => {
  assert.ok(offerShown(null));
  assert.ok(offerShown(undefined));
  assert.ok(!offerShown("dismissed"));
  assert.ok(!offerShown("taken"));
});

test("a step waits for the animations around its anchor to end, not for others or for endless ones", async () => {
  const node = (parent = null) => ({ parent, contains(x) { for (let n = x; n; n = n.parent) if (n === this) return true; return false; } });
  const sheet = node();
  const anchor = node(sheet);
  const elsewhere = node();
  const log = [];
  const animation = (target, { iterations = 1, ends = "finishes" } = {}) => {
    let settle;
    const finished = new Promise((resolve, reject) => { settle = ends === "finishes" ? () => resolve() : () => reject(new Error("cancelled")); });
    if (ends !== "never") setTimeout(() => { log.push(`${target === sheet ? "sheet" : target === anchor ? "anchor" : "elsewhere"} ${ends}`); settle(); }, 10);
    finished.catch(() => {});
    return { effect: { target, getTiming: () => ({ iterations }) }, finished, playState: "running", playbackRate: 1 };
  };
  const doc = { getAnimations: () => [
    animation(sheet),                                   // the sheet fading in: waited for
    animation(anchor, { ends: "cancels" }),             // on the anchor itself, cancelled: tolerated
    animation(elsewhere, { ends: "never" }),            // somewhere else: not waited for
    animation(sheet, { iterations: Infinity, ends: "never" }), // a spinner on the sheet: not waited for
    { effect: null, finished: new Promise(() => {}) },  // no effect: ignored
  ] };
  await animationsDone(anchor, doc);
  assert.deepEqual(log.sort(), ["anchor cancels", "sheet finishes"]);
  // Nothing animating: returns at once.
  await animationsDone(anchor, { getAnimations: () => [] });
});

test("a paused or stopped animation is not waited for, and a stuck one gives up after the cap", async () => {
  const sheet = { contains: () => true };
  const never = (extra) => ({ effect: { target: sheet, getTiming: () => ({ iterations: 1 }) }, finished: new Promise(() => {}), playState: "running", playbackRate: 1, ...extra });
  // Paused, or at rate 0: skipped, so this returns at once.
  const t0 = Date.now();
  await animationsDone({}, { getAnimations: () => [never({ playState: "paused" }), never({ playbackRate: 0 })] }, 5000);
  assert.ok(Date.now() - t0 < 500, "no wait for a paused animation");
  // Running but never finishing: the cap ends the wait.
  const t1 = Date.now();
  await animationsDone({}, { getAnimations: () => [never()] }, 50);
  const waited = Date.now() - t1;
  assert.ok(waited >= 45 && waited < 1000, `waited ${waited} ms, the cap`);
});
