// The transfer page's queue (web/tx/queue.js) and the panes' actions
// (web/tx/actions.js): one job per place at a time, each place's jobs in
// the order asked, the waiting ones cancellable, progress reported; what
// a row's menu, the pane's "⋯" and the keys offer on a calculator and on a
// folder. Pure, no browser.

import assert from "node:assert/strict";
import { test } from "node:test";
import { KEYS, keyAction, paneActions, rowActions } from "../tx/actions.js";
import { Cancelled, JobQueue } from "../tx/queue.js";

/** A job that ends when `release()` is called; its runs logged in `log`. */
function gate(log, name) {
  let release;
  const done = new Promise((r) => {
    release = r;
  });
  return { release: () => release(name), run: async ({ report }) => { log.push(`start ${name}`); report(5, 10); const v = await done; log.push(`end ${name}`); return v; } };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

test("one job per place at a time; another place's job runs beside it", async () => {
  const q = new JobQueue();
  const log = [];
  const a = gate(log, "a");
  const b = gate(log, "b");
  const c = gate(log, "c");
  const pa = q.add({ label: "A", places: ["calc"], run: a.run });
  const pb = q.add({ label: "B", places: ["calc"], run: b.run });
  const pc = q.add({ label: "C", places: ["folder"], run: c.run });
  await tick();
  assert.deepEqual(log, ["start a", "start c"]);
  assert.deepEqual(q.running.map((j) => j.label), ["A", "C"]);
  assert.deepEqual(q.waiting.map((j) => j.label), ["B"]);
  assert.equal(q.busy("calc"), true);
  assert.equal(q.uses("folder"), true);
  a.release();
  assert.equal(await pa, "a");
  await tick();
  assert.deepEqual(log, ["start a", "start c", "end a", "start b"]);
  b.release();
  c.release();
  assert.deepEqual(await Promise.all([pb, pc]), ["b", "c"]);
  await tick();
  assert.equal(q.jobs.length, 0);
  assert.equal(q.uses("calc"), false);
});

test("a job on two places waits for both, and holds both for the jobs asked after it", async () => {
  const q = new JobQueue();
  const log = [];
  const a = gate(log, "a");
  const copy = gate(log, "copy");
  const later = gate(log, "later");
  q.add({ label: "A", places: ["calc"], run: a.run });
  const pCopy = q.add({ label: "copy", places: ["calc", "folder"], run: copy.run });
  const pLater = q.add({ label: "later", places: ["folder"], run: later.run });
  await tick();
  // The folder is free, but the copy asked first for it waits for the calculator: the later job waits too.
  assert.deepEqual(log, ["start a"]);
  a.release();
  await tick();
  await tick();
  assert.deepEqual(log, ["start a", "end a", "start copy"]);
  copy.release();
  await pCopy;
  await tick();
  assert.ok(log.includes("start later"));
  later.release();
  await pLater;
});

test("waiting jobs cancel; a running one does not; cancelWaiting by place", async () => {
  const q = new JobQueue();
  const log = [];
  const a = gate(log, "a");
  const changes = [];
  q.addEventListener("change", () => changes.push(q.jobs.map((j) => `${j.label}:${j.state}`).join(",")));
  const pa = q.add({ label: "A", places: ["calc"], run: a.run });
  const pb = q.add({ label: "B", places: ["calc"], run: async () => "b" });
  const pc = q.add({ label: "C", places: ["calc"], run: async () => "c" });
  const pd = q.add({ label: "D", places: ["folder"], run: () => new Promise(() => {}) });
  await tick();
  assert.equal(q.cancel(pa.id), false, "a running job is not cancelled");
  assert.equal(q.cancel(pb.id), true);
  await assert.rejects(pb, (e) => e instanceof Cancelled && e.kind === "cancelled" && /B was cancelled before it started/.test(e.message));
  assert.equal(q.cancelWaiting("folder"), 0, "D runs already");
  assert.equal(q.cancelWaiting("calc"), 1);
  await assert.rejects(pc, Cancelled);
  a.release();
  await pa;
  assert.ok(changes.some((s) => s.includes("B:queued")), "the waiting job was shown");
  void pd;
});

test("progress: the running job reports its bytes; a failure rejects and frees the place", async () => {
  const q = new JobQueue();
  let seen = null;
  q.addEventListener("change", () => {
    const r = q.running[0];
    if (r?.done) seen = [r.done, r.total, r.note];
  });
  await q.add({ label: "P", places: ["x"], total: 100, run: async ({ report }) => { report(40, 100, "Putting PRG…"); await tick(); } });
  assert.deepEqual(seen, [40, 100, "Putting PRG…"]);
  await assert.rejects(q.add({ label: "F", places: ["x"], run: async () => { throw new Error("boom"); } }), /boom/);
  assert.equal(await q.add({ label: "G", places: ["x"], run: async () => 7 }), 7);
});

test("the keys: F5 copies, F2 renames, Delete removes; not with a modifier", () => {
  assert.equal(keyAction({ key: "F5" }), "copy");
  assert.equal(keyAction({ key: "F2" }), "rename");
  assert.equal(keyAction({ key: "Delete" }), "remove");
  assert.equal(keyAction({ key: "F5", ctrlKey: true }), null);
  assert.equal(keyAction({ key: "Backspace" }), null);
  assert.deepEqual(KEYS, { copy: "F5", rename: "F2", remove: "Del" });
});

const ids = (items) => items.filter((i) => i !== "-").map((i) => i.id);

test("a calculator's rows: copy, download, rename, purge; a directory opens and goes as one file", () => {
  const v = { name: "PRG", kind: "Program", directory: false };
  const d = { name: "GAMES", kind: "Directory", directory: true };
  assert.deepEqual(ids(rowActions({ kind: "device", entries: [v], other: "folder" })), ["copy", "download", "downloadText", "rename", "remove"]);
  const dir = rowActions({ kind: "device", entries: [d], other: "folder" });
  assert.deepEqual(ids(dir), ["open", "copy", "getOne", "rename", "remove"]);
  assert.equal(dir.find((i) => i.id === "getOne").text, "Copy as one file to the other pane");
  assert.equal(rowActions({ kind: "device", entries: [d], other: null }).find((i) => i.id === "getOne").text, "Download as one file");
  const two = rowActions({ kind: "device", entries: [v, d], other: "device" });
  assert.deepEqual(ids(two), ["copy", "remove"], "no rename for two");
  assert.equal(two.find((i) => i.id === "copy").text, "Copy 2 to the other pane");
  assert.equal(two.find((i) => i.id === "remove").text, "Purge 2…");
  const purge = rowActions({ kind: "device", entries: [v], other: "folder" }).find((i) => i.id === "remove");
  assert.deepEqual([purge.text, purge.key, purge.danger], ["Purge…", "Del", true]);
  // Nothing in the other pane: copy is there but off, and says why.
  const off = rowActions({ kind: "device", entries: [v], other: null }).find((i) => i.id === "copy");
  assert.equal(off.disabled, true);
  assert.match(off.title, /other pane first/);
  assert.deepEqual(rowActions({ kind: "device", entries: [], other: "folder" }), []);
});

test("a folder's rows: copy, download (a .zip for a folder), rename, delete; a .hp file restores onto a calculator", () => {
  const f = { name: "48GX-backup-2026-10-11-1432.hp", kind: "File", directory: false };
  const t = { name: "notes.txt", kind: "File", directory: false };
  const d = { name: "saved", kind: "Folder", directory: true };
  assert.deepEqual(ids(rowActions({ kind: "folder", entries: [f], other: "device" })), ["copy", "download", "restoreFrom", "rename", "remove"]);
  assert.deepEqual(ids(rowActions({ kind: "folder", entries: [f], other: "folder" })), ["copy", "download", "rename", "remove"]);
  assert.deepEqual(ids(rowActions({ kind: "folder", entries: [t], other: "device" })), ["copy", "download", "rename", "remove"]);
  const dir = rowActions({ kind: "folder", entries: [d], other: "device" });
  assert.deepEqual(ids(dir), ["open", "copy", "download", "rename", "remove"]);
  assert.equal(dir.find((i) => i.id === "download").text, "Download as .zip");
  assert.equal(dir.find((i) => i.id === "remove").text, "Delete…");
  assert.equal(rowActions({ kind: "folder", entries: [t, d], other: null }).find((i) => i.id === "remove").text, "Delete 2…");
});

test("the pane's own menu: a calculator's and a folder's", () => {
  const calc = paneActions({ kind: "device", connected: true, emulated: false, other: "folder" });
  assert.deepEqual(ids(calc), ["mkdir", "backup", "restore", "pict", "copyAll", "settings", "refresh", "close"]);
  assert.equal(calc.find((i) => i.id === "close").text, "Disconnect");
  assert.equal(calc.find((i) => i.id === "pict").text, "Graphics screen");
  assert.ok(!calc.find((i) => i.id === "copyAll").disabled);
  const noFolder = paneActions({ kind: "device", connected: true, other: "device" }).find((i) => i.id === "copyAll");
  assert.equal(noFolder.disabled, true);
  assert.match(noFolder.title, /folder in the other pane/);
  // A calculator in the right pane: the left one never shows a folder, so the hint says where one goes.
  const rightCalc = paneActions({ kind: "device", connected: true, other: "device", otherFolders: false }).find((i) => i.id === "copyAll");
  assert.equal(rightCalc.disabled, true);
  assert.match(rightCalc.title, /only in the right pane/);
  assert.equal(rowActions({ kind: "device", entries: [{ name: "X", directory: false }], other: null, otherFolders: false }).find((i) => i.id === "copy").title, "Show a calculator in the other pane first.");
  // After a restore: everything but the settings and the disconnect off.
  const restarted = paneActions({ kind: "device", connected: false, emulated: true, other: "folder" });
  assert.deepEqual(restarted.filter((i) => i !== "-" && !i.disabled).map((i) => i.id), ["settings", "close"]);
  assert.equal(restarted.find((i) => i.id === "close").text, "Turn the emulated calculator off");
  const folder = paneActions({ kind: "folder", picked: true });
  assert.deepEqual(ids(folder), ["mkdir", "upload", "zipAll", "refresh", "close"]);
  assert.equal(folder.find((i) => i.id === "close").text, "Close this folder");
  assert.equal(paneActions({ kind: "folder", picked: false }).find((i) => i.id === "close").text, "Close the files");
});
