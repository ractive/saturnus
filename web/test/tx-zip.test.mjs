// The transfer page's zip writer (web/tx/zip.js): CRC-32 against its
// check value, every header byte by byte, and the result read back by
// Python's zipfile (a reference reader, when python3 is there) with the
// names, bytes, CRCs and times it holds.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { MAX_BYTES, MAX_ENTRIES, crc32, dosTime, zip, zipParts } from "../tx/zip.js";
import { readZip } from "./zip-reader.mjs";

const enc = new TextEncoder();
const dec = new TextDecoder();

test("CRC-32: the check value, the empty input, and continuing from a part", () => {
  assert.equal(crc32(enc.encode("123456789")), 0xcbf43926);
  assert.equal(crc32(new Uint8Array(0)), 0);
  assert.equal(crc32(enc.encode("The quick brown fox jumps over the lazy dog")), 0x414fa339);
  const all = enc.encode("HPHP48-R and more bytes");
  assert.equal(crc32(all.subarray(8), crc32(all.subarray(0, 8))), crc32(all));
  assert.equal(crc32(new Uint8Array([0xff, 0, 0x80])), crc32(new Uint8Array([0xff, 0, 0x80])) >>> 0, "unsigned");
});

test("DOS times: local time, two-second steps, clamped to 1980", () => {
  const { time, date } = dosTime(new Date(2026, 9, 10, 14, 32, 59));
  assert.equal(date, ((2026 - 1980) << 9) | (10 << 5) | 10);
  assert.equal(time, (14 << 11) | (32 << 5) | 29);
  assert.deepEqual(dosTime(new Date(1970, 0, 1)), { time: 0, date: (1 << 5) | 1 });
});

test("every header byte: stored, UTF-8 names, CRCs and sizes, Unix modes, offsets", () => {
  const when = new Date(2026, 9, 10, 14, 32, 10);
  const files = [
    { name: "HOME", directory: true, modified: when },
    { name: "HOME/A→B.hp", data: enc.encode("HPHP48-R\x2c\x2a payload"), modified: when },
    { name: "HOME/EMPTY.txt", data: new Uint8Array(0), modified: when },
    { name: "HOME/D1/", directory: true, modified: when },
  ];
  const bytes = zip(files);
  const entries = readZip(bytes);
  assert.deepEqual(entries.map((e) => e.name), ["HOME/", "HOME/A→B.hp", "HOME/EMPTY.txt", "HOME/D1/"]);
  for (const e of entries) {
    assert.equal(e.method, 0, "stored");
    assert.equal(e.flags, 0x0800, "UTF-8 names");
    assert.equal(e.needed, 10);
    assert.equal(e.madeBy, (3 << 8) | 20, "made on Unix, version 2.0");
    assert.equal(e.compressed, e.size);
    assert.equal(e.crc, crc32(e.data));
    assert.deepEqual({ time: e.time, date: e.date }, dosTime(when));
    assert.equal(e.extra, 0);
    assert.equal(e.comment, 0);
  }
  assert.equal(entries[0].external, ((0o40755 << 16) | 0x10) >>> 0, "a directory, 0755");
  assert.equal(entries[1].external, (0o100644 << 16) >>> 0, "a file, 0644");
  assert.equal(dec.decode(entries[1].data), "HPHP48-R\x2c\x2a payload");
  assert.equal(entries[2].crc, 0);
  assert.equal(entries[0].offset, 0);
  assert.equal(entries[1].offset, 30 + "HOME/".length);
  // The empty zip is the end record alone.
  assert.deepEqual([...zip([])], [0x50, 0x4b, 0x05, 0x06, ...new Array(18).fill(0)]);
});

test("refused: a repeated name, an absolute or climbing path, a backslash", () => {
  const data = new Uint8Array([1]);
  assert.throws(() => zip([{ name: "A", data }, { name: "A", data }]), /A twice/);
  assert.throws(() => zip([{ name: "D", directory: true }, { name: "D/", directory: true }]), /D\/ twice/);
  for (const name of ["/A", "../A", "D/../A", "D//A", "D\\A", "", "./A", "A/"]) {
    assert.throws(() => zip([{ name, data }]), RangeError, JSON.stringify(name));
  }
  assert.throws(() => zip([{ name: "A", data }, { name: "A", data }]), (e) => e.code === "bad-name");
  assert.throws(() => zip([{ name: "x".repeat(0xffff), data }]), (e) => e.code === "bad-name" && /too long/.test(e.message));
});

test("the Zip64 markers are never written: 0xFFFF entries and 0xFFFFFFFF bytes are refused", () => {
  const data = new Uint8Array(0);
  assert.equal(MAX_ENTRIES, 0xfffe);
  assert.equal(MAX_BYTES, 0xfffffffe);
  const many = Array.from({ length: 0xffff }, (_, i) => ({ name: `F${i}`, data }));
  assert.throws(() => zip(many), (e) => e instanceof RangeError && e.code === "too-big" && /65534/.test(e.message));
  // One fewer is the most a zip without Zip64 holds; the end record says so.
  const most = zip(many.slice(1));
  const view = new DataView(most.buffer, most.length - 22);
  assert.equal(view.getUint16(10, true), 0xfffe);
  // A file of 0xFFFFFFFF bytes (a stand-in with that length; nothing is read before the check).
  assert.throws(() => zip([{ name: "BIG", data: { length: 0xffffffff, __proto__: Uint8Array.prototype } }]), (e) => e.code === "too-big");
  // zipParts: the same bytes, the files' own arrays among the parts.
  const own = new Uint8Array([1, 2, 3]);
  const parts = zipParts([{ name: "A", data: own }]);
  assert.ok(parts.includes(own), "not copied");
  assert.deepEqual(Buffer.concat(parts), Buffer.from(zip([{ name: "A", data: own }])));
});

const python = spawnSync("python3", ["--version"]).status === 0;

test("Python's zipfile reads it back: names, bytes, CRCs, times", { skip: !python && "no python3" }, () => {
  const when = new Date(2025, 0, 2, 3, 4, 6);
  const big = new Uint8Array(70_000).map((_, i) => (i * 31) & 0xff);
  const files = [
    { name: "hp-files/", directory: true, modified: when },
    { name: "hp-files/PRG.hp", data: big, modified: when },
    { name: "hp-files/Σx².txt", data: enc.encode("%%HP: T(3)A(R)F(.);\r\n\"hi\""), modified: when },
    { name: "hp-files/%41UX.hp", data: new Uint8Array([0, 255, 13, 10]), modified: when },
    { name: "hp-files/sub/", directory: true, modified: when },
  ];
  const dir = mkdtempSync(join(tmpdir(), "tx-zip-"));
  try {
    const path = join(dir, "t.zip");
    writeFileSync(path, zip(files));
    const script = [
      "import json, sys, zipfile, zlib",
      "z = zipfile.ZipFile(sys.argv[1])",
      "assert z.testzip() is None",
      "print(json.dumps([[i.filename, i.compress_type, list(i.date_time), i.CRC, zlib.crc32(z.read(i)), z.read(i).hex(), i.is_dir(), i.external_attr >> 16] for i in z.infolist()]))",
    ].join("\n");
    const r = spawnSync("python3", ["-c", script, path], { encoding: "utf8" });
    assert.equal(r.status, 0, r.stderr);
    const got = JSON.parse(r.stdout);
    assert.equal(got.length, files.length);
    got.forEach(([name, method, dt, crc, actual, hex, isDir, mode], i) => {
      assert.equal(name, files[i].name);
      assert.equal(method, 0);
      assert.deepEqual(dt, [2025, 1, 2, 3, 4, 6]);
      assert.equal(crc, actual);
      assert.equal(isDir, !!files[i].directory);
      assert.equal(mode, files[i].directory ? 0o40755 : 0o100644);
      assert.equal(hex, Buffer.from(files[i].data ?? []).toString("hex"));
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
