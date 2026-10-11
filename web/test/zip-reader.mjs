// A zip read back from its central directory, each local header checked
// against it: for web/test/tx-zip.test.mjs and tx-folder.test.mjs.

import assert from "node:assert/strict";

const dec = new TextDecoder();

/** The zip read back from its central directory, each local header checked against it. */
export function readZip(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const end = bytes.length - 22;
  assert.equal(view.getUint32(end, true), 0x06054b50, "end of central directory");
  const count = view.getUint16(end + 10, true);
  assert.equal(view.getUint16(end + 8, true), count);
  const size = view.getUint32(end + 12, true);
  let at = view.getUint32(end + 16, true);
  assert.equal(at + size, end, "the central directory ends where the end record starts");
  assert.equal(view.getUint16(end + 20, true), 0, "no comment");
  const entries = [];
  for (let i = 0; i < count; i++) {
    assert.equal(view.getUint32(at, true), 0x02014b50, "central header");
    const c = {
      madeBy: view.getUint16(at + 4, true), needed: view.getUint16(at + 6, true), flags: view.getUint16(at + 8, true),
      method: view.getUint16(at + 10, true), time: view.getUint16(at + 12, true), date: view.getUint16(at + 14, true),
      crc: view.getUint32(at + 16, true), compressed: view.getUint32(at + 20, true), size: view.getUint32(at + 24, true),
      nameLength: view.getUint16(at + 28, true), extra: view.getUint16(at + 30, true), comment: view.getUint16(at + 32, true),
      external: view.getUint32(at + 38, true), offset: view.getUint32(at + 42, true),
    };
    c.name = dec.decode(bytes.subarray(at + 46, at + 46 + c.nameLength));
    at += 46 + c.nameLength;
    const l = c.offset;
    assert.equal(view.getUint32(l, true), 0x04034b50, `${c.name}: local header`);
    for (const [off, key] of [[4, "needed"], [6, "flags"], [8, "method"], [10, "time"], [12, "date"], [26, "nameLength"]]) {
      assert.equal(view.getUint16(l + off, true), c[key], `${c.name}: local ${key}`);
    }
    for (const [off, key] of [[14, "crc"], [18, "compressed"], [22, "size"]]) {
      assert.equal(view.getUint32(l + off, true), c[key], `${c.name}: local ${key}`);
    }
    assert.equal(view.getUint16(l + 28, true), 0, "no extra field");
    assert.equal(dec.decode(bytes.subarray(l + 30, l + 30 + c.nameLength)), c.name);
    c.data = bytes.slice(l + 30 + c.nameLength, l + 30 + c.nameLength + c.size);
    entries.push(c);
  }
  return entries;
}
