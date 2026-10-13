// The cable console's model (web/tx/cable.js): the Kermit and XModem
// decoder over the recorded traces (the page's session, the fast
// transfers of the 48GX and 49G), bad checks, resent packets, parity, a
// packet cut off, the ring buffer, and the tapped link, which changes
// nothing for the protocol. No browser.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { CableDecoder, CableLog, bytesHex, bytesText, kermitCheck, lineText, unquote } from "../tx/cable.js";
import { FakeLink } from "../tx/links.js";
import { parseTrace } from "../tx/trace.js";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

/** Every line of a trace, as the decoder sees it on a tapped link. */
function decode(text, { expected = () => null } = {}) {
  let t = 0;
  const log = new CableLog({ now: () => t });
  const d = new CableDecoder({ source: "left", log, expected });
  for (const s of parseTrace(text)) {
    if (s.at !== undefined) t = s.at;
    if (s.kind === "open") d.open(s);
    else if (s.kind === "write") d.feed("out", s.bytes);
    else if (s.kind === "read") d.feed("in", s.bytes);
  }
  d.close();
  return log.since(0);
}

/** A Kermit packet as written on the wire. */
function packet(seq, type, data, check = 1) {
  const body = [32 + 2 + data.length + check, 32 + seq, type.charCodeAt(0), ...[...data].map((c) => c.charCodeAt(0))];
  return new Uint8Array([1, ...body, ...kermitCheck(check, body), 13]);
}

test("the page's recorded session: one line per packet, with types, sequence numbers and plain words", () => {
  const lines = decode(read("./fixtures/tx-page.trace"));
  assert.equal(lines[0].desc, "opened at 9600 baud, no parity");
  const init = lines[1];
  assert.deepEqual([init.dir, init.type, init.seq], ["out", "I", 0]);
  assert.match(init.desc, /^init: up to 94 bytes a packet, check type 1$/);
  assert.equal(lines[2].desc, "ack of the init: up to 94 bytes a packet, check type 1");
  assert.equal(lines[3].desc, 'host command: "SATX-bcfbdd"');
  const kinds = new Set(lines.map((l) => l.type));
  for (const k of ["I", "Y", "C", "S", "X", "D", "Z", "B", "G", "F", "R"]) assert.ok(kinds.has(k), `a ${k} packet`);
  // Nothing in a clean session is red, and nothing is left outside a packet.
  assert.deepEqual(lines.filter((l) => l.bad).map(lineText), []);
  assert.deepEqual(lines.filter((l) => l.proto === "bytes").map(lineText), []);
  // A listing asked for, a file named, its data decoded.
  assert.ok(lines.some((l) => l.desc === "server command: directory listing"));
  const data = lines.find((l) => l.type === "D" && l.dir === "out" && /PRG|%%HP/.test(l.text ?? ""));
  assert.ok(data, "a put's data, readable");
  assert.match(data.hex, /^[0-9a-f]{2}( [0-9a-f]{2})*( …)?$/);
});

test("data lines count the file's bytes, of the total the page expects", () => {
  const lines = decode(read("../../crates/saturnus-tx/traces/48gx-xmodem.trace"), { expected: () => 1106 });
  const d = lines.find((l) => l.type === "D" && l.dir === "in");
  assert.match(d.desc, /^data, \d+ bytes \(\d+ of 1,106 bytes\)$/);
  const none = decode(read("../../crates/saturnus-tx/traces/48gx-xmodem.trace")).find((l) => l.type === "D");
  assert.match(none.desc, /bytes so far\)$/);
});

test("the fast transfers: XModem's start, blocks, acks and the end", () => {
  const gx = decode(read("../../crates/saturnus-tx/traces/48gx-xmodem.trace"));
  const start = gx.find((l) => l.proto === "xmodem");
  assert.deepEqual([start.type, start.desc], ["NAK", "ready, with checksum"]);
  assert.equal(start.bad, undefined);
  const block = gx.find((l) => l.type === "SOH");
  assert.deepEqual([block.seq, block.len, block.desc, block.bad], [1, 132, "block 1, 128 bytes, checksum", false]);
  assert.match(block.text, /^HPHP48-R/);
  assert.ok(gx.some((l) => l.type === "EOT" && l.desc === "end of transfer"));

  const g49 = decode(read("../../crates/saturnus-tx/traces/49g-xmodem.trace"));
  assert.ok(g49.some((l) => l.type === "D" && l.proto === "xmodem" && l.desc === "ready, with HP's CRC"));
  const b49 = g49.find((l) => l.type === "SOH");
  assert.deepEqual([b49.len, b49.desc, b49.bad], [133, "block 1, 128 bytes", false]);
  assert.deepEqual(g49.filter((l) => l.proto === "bytes"), [], "nothing outside a packet");
});

test("red: a bad check, a packet sent again, a NAK, an error packet, a packet cut off", () => {
  const log = new CableLog({ now: () => 0 });
  const d = new CableDecoder({ source: "right", log });
  d.open({ baud: 9600 });
  const good = packet(3, "D", "hello");
  d.feed("out", good);
  d.feed("in", packet(3, "N", "", 1));
  d.feed("out", good);
  const broken = good.slice();
  broken[6] ^= 1;
  d.feed("out", broken);
  d.feed("in", packet(4, "E", "Bad Argument Type"));
  d.feed("out", new Uint8Array([1, 0x2c, 0x23]));
  d.feed("in", packet(0, "Y", ""));
  const lines = log.since(0).slice(1);
  assert.deepEqual(lines.map((l) => [l.type, Boolean(l.bad)]), [["D", false], ["N", true], ["D", true], ["D", true], ["E", true], ["…", true], ["Y", false]]);
  assert.equal(lines[1].desc, "nak: send 3 again");
  assert.equal(lines[2].desc, "data, 5 bytes (5 bytes so far), resent");
  assert.match(lines[3].desc, /\(bad check\)$/);
  assert.equal(lines[4].desc, "error: Bad Argument Type");
  assert.equal(lines[5].desc, "cut off: an incomplete packet");
  assert.equal(lineText(lines[1]).includes("! nak"), true);
});

test("parity: bit 7 is the line's, not the packet's", () => {
  const log = new CableLog({ now: () => 0 });
  const d = new CableDecoder({ source: "left", log });
  d.open({ baud: 2400, parity: "even" });
  const p = packet(1, "Y", "");
  d.feed("in", p.map((b) => b | 0x80));
  const [opened, y] = log.since(0);
  assert.equal(opened.desc, "opened at 2400 baud, even parity");
  assert.deepEqual([y.type, y.seq, y.bad], ["Y", 1, false]);
});

test("a packet in pieces comes out whole; bytes outside a packet are said", () => {
  const log = new CableLog({ now: () => 0 });
  const d = new CableDecoder({ source: "left", log });
  const p = packet(2, "F", "PRG");
  d.feed("in", p.subarray(0, 2));
  d.feed("in", p.subarray(2, 5));
  assert.equal(log.size, 0);
  d.feed("in", p.subarray(5));
  d.feed("in", new TextEncoder().encode("garbage"));
  d.feed("out", packet(2, "Y", ""));
  const lines = log.since(0);
  assert.deepEqual(lines.map((l) => l.desc), ["file name PRG", "7 bytes outside a packet", "ack 2"]);
});

test("the quoting: control, eighth bit and repeat prefixes", () => {
  assert.deepEqual(unquote([0x23, 0x4d, 0x23, 0x4a]), [13, 10]);
  assert.deepEqual(unquote([0x7e, 32 + 5, 0x41], { rept: 0x7e }), [65, 65, 65, 65, 65]);
  assert.deepEqual(unquote([0x26, 0x41], { qbin: 0x26 }), [0xc1]);
  assert.equal(unquote([0x23]), null);
  assert.equal(bytesText([72, 105, 0, 200], 3), "Hi·…");
  assert.equal(bytesHex([1, 2, 255], 2), "01 02 …");
});

test("the ring buffer keeps the newest lines; Clear empties it; the text is one line each", () => {
  let t = 0;
  const log = new CableLog({ capacity: 5, now: () => t });
  const seen = [];
  log.onAdd = (l) => seen.push(l.n);
  for (let i = 0; i < 8; i++) {
    t = i * 10;
    log.add({ source: "left", dir: "out", desc: `line ${i}` });
  }
  assert.deepEqual(seen, [0, 1, 2, 3, 4, 5, 6, 7]);
  assert.equal(log.size, 5);
  assert.equal(log.first, 3);
  assert.deepEqual(log.since(0).map((l) => l.desc), ["line 3", "line 4", "line 5", "line 6", "line 7"]);
  assert.deepEqual(log.since(6).map((l) => l.n), [6, 7]);
  assert.equal(log.text().trimEnd().split("\n").length, 5);
  assert.match(log.text(), /^ {3}0\.030 {2}L {2}→/);
  log.clear();
  assert.equal(log.size, 0);
  assert.equal(log.text(), "");
  assert.deepEqual(log.since(0), []);
  log.add({ source: "right", dir: "in", desc: "after" });
  assert.deepEqual(log.since(0).map((l) => [l.n, l.t]), [[8, 0]]);
});

test("a tapped link changes nothing: the recorded session plays through it byte for byte", async () => {
  const trace = read("./fixtures/tx-page.trace");
  const steps = parseTrace(trace).filter((s) => s.kind !== "note");
  const fake = new FakeLink(trace, { speed: 1000 });
  const log = new CableLog();
  const link = log.tap(fake, { source: "left" });
  assert.equal(link.label, "test link");
  const got = [];
  link.addEventListener("data", (e) => got.push(...e.detail));
  for (const s of steps) {
    if (s.kind === "open") await link.open({ baud: s.baud, parity: s.parity });
    else if (s.kind === "write") await link.write(s.bytes);
    if (s.kind !== "read") await new Promise((r) => setTimeout(r, 5));
  }
  await new Promise((r) => setTimeout(r, 20));
  await link.close();
  assert.equal(fake.failure, null);
  assert.ok(fake.done);
  const reads = steps.filter((s) => s.kind === "read").flatMap((s) => [...s.bytes]);
  assert.deepEqual(got, reads);
  assert.ok(log.size > 100);
  assert.equal(log.since(0).at(-1).desc, "closed");
});

test("a decoder that throws never breaks the link", async () => {
  const fake = new FakeLink("       0 = 9600 none\n       0 > A\n", { speed: 1000 });
  const log = new CableLog();
  const link = log.tap(fake, { source: "left" });
  link.decoder.feed = () => { throw new Error("boom"); };
  await link.open({ baud: 9600, parity: "none" });
  await link.write(new Uint8Array([65]));
  assert.equal(fake.failure, null);
  assert.ok(fake.done);
});

test("what a preview being read says: where from, how far, and how long when it is slow", async () => {
  const { estimateSeconds, readingText } = await import("../tx/preview.js");
  const base = { name: "P", from: "48SX", done: 0, total: 1106, baud: 9600, emulated: false, waiting: null, started: null, at: null };
  assert.deepEqual(readingText(base), { line: "Reading P from the 48SX…", amount: "0 of 1,106 bytes", hint: `Slow over the cable at 9600 baud: about ${Math.round(estimateSeconds(1106, 9600))} s.` });
  // Measured once packets came: the time left.
  assert.deepEqual(readingText({ ...base, done: 412, started: 1000, at: 2000 }), { line: "Reading P from the 48SX…", amount: "412 of 1,106 bytes", hint: "Slow over the cable at 9600 baud: about 2 s left." });
  // Small: no hint. Emulated: no guess from the baud rate, only what is measured.
  assert.equal(readingText({ ...base, total: 40 }).hint, "");
  assert.equal(readingText({ ...base, emulated: true, from: "emulated 48GX" }).hint, "");
  assert.equal(readingText({ ...base, emulated: true, from: "emulated 48GX", done: 100, started: 0, at: 1000 }).hint, "About 10 s left.");
  // Behind another job: it says so, no amount.
  assert.deepEqual(readingText({ ...base, waiting: "Copying PRG…" }), { line: "Reading P from the 48SX…", amount: "Waiting: Copying PRG first.", hint: "" });
  // Never more than all; a folder's read says only what.
  assert.equal(readingText({ ...base, done: 5000 }).amount, "1,106 of 1,106 bytes");
  assert.deepEqual(readingText({ name: "P" }), { line: "Reading P…", amount: "", hint: "" });
  assert.ok(estimateSeconds(1106, 9600) > 4 && estimateSeconds(1106, 9600) < 6);
  assert.ok(estimateSeconds(1106, 2400) > 3 * estimateSeconds(1106, 9600) - 3);
});
