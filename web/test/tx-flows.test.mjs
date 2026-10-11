// The transfer page's pure modules (web/tx/flows.js, trace.js,
// kept-roms.js, kermit-device.js with a fake client, and
// web/shared/device.js): the words and decisions, the trace format, the
// kept-ROM rows, the device's queue and listings. No browser, no wasm.

import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";
import { HOME, checksumCell, directoriesOf, parentPath, pathKey, pathTitle, samePath, sizeCell, sortEntries } from "../shared/device.js";
import {
  AUTO_PREVIEW_BYTES, MODELS, START_SERVER, bytesText, disconnectNote, failureText, linkSettings, placeTitle, previewPlan,
  progressText, putPlan, replaceQuestion, searchText, settingsText, stringQuestion,
} from "../tx/flows.js";
import { SERIAL_MODELS, slotsOf } from "../tx/kept-roms.js";
import { problemText } from "../tx/transfers.js";
import { EmulatedCalculator, emulatedRefusal } from "../tx/emulator.js";
import { KermitDevice } from "../tx/kermit-device.js";
import { EmulatorLink, FakeLink, PORT_IN_USE, WebSerialLink } from "../tx/links.js";
import { ProtocolClient } from "../tx/protocol.js";
import { escapeBytes, formatTrace, parseTrace, unescapeBytes } from "../tx/trace.js";

const TRACES = new URL("../../crates/saturnus-tx/traces/", import.meta.url);

test("the words: sizes, progress, settings, the search, places", () => {
  assert.equal(bytesText(0), "0 bytes");
  assert.equal(bytesText(1), "1 byte");
  assert.equal(bytesText(999), "999 bytes");
  assert.equal(bytesText(1229), "1.2 KB");
  assert.equal(bytesText(30 * 1024), "30 KB");
  assert.equal(progressText({ label: "Getting PRG…" }), "Getting PRG…");
  assert.equal(progressText({ label: "Getting PRG…", done: 1229, total: 3482 }), "Getting PRG… 1.2 KB of 3.4 KB");
  assert.equal(progressText({ label: "Getting PRG…", done: 5000, total: 3482 }), "Getting PRG… 3.4 KB of 3.4 KB", "never more than all");
  assert.equal(progressText({ label: "Getting X…", done: 120 }), "Getting X… 120 bytes");
  assert.equal(settingsText({ baud: 9600, parity: "none" }), "9600 baud");
  assert.equal(settingsText({ baud: 2400, parity: "odd" }), "2400 baud, odd parity");
  assert.equal(searchText({ type: "listening", settings: { baud: 4800, parity: "none" } }), "Listening at 4800 baud…");
  assert.equal(searchText({ type: "found", settings: { baud: 2400, parity: "even" } }), "The calculator talks at 2400 baud, even parity.");
  assert.equal(placeTitle({ model: "48gx", linkLabel: "USB serial port" }), "HP 48G/GX on USB serial port");
  assert.equal(placeTitle({ model: "49g", emulated: true }), "Emulated 49G");
  assert.equal(placeTitle({ model: null }), "Calculator on serial port");
});

test("no protocol words in what the main flows say", () => {
  const said = [
    ...Object.values(START_SERVER).flatMap((s) => [s.title, ...s.steps, s.keysNote]),
    stringQuestion("a.md", "A").body, replaceQuestion("A").body, disconnectNote("49g"),
    ...["noAnswer", "nameClash", "isDirectory", "badName", "notAnObject", "cancelled", "busy", "notConnected"].map((kind) => failureText({ kind, varName: "A" })),
  ].join(" ");
  assert.doesNotMatch(said, /kermit|packet|iopar|xmodem|baud rate/i);
});

test("the connect dialog has every model, the 49G's keys quickly", () => {
  assert.deepEqual(MODELS, ["48sx", "48gx", "49g"]);
  for (const m of MODELS) assert.ok(START_SERVER[m].steps.length && START_SERVER[m].keys.length, m);
  assert.match(START_SERVER["48sx"].steps[0], /SERVER/);
  assert.match(START_SERVER["49g"].steps[0], /quickly/);
  assert.match(START_SERVER["49g"].keysNote, /pause/);
});

test("the stored connection settings, and the defaults for anything else", () => {
  assert.deepEqual(linkSettings(null), { find: true, baud: 9600, parity: "none" });
  assert.deepEqual(linkSettings("not json"), { find: true, baud: 9600, parity: "none" });
  assert.deepEqual(linkSettings('{"find":false,"baud":2400,"parity":"odd"}'), { find: false, baud: 2400, parity: "odd" });
  assert.deepEqual(linkSettings('{"baud":1234,"parity":"mark"}'), { find: true, baud: 9600, parity: "none" });
});

test("a put: binary and text as they are, anything else asks to be a string", () => {
  assert.deepEqual(putPlan("binary"), { ask: null, asString: false, how: "an exact copy" });
  assert.equal(putPlan("text").ask, null);
  assert.deepEqual(putPlan("other"), { ask: "string", asString: true, how: "a string" });
  const q = stringQuestion("notes.md", "NOTES");
  assert.equal(q.title, "Store it as a string?");
  assert.equal(q.danger, false);
  const r = replaceQuestion("PRG");
  assert.equal(r.title, "Replace PRG?");
  assert.equal(r.action, "Replace");
  assert.equal(r.danger, true);
});

test("a preview: programs as text, data as the file, nothing for libraries; large ones on request", () => {
  assert.deepEqual(previewPlan({ kind: "Program", size: 25 }), { fetch: "text", auto: true });
  assert.deepEqual(previewPlan({ kind: "Algebraic", size: 30 }), { fetch: "text", auto: true });
  assert.deepEqual(previewPlan({ kind: "Graphic", size: 1103.5 }), { fetch: "binary", auto: true });
  assert.deepEqual(previewPlan({ kind: "String", size: AUTO_PREVIEW_BYTES + 1 }), { fetch: "binary", auto: false });
  assert.deepEqual(previewPlan({ kind: "Library", size: 10 }), { fetch: null, auto: false });
  assert.deepEqual(previewPlan({ kind: "Directory", size: 10, directory: true }), { fetch: null, auto: false });
  assert.deepEqual(previewPlan(null), { fetch: null, auto: false });
});

test("failures read as sentences; the 49G's note after FINISH", () => {
  assert.match(failureText({ kind: "noAnswer" }), /server mode/);
  assert.equal(failureText({ kind: "nameClash", varName: "X" }), "X is already there.");
  assert.equal(failureText(new Error("the calculator said: No Room")), "The calculator said: No Room");
  // A request the wasm package does not know: the page's files are from two builds.
  assert.equal(failureText(new Error('no request "pict"')), "This page's files are from different versions. Reload the page.");
  assert.equal(problemText(new Error('no request "pict"')), "This page's files are from different versions. Reload the page.");
  assert.equal(disconnectNote("48gx"), "The calculator has left server mode.");
  assert.match(disconnectNote("49g"), /^The calculator has left server mode\. .*algebraic mode.*one list/);
});

test("the emulated calculator takes the models with a serial port", () => {
  assert.deepEqual(SERIAL_MODELS, ["48sx", "48gx", "49g"]);
  for (const m of SERIAL_MODELS) assert.equal(emulatedRefusal(m), null);
  assert.match(emulatedRefusal("42s"), /no serial port/);
  assert.match(emulatedRefusal("39g"), /aplets/);
});

test("the kept ROMs: the serial models only, in order, and nothing from a store it does not know", () => {
  const rows = slotsOf(["@settings", "49g", "42s", "48sx"], [{ lastModel: "49g" }, { name: "rom.49g", sha256: "b", revision: "1.19-6" }, { name: "42s.rom", sha256: "c" }, { name: "sxrom-j", sha256: "a" }]);
  assert.deepEqual(rows, [{ model: "48sx", name: "sxrom-j", sha256: "a" }, { model: "49g", name: "rom.49g", sha256: "b" }]);
  assert.deepEqual(slotsOf(["48gx"], [{ something: "else" }]), []);
  assert.deepEqual(slotsOf(["48gx"], [null]), []);
  assert.deepEqual(slotsOf(["48gx"], [{ sha256: "d" }]), [{ model: "48gx", name: "48gx", sha256: "d" }]);
});

test("paths, sizes, checksums and the list's order: directories first", () => {
  assert.equal(pathKey(["HOME", "D1"]), "HOME/D1");
  assert.ok(samePath(["HOME"], [...HOME]));
  assert.ok(!samePath(["HOME"], ["HOME", "A"]));
  assert.deepEqual(parentPath(["HOME", "A", "B"]), ["HOME", "A"]);
  assert.deepEqual(parentPath(["HOME"]), ["HOME"]);
  assert.equal(pathTitle(["HOME", "A"]), "HOME › A");
  assert.equal(sizeCell(29.5), "29.5");
  assert.equal(sizeCell(1204), "1,204");
  assert.equal(checksumCell(0x229d), "# 229Dh");
  const e = (name, kind, size, directory = false) => ({ name, kind, size, checksum: 0, directory });
  const entries = [e("b", "Real Number", 10), e("A", "Program", 30), e("Z", "Directory", 5, true), e("C", "Directory", 9, true)];
  assert.deepEqual(sortEntries(entries).map((x) => x.name), ["C", "Z", "A", "b"]);
  assert.deepEqual(sortEntries(entries, "name", false).map((x) => x.name), ["Z", "C", "b", "A"]);
  assert.deepEqual(sortEntries(entries, "size").map((x) => x.name), ["Z", "C", "b", "A"]);
  assert.deepEqual(directoriesOf({ entries }).map((x) => x.name), ["C", "Z"]);
});

test("the trace format reads saturnus-tx's traces and writes them back the same", () => {
  const names = readdirSync(TRACES).filter((f) => f.endsWith(".trace"));
  assert.ok(names.length >= 3);
  for (const name of names) {
    const text = readFileSync(new URL(name, TRACES), "utf8");
    const steps = parseTrace(text);
    assert.ok(steps.some((s) => s.kind === "open") && steps.some((s) => s.kind === "write") && steps.some((s) => s.kind === "read"), name);
    const again = parseTrace(formatTrace(steps));
    assert.deepEqual(again, steps, name);
  }
  const bytes = new Uint8Array([1, 0x5c, 13, 10, 0xcd, 65]);
  assert.equal(escapeBytes(bytes), "\\x01\\\\\\r\\n\\xcdA");
  assert.deepEqual(unescapeBytes(escapeBytes(bytes)), bytes);
  assert.throws(() => unescapeBytes("\\q"));
  assert.throws(() => parseTrace("nonsense"));
});

test("a FakeLink plays the trace's reads after each write, and says where a write differs", async () => {
  const link = new FakeLink("# hello\n0 = 9600 none\n0 > ab\n5 < cd\n10 < ef\n20 > gh\n", { speed: 10 });
  const got = [];
  link.addEventListener("data", (e) => got.push(new TextDecoder().decode(e.detail)));
  await link.open({ baud: 9600, parity: "none" });
  await link.write(new TextEncoder().encode("ab"));
  await new Promise((r) => setTimeout(r, 30));
  assert.deepEqual(got, ["cd", "ef"]);
  const lost = [];
  link.addEventListener("lost", (e) => lost.push(e.detail));
  await assert.rejects(link.write(new TextEncoder().encode("xx")), /step 4 \(write at 20 ms\): wrote "xx", the trace "gh"/);
  assert.equal(lost.length, 1);
  assert.ok(link.failure);
});

/** A ProtocolClient stand-in: answers each request after `ms` from `answers(request)`. */
function fakeClient(answers, ms = 5) {
  const c = new EventTarget();
  c.connection = null;
  c.requests = [];
  c.request = (req, data) => {
    c.requests.push({ ...req, data });
    return new Promise((resolve, reject) => setTimeout(() => {
      try {
        const out = answers(req, data);
        if (req.type === "connect") c.connection = out.connection;
        resolve(out);
      } catch (err) {
        reject(err);
      }
    }, ms));
  };
  c.close = async () => {};
  return c;
}

test("the device: one operation at a time, listings kept until a change, the queue cancelled", async () => {
  const listing = { type: "listing", path: ["HOME"], free: 1000, entries: [] };
  const client = fakeClient((req) => {
    if (req.type === "connect") return { type: "connected", connection: { model: "48gx", settings: { baud: 9600, parity: "none" } } };
    if (req.type === "list") return { ...listing, path: req.dir };
    if (req.type === "put") return { type: "stored", name: req.name };
    if (req.type === "get") return { type: "file", name: req.name, fileName: `${req.name}.hp`, format: req.format, data: new Uint8Array([1]) };
    return { type: "disconnected" };
  });
  const d = new KermitDevice(client);
  const busy = [];
  const changes = [];
  d.addEventListener("busy", (e) => busy.push(e.detail));
  d.addEventListener("change", (e) => changes.push(e.detail.path));
  assert.equal((await d.connect()).model, "48gx");
  const [a, b] = await Promise.all([d.list(["HOME"]), d.list(["HOME"])]);
  assert.equal(a, b);
  assert.equal(client.requests.filter((r) => r.type === "list").length, 1, "one listing however many ask");
  // Queued behind each other, never two at once.
  const p1 = d.read(["HOME"], "A");
  const p2 = d.read(["HOME"], "B", { text: true });
  assert.equal(d.queued, 1);
  assert.deepEqual((await p1).fileName, "A.hp");
  assert.equal((await p2).format, "text");
  // A put reads the directory again.
  assert.equal(await d.write(["HOME"], "C", new Uint8Array([1, 2])), "C");
  assert.deepEqual(changes, [["HOME"]]);
  await d.list(["HOME"]);
  assert.equal(client.requests.filter((r) => r.type === "list").length, 2);
  assert.deepEqual(client.requests.find((r) => r.type === "put").data, new Uint8Array([1, 2]));
  // Operations not started yet can be dropped.
  const running = d.read(["HOME"], "X");
  const waiting = [d.read(["HOME"], "Y"), d.read(["HOME"], "Z")];
  assert.equal(d.cancelQueued(), 2);
  for (const w of waiting) await assert.rejects(w, (err) => err.kind === "cancelled");
  await running;
  assert.ok(busy.some((x) => x.label === "Getting X…") && busy.at(-1).label === null);
  await d.disconnect();
  assert.equal(client.requests.at(-1).type, "disconnect");
});

test("the device: a listing that failed is asked for again", async () => {
  let n = 0;
  const client = fakeClient((req) => {
    if (req.type === "list" && n++ === 0) throw Object.assign(new Error("timeout"), { kind: "calculator" });
    return { type: "listing", path: req.dir, entries: [] };
  });
  const d = new KermitDevice(client);
  await assert.rejects(d.list(["HOME"]), /timeout/);
  assert.deepEqual((await d.list(["HOME"])).entries, []);
});

// ---------------------------------------------------------------- the review's cases (PR 130)

test("the device drops only the waiting operations of a tag", async () => {
  const client = fakeClient((req) => ({ type: "file", name: req.name, fileName: req.name, format: "binary", data: new Uint8Array() }), 20);
  const d = new KermitDevice(client);
  const running = d.read(["HOME"], "A", { tag: "preview" });
  const preview = d.read(["HOME"], "B", { tag: "preview" });
  const get = d.read(["HOME"], "C");
  assert.equal(d.cancelQueued("preview"), 1);
  await assert.rejects(preview, (err) => err.kind === "cancelled");
  assert.equal((await running).name, "A");
  assert.equal((await get).name, "C", "an untagged one stays");
  assert.deepEqual(client.requests.map((r) => r.name), ["A", "C"]);
});

test("a closed link reads as not connected, not as \"Closed\"", () => {
  assert.equal(failureText({ kind: "closed", message: "closed" }), "Not connected to a calculator.");
});

test("a port held elsewhere says so in words", async () => {
  for (const name of ["NetworkError", "InvalidStateError"]) {
    const port = { open: async () => { throw Object.assign(new Error("Failed to open serial port."), { name }); }, close: async () => {} };
    await assert.rejects(new WebSerialLink(port, { serial: null }).open({ baud: 9600 }), (err) => err.message === PORT_IN_USE);
  }
  const other = { open: async () => { throw Object.assign(new Error("odd"), { name: "TypeError" }); }, close: async () => {} };
  await assert.rejects(new WebSerialLink(other, { serial: null }).open({ baud: 9600 }), /odd/);
});

test("an emulated calculator that does not take a write loses the link", async () => {
  const emu = Object.assign(new EventTarget(), { request: () => new Promise(() => {}) });
  const link = new EmulatorLink(emu, { writeTimeoutMs: 30 });
  const lost = [];
  link.addEventListener("lost", (e) => lost.push(e.detail));
  await assert.rejects(link.write(new Uint8Array([1])), /does not answer/);
  assert.deepEqual(lost, ["The emulated calculator does not answer."]);
});

test("a failed emulator core is the calculator gone: its requests fail", async () => {
  const worker = { postMessage() {}, terminate() {} };
  const emu = new EmulatedCalculator({ worker: () => worker });
  const gone = [];
  emu.addEventListener("gone", (e) => gone.push(e.detail));
  const waiting = emu.request("stack");
  worker.onmessage({ data: { type: "fatal", message: "the emulated calculator failed: unreachable" } });
  await assert.rejects(waiting, /unreachable/);
  assert.deepEqual(gone, ["the emulated calculator failed: unreachable"]);
});

test("a protocol Worker that fails is a lost link; a closed client rejects as closed", async () => {
  const make = () => {
    const w = { posted: [], postMessage(m) { w.posted.push(m); }, terminate() {} };
    return w;
  };
  let w;
  const link = Object.assign(new EventTarget(), { open: async () => {}, write: async () => {}, close: async () => {} });
  const client = new ProtocolClient(link, { worker: () => (w = make()) });
  const lost = [];
  client.addEventListener("lost", (e) => lost.push(e.detail));
  w.onmessage({ data: { type: "ready" } });
  const req = client.request({ type: "list", dir: ["HOME"] });
  await new Promise((r) => setTimeout(r, 0));
  w.onmessage({ data: { type: "failed", message: "unreachable" } });
  await assert.rejects(req, /unreachable/);
  assert.deepEqual(lost, ["the transfer part of the page stopped (unreachable)"]);
  await assert.rejects(client.request({ type: "list" }), (err) => err.kind === "closed");
  const w2 = make();
  const c2 = new ProtocolClient(link, { worker: () => w2 });
  w2.onmessage({ data: { type: "ready" } });
  const pending = c2.request({ type: "list" });
  await new Promise((r) => setTimeout(r, 0));
  await c2.close();
  await assert.rejects(pending, (err) => err.kind === "closed");
});
