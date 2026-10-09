// The memory view's actions per subject (web/actions.js) and where a
// menu goes (components/menu.js `placeMenu`), without a browser.
import { test } from "node:test";
import assert from "node:assert/strict";
import { contextItems, subjectActions } from "../actions.js";
import { placeMenu } from "../components/menu.js";

const ctx = (extra = {}) => ({ writes: true, off: false, editor: true, read: "text", copy: true, hints: { rename: "F2", purge: "Del", copy: "⌘C", edit: "⌘E" }, ...extra });
/** The menu as ids, "-" for a rule, "note" for a line of text. */
const ids = (menu) => menu.map((x) => (x === "-" ? "-" : x.note ? "note" : x.id));
const off = (menu) => menu.filter((x) => x !== "-" && !x.note && x.disabled).map((x) => x.id);

test("an object: Edit, and a menu of Copy text, Save as file, Copy to, Move to, Rename and Purge last", () => {
  const a = subjectActions({ kind: "object", name: "P" }, ctx());
  assert.equal(a.primary.id, "edit");
  assert.equal(a.primary.disabled, false);
  assert.deepEqual(ids(a.menu), ["copy", "save", "-", "copyto", "moveto", "rename", "-", "purge"]);
  assert.deepEqual(a.inline, []);
  const purge = a.menu.at(-1);
  assert.equal(purge.danger, true);
  assert.equal(purge.hint, "Del");
  assert.equal(a.menu.find((x) => x.id === "rename").hint, "F2");
  assert.deepEqual(a.menu.map((x) => x.text ?? x), ["Copy text", "Save as file…", "-", "Copy to…", "Move to…", "Rename…", "-", "Purge…"]);
});

test("an object's Edit is off, with the reason, until it can be edited", () => {
  for (const [read, why] of [["reading", /Still reading/], ["failed", /Could not read/], ["textless", /no text form/]]) {
    const a = subjectActions({ kind: "object", name: "G" }, ctx({ read, copy: false }));
    assert.equal(a.primary.id, "edit");
    assert.equal(a.primary.disabled, true, read);
    assert.equal(a.primary.blocked, true, read);
    assert.match(a.primary.title, why);
    assert.deepEqual(ids(a.menu), ["save", "-", "copyto", "moveto", "rename", "-", "purge"], "no Copy text without a text form");
  }
});

test("a graphic: Copy image and Save as image in their own group, Edit off with its reason", () => {
  const a = subjectActions({ kind: "object", name: "PIC" }, ctx({ read: "textless", copy: false, image: true }));
  assert.equal(a.primary.disabled, true);
  assert.equal(a.primary.title, "A graphic has no text form to edit");
  assert.deepEqual(ids(a.menu), ["save", "-", "copy-image", "save-image", "-", "copyto", "moveto", "rename", "-", "purge"]);
  assert.deepEqual(off(a.menu), [], "the image items need no writes");
  const level = subjectActions({ kind: "level", name: "Level 1" }, ctx({ read: "textless", copy: false, image: true, writes: false }));
  assert.deepEqual(ids(level.menu), ["copy-image", "save-image"]);
  assert.deepEqual(level.menu.map((x) => x.text), ["Copy image", "Save as image…"]);
  const busy = subjectActions({ kind: "level", name: "Level 1" }, ctx({ read: "textless", copy: false, image: true, off: true }));
  assert.deepEqual(off(busy.menu), [], "nor wait for a write to end");
});

test("a directory in the list: Open, and Make current with the directory's writes in the menu", () => {
  const a = subjectActions({ kind: "dir", name: "DATA" }, ctx());
  assert.equal(a.primary.id, "open");
  assert.deepEqual(ids(a.menu), ["cd", "-", "store", "mkdir", "-", "copyto", "moveto", "rename", "-", "purge"]);
  // The calculator's current directory: no Make current.
  const here = subjectActions({ kind: "dir", name: "DATA", current: true }, ctx());
  assert.deepEqual(ids(here.menu), ["store", "mkdir", "-", "copyto", "moveto", "rename", "-", "purge"]);
});

test("the directory shown: Make current, the same set but Open", () => {
  const a = subjectActions({ kind: "shown", name: "DATA" }, ctx());
  assert.equal(a.primary.id, "cd");
  assert.deepEqual(ids(a.menu), ["store", "mkdir", "-", "copyto", "moveto", "rename", "-", "purge"]);
  assert.deepEqual(off(a.menu), []);
  const list = subjectActions({ kind: "dir", name: "DATA" }, ctx());
  const set = (x) => new Set(contextItems(x).map((i) => i.id ?? i));
  assert.deepEqual(set(a), set({ ...list, primary: null }), "the same actions but Open, done already");
  const here = subjectActions({ kind: "shown", name: "DATA", current: true }, ctx());
  assert.equal(here.primary, null, "already current: no primary");
});

test("HOME: Make current, no Rename or Purge, and a line that says so", () => {
  const a = subjectActions({ kind: "home", name: "HOME" }, ctx());
  assert.equal(a.primary.id, "cd");
  assert.deepEqual(ids(a.menu), ["store", "mkdir", "note"]);
  assert.equal(a.menu.at(-1).note, "HOME cannot be renamed or purged.");
  assert.deepEqual(contextItems(a).filter((x) => x.id).map((x) => x.id), ["cd", "store", "mkdir"]);
});

test("without writes: write actions left out, and one action left is drawn inline", () => {
  const w = ctx({ writes: false, off: true });
  const obj = subjectActions({ kind: "object", name: "P" }, w);
  assert.equal(obj.primary.id, "edit");
  assert.equal(obj.primary.disabled, true);
  assert.deepEqual(obj.menu, []);
  assert.deepEqual(obj.inline.map((x) => x.id), ["copy"]);
  const dir = subjectActions({ kind: "dir", name: "DATA" }, w);
  assert.equal(dir.primary.id, "open");
  assert.deepEqual([dir.menu, dir.inline], [[], []]);
  for (const kind of ["home", "shown"]) {
    const d = subjectActions({ kind, name: "X" }, w);
    assert.deepEqual([d.primary, d.menu, d.inline], [null, [], []], `${kind}: a head and no buttons`);
  }
  // No editor either: only Copy text, inline.
  const bare = subjectActions({ kind: "object", name: "P" }, { ...w, editor: false });
  assert.deepEqual([bare.primary, bare.inline.map((x) => x.id)], [null, ["copy"]]);
});

test("a stack level: Edit and Copy text side by side, no menu", () => {
  const a = subjectActions({ kind: "level", name: "Level 1" }, ctx());
  assert.equal(a.primary.id, "edit");
  assert.deepEqual(a.inline.map((x) => x.id), ["copy"]);
  assert.deepEqual(a.menu, []);
});

test("while a write runs: the writes stay in place, off; Copy text and Open stay on", () => {
  const a = subjectActions({ kind: "object", name: "P" }, ctx({ off: true }));
  assert.equal(a.primary.disabled, true);
  assert.equal(a.primary.blocked, false, "off for now, not for good");
  assert.deepEqual(off(a.menu), ["save", "copyto", "moveto", "rename", "purge"]);
  const d = subjectActions({ kind: "dir", name: "DATA" }, ctx({ off: true }));
  assert.equal(d.primary.disabled, false);
  assert.deepEqual(off(d.menu), ["cd", "store", "mkdir", "copyto", "moveto", "rename", "purge"]);
});

test("a menu goes under its button, right-aligned, and above it without room below", () => {
  const view = { width: 400, height: 800 };
  const size = { width: 220, height: 200 };
  const button = { left: 340, right: 380, top: 300, bottom: 336 };
  assert.deepEqual(placeMenu(button, size, view), { x: 160, y: 340, flipped: false });
  assert.deepEqual(placeMenu({ ...button, top: 700, bottom: 736 }, size, view), { x: 160, y: 496, flipped: true });
  // Left-aligned (the bar's New), kept off the right edge.
  assert.deepEqual(placeMenu({ left: 300, right: 360, top: 10, bottom: 46 }, size, view, { align: "start" }), { x: 172, y: 50, flipped: false });
  // Never past the left gutter.
  assert.equal(placeMenu({ left: 10, right: 50, top: 10, bottom: 46 }, size, view).x, 8);
  // Taller than either side: the larger side, kept inside.
  const tall = placeMenu({ left: 10, right: 50, top: 380, bottom: 416 }, { width: 220, height: 600 }, view);
  assert.ok(tall.y >= 8 && tall.y + 600 <= 792, JSON.stringify(tall));
});

test("a context menu opens at the pointer, to its left or above it at the edges", () => {
  const view = { width: 400, height: 800 };
  const size = { width: 220, height: 200 };
  const at = (x, y) => ({ left: x, right: x, top: y, bottom: y });
  assert.deepEqual(placeMenu(at(100, 100), size, view), { x: 100, y: 100, flipped: false });
  assert.deepEqual(placeMenu(at(300, 100), size, view), { x: 80, y: 100, flipped: false });
  assert.deepEqual(placeMenu(at(100, 700), size, view), { x: 100, y: 496, flipped: true });
});
