// A `.zip` made in the page, for downloading a directory or a selection of
// the transfer page's folder (web/tx/folder.js; kb/docs/saturnus-tx.md,
// "The local folder"). STORE only: calculator files are small and mostly
// do not shrink, and a deflater is not worth its code here. Names in
// UTF-8 (general purpose flag bit 11), so `A→B.hp` stays itself; times in
// the DOS form of the local time, as every unzip reads them; Unix
// permissions in the central directory (0644 files, 0755 directories).
// No Zip64: more than 65 535 entries or 4 GiB is refused.
// Tested by web/test/tx-zip.test.mjs (headers, CRCs, Python's zipfile).

const LOCAL = 0x04034b50;
const CENTRAL = 0x02014b50;
const END = 0x06054b50;
/** UTF-8 names. */
const FLAG_UTF8 = 0x0800;
/** Version 2.0 made on Unix (3), so the permissions in the external attributes count. */
const MADE_BY = (3 << 8) | 20;
const NEEDED = 10;

let table = null;

/** The CRC-32 of `bytes` (ISO 3309, the one zip uses), continuing from `crc`. */
export function crc32(bytes, crc = 0) {
  if (!table) {
    table = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c >>> 0;
    }
  }
  let c = ~crc >>> 0;
  for (let i = 0; i < bytes.length; i++) c = table[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return ~c >>> 0;
}

/** `when` (a Date or milliseconds) as the DOS time and date, local time, clamped to 1980-2107. */
export function dosTime(when) {
  const d = new Date(when ?? Date.now());
  const year = Math.min(Math.max(d.getFullYear(), 1980), 2107);
  const time = (d.getHours() << 11) | (d.getMinutes() << 5) | (d.getSeconds() >> 1);
  const date = ((year - 1980) << 9) | ((d.getMonth() + 1) << 5) | d.getDate();
  return year === d.getFullYear() ? { time, date } : { time: 0, date: ((year - 1980) << 9) | (1 << 5) | 1 };
}

/** Why `name` cannot be a zip entry's name, or null. */
function nameProblem(name, directory) {
  if (typeof name !== "string" || !name) return "an empty name";
  if (name.startsWith("/") || name.includes("\\")) return `${name}: not a relative path with /`;
  const parts = (directory ? name.replace(/\/$/, "") : name).split("/");
  if (parts.some((p) => !p || p === "." || p === "..")) return `${name}: an empty, . or .. part`;
  if (!directory && name.endsWith("/")) return `${name}: a file name ending in /`;
  return null;
}

/**
 * A zip of `entries`, in their order: `{name, data, modified}` for a file
 * (`name` the path in the zip, `D1/A.hp`), `{name, directory: true,
 * modified}` for a directory (written `D1/`; only needed for an empty
 * one). Throws a RangeError for a bad or repeated name or beyond the
 * format's limits.
 */
export function zip(entries) {
  if (entries.length > 0xffff) throw new RangeError(`${entries.length} entries: a zip without Zip64 holds 65 535`);
  const enc = new TextEncoder();
  const seen = new Set();
  const parts = [];
  const central = [];
  let offset = 0;
  for (const entry of entries) {
    const directory = !!entry.directory;
    const problem = nameProblem(entry.name, directory);
    if (problem) throw new RangeError(`zip: ${problem}`);
    const name = directory && !entry.name.endsWith("/") ? `${entry.name}/` : entry.name;
    if (seen.has(name)) throw new RangeError(`zip: ${name} twice`);
    seen.add(name);
    const nameBytes = enc.encode(name);
    if (nameBytes.length > 0xffff) throw new RangeError(`zip: ${name} is too long`);
    const data = directory ? new Uint8Array(0) : entry.data instanceof Uint8Array ? entry.data : new Uint8Array(entry.data ?? []);
    if (data.length > 0xfffffffe) throw new RangeError(`zip: ${name} is larger than 4 GiB`);
    const crc = crc32(data);
    const { time, date } = dosTime(entry.modified);

    const local = new DataView(new ArrayBuffer(30));
    local.setUint32(0, LOCAL, true);
    local.setUint16(4, NEEDED, true);
    local.setUint16(6, FLAG_UTF8, true);
    local.setUint16(8, 0, true); // stored
    local.setUint16(10, time, true);
    local.setUint16(12, date, true);
    local.setUint32(14, crc, true);
    local.setUint32(18, data.length, true);
    local.setUint32(22, data.length, true);
    local.setUint16(26, nameBytes.length, true);
    local.setUint16(28, 0, true);
    parts.push(new Uint8Array(local.buffer), nameBytes, data);

    const head = new DataView(new ArrayBuffer(46));
    head.setUint32(0, CENTRAL, true);
    head.setUint16(4, MADE_BY, true);
    head.setUint16(6, NEEDED, true);
    head.setUint16(8, FLAG_UTF8, true);
    head.setUint16(10, 0, true);
    head.setUint16(12, time, true);
    head.setUint16(14, date, true);
    head.setUint32(16, crc, true);
    head.setUint32(20, data.length, true);
    head.setUint32(24, data.length, true);
    head.setUint16(28, nameBytes.length, true);
    head.setUint16(30, 0, true); // extra
    head.setUint16(32, 0, true); // comment
    head.setUint16(34, 0, true); // disk
    head.setUint16(36, 0, true); // internal attributes
    const mode = directory ? 0o40755 : 0o100644;
    head.setUint32(38, ((mode << 16) | (directory ? 0x10 : 0)) >>> 0, true);
    head.setUint32(42, offset, true);
    central.push(new Uint8Array(head.buffer), nameBytes);

    offset += 30 + nameBytes.length + data.length;
    if (offset > 0xffffffff) throw new RangeError("zip: more than 4 GiB");
  }
  const centralSize = central.reduce((n, p) => n + p.length, 0);
  if (offset + centralSize > 0xffffffff) throw new RangeError("zip: more than 4 GiB");
  const end = new DataView(new ArrayBuffer(22));
  end.setUint32(0, END, true);
  end.setUint16(8, entries.length, true);
  end.setUint16(10, entries.length, true);
  end.setUint32(12, centralSize, true);
  end.setUint32(16, offset, true);
  const all = [...parts, ...central, new Uint8Array(end.buffer)];
  const out = new Uint8Array(all.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of all) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}
