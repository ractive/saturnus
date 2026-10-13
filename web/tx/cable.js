// The cable console's model (iteration 45, kb/docs/saturnus-tx.md, "The
// cable"): every link a pane or the serving screen opens is wrapped in a
// `TappedLink` before the protocol sees it, so the bytes of a real cable
// (Web Serial), the emulated calculator and the desktop app's native
// serial all pass here the same way. A `CableDecoder` per link turns them
// into one line per Kermit packet or XModem block (type, sequence number,
// length, a plain-English description, the payload as text and hex), and
// the lines go into the `CableLog`, a ring buffer of a few thousand. The
// panel (cable-view.js) draws them; while it is closed nothing else runs.
//
// Kermit as kermit-proto writes it (crates/kermit-proto/src/codec.rs,
// params.rs, prefix.rs): MARK (SOH), LEN, SEQ, TYPE, data, check, EOL;
// no long packets. The check type is found by trying (1, 3, 2): the
// negotiation is seen too, but a check that fits is the plainest proof.
// XModem as xmodem-proto: SOH (128) or STX (1024) blocks, n, 255 - n,
// data, one checksum byte or two CRC bytes (after the receiver's "C",
// or "D" for HP's CRC; xmodem-proto's codec.rs).
// Pure apart from the clock: web/test/tx-cable.test.mjs.

import { Link } from "./links.js";

const SOH = 0x01;
const STX = 0x02;
const EOT = 0x04;
const ACK = 0x06;
const NAK = 0x15;
const CAN = 0x18;
const CR = 0x0d;
const LF = 0x0a;

/** How many lines the log keeps. */
export const RING = 3000;
/** How much of a payload a line shows. */
export const TEXT_CHARS = 40;
export const HEX_BYTES = 16;

const unchar = (b) => (b - 32) & 0xff;
const tochar = (x) => (x + 32) & 0xff;
const ctl = (b) => b ^ 64;

/** Kermit's three block checks of `body` (LEN through the last data byte), as bytes. */
export function kermitCheck(type, body) {
  if (type === 1) {
    let s = 0;
    for (const b of body) s += b;
    return [tochar((s + ((s & 192) >> 6)) & 63)];
  }
  if (type === 2) {
    let s = 0;
    for (const b of body) s += b;
    s &= 0xfff;
    return [tochar((s >> 6) & 63), tochar(s & 63)];
  }
  let crc = 0;
  for (const b of body) {
    crc = (crc >> 4) ^ (((crc ^ b) & 15) * 0x1081);
    crc = (crc >> 4) ^ (((crc ^ (b >> 4)) & 15) * 0x1081);
  }
  crc &= 0xffff;
  return [tochar((crc >> 12) & 15), tochar((crc >> 6) & 63), tochar(crc & 63)];
}

/** A Send-Init's fields (kermit-proto's `InitParams::decode`, the ones the console names). */
export function initParams(data) {
  const field = (i) => (data[i] !== undefined && data[i] !== 0x20 ? data[i] : null);
  const maxl = field(0) !== null ? unchar(field(0)) || 80 : 80;
  return {
    maxl,
    time: field(1) !== null ? unchar(field(1)) : 5,
    qctl: field(5) ?? 0x23,
    qbin: data[6] ?? 0x4e,
    chkt: field(7) ?? 0x31,
    rept: data[8] ?? 0x20,
  };
}

const isPrefix = (c) => (c >= 33 && c <= 62) || (c >= 96 && c <= 126);

/** The quoting both sides use after an init exchange (kermit-proto's `negotiate`). */
export function negotiate(a, b) {
  let qbin = null;
  if (isPrefix(a.qbin) && (b.qbin === 0x59 || b.qbin === a.qbin)) qbin = a.qbin;
  else if (isPrefix(b.qbin) && (a.qbin === 0x59 || a.qbin === b.qbin)) qbin = b.qbin;
  let rept = a.rept === b.rept && isPrefix(a.rept) ? a.rept : null;
  const isQctl = (c) => c === a.qctl || c === b.qctl;
  if (qbin !== null && isQctl(qbin)) qbin = null;
  if (rept !== null && (isQctl(rept) || rept === qbin)) rept = null;
  return { qbin, rept };
}

/** A data field without its prefixes (kermit-proto's `prefix::decode`), or null when it is cut short. */
export function unquote(data, { qctl = 0x23, qbin = null, rept = null } = {}) {
  const out = [];
  let i = 0;
  const next = () => (i < data.length ? data[i++] : null);
  while (i < data.length) {
    let b = next();
    let n = 1;
    if (rept !== null && b === rept) {
      const c = next();
      b = next();
      if (c === null || b === null) return null;
      n = unchar(c);
    }
    let hi = 0;
    if (qbin !== null && b === qbin) {
      hi = 0x80;
      b = next();
      if (b === null) return null;
    }
    if (b === qctl) {
      b = next();
      if (b === null) return null;
      if ((b & 0x7f) >= 63 && (b & 0x7f) <= 95) b = ctl(b);
    }
    for (let k = 0; k < n; k++) out.push(b | hi);
  }
  return out;
}

/** Bytes as text: printable ASCII as it is, anything else "·"; at most `max` characters. */
export function bytesText(bytes, max = TEXT_CHARS) {
  let s = "";
  const n = Math.min(bytes.length, max);
  for (let i = 0; i < n; i++) {
    const b = bytes[i];
    s += b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : "·";
  }
  return bytes.length > max ? `${s}…` : s;
}

/** Bytes as hex pairs; at most `max` bytes. */
export function bytesHex(bytes, max = HEX_BYTES) {
  const n = Math.min(bytes.length, max);
  const parts = [];
  for (let i = 0; i < n; i++) parts.push(bytes[i].toString(16).padStart(2, "0"));
  return bytes.length > max ? `${parts.join(" ")} …` : parts.join(" ");
}

const grouped = (n) => n.toLocaleString("en-US");

/** The generic server commands (Kermit's G packet), by their letter. */
const GENERIC = {
  F: "finish (leave server mode)", L: "log out", C: "change directory", D: "directory listing", E: "delete",
  R: "rename", U: "free space", T: "type", I: "log in", K: "copy", M: "message", Q: "status", H: "help",
  V: "variable", W: "who", P: "run a program", S: "submit",
};

/** What each Kermit packet type is, in plain words. */
const KERMIT = {
  S: "send-init", I: "init", Y: "ack", N: "nak", F: "file name", X: "text to show", A: "file attributes",
  D: "data", Z: "end of file", B: "end of transfer", E: "error", G: "server command", R: "receive request",
  C: "host command", K: "kermit command", T: "reserved",
};

/**
 * The bytes of one link as lines. `source` names the place ("left",
 * "right", "serve"); `log` takes the lines; `expected()` the total bytes
 * of the running transfer, when the page knows it (the listing's size),
 * for "data, 412 of 1,106 bytes".
 */
export class CableDecoder {
  constructor({ source, log, expected = () => null }) {
    this.source = source;
    this.log = log;
    this.expected = expected;
    this.mask = 0xff;
    this.reset();
  }

  reset() {
    this.dirs = { out: this.fresh(), in: this.fresh() };
    /** The quoting after the last init exchange: qbin and rept, both sides alike. */
    this.quoting = { qbin: null, rept: null };
    /** The check type both sides agreed on in the last init exchange (null: none seen; kermit-proto's `negotiate`). */
    this.check = null;
    /** Bytes of data in the current file, and whether XModem's CRC was asked for. */
    this.fileBytes = 0;
    this.crc = false;
    this.blocks = 0;
  }

  fresh() { return { buf: new Uint8Array(0), junk: [], last: null, init: null, qctl: 0x23 }; }

  emit(line) { this.log.add({ source: this.source, ...line }); }

  /** The link opened (again) at `settings`: a parity strips bit 7. */
  open({ baud, parity = "none" } = {}) {
    this.flushAll();
    this.reset();
    this.mask = parity && parity !== "none" ? 0x7f : 0xff;
    this.emit({ dir: "link", proto: "link", desc: `opened at ${baud} baud, ${parity === "none" || !parity ? "no parity" : `${parity} parity`}` });
  }

  close() {
    this.flushAll();
    this.emit({ dir: "link", proto: "link", desc: "closed" });
  }

  lost(reason) {
    this.flushAll();
    this.emit({ dir: "link", proto: "link", desc: `lost: ${reason}`, bad: true });
  }

  /** Bytes that went `dir` ("out" to the calculator, "in" from it). */
  feed(dir, bytes) {
    const st = this.dirs[dir];
    if (!st || !bytes?.length) return;
    // Half duplex: when one side speaks, what the other left unfinished is all it will send.
    const otherDir = dir === "out" ? "in" : "out";
    const other = this.dirs[otherDir];
    if (other.buf.length && !this.streaming(other)) this.cut(otherDir);
    else this.flushJunk(otherDir);
    const joined = new Uint8Array(st.buf.length + bytes.length);
    joined.set(st.buf);
    joined.set(bytes, st.buf.length);
    st.buf = joined;
    this.scan(dir);
  }

  /** Whether `st` waits for the rest of an XModem block (the receiver is quiet while it streams). */
  streaming(st) {
    const b = st.buf;
    return b.length >= 3 && (b[0] === SOH || b[0] === STX) && ((b[1] + b[2]) & 0xff) === 255;
  }

  /** What is left in `dir` as one line: a packet cut off. */
  cut(dir) {
    const st = this.dirs[dir];
    const rest = st.buf;
    st.buf = new Uint8Array(0);
    this.flushJunk(dir);
    if (rest.length) this.emit({ dir, proto: "bytes", type: "…", len: rest.length, desc: "cut off: an incomplete packet", text: bytesText(rest), hex: bytesHex(rest), bad: true });
  }

  flushAll() {
    for (const dir of ["out", "in"]) {
      if (this.dirs?.[dir]?.buf.length) this.cut(dir);
      else if (this.dirs?.[dir]) this.flushJunk(dir);
    }
  }

  flushJunk(dir) {
    const st = this.dirs[dir];
    if (!st.junk.length) return;
    const j = st.junk;
    st.junk = [];
    this.emit({ dir, proto: "bytes", len: j.length, desc: j.length === 1 ? "1 byte outside a packet" : `${j.length} bytes outside a packet`, text: bytesText(j), hex: bytesHex(j) });
  }

  scan(dir) {
    const st = this.dirs[dir];
    const b = st.buf;
    let at = 0;
    while (at < b.length) {
      const c = b[at] & this.mask;
      if (c === SOH || c === STX) {
        if (b.length - at < 3) break;
        const n = b[at + 1];
        const inv = b[at + 2];
        if (((n + inv) & 0xff) === 255) {
          const size = (c === STX ? 1024 : 128) + 3 + (this.crc ? 2 : 1);
          if (b.length - at < size) break;
          this.flushJunk(dir);
          this.xmodemBlock(dir, b.subarray(at, at + size));
          at += size;
          continue;
        }
        if (c === STX) {
          st.junk.push(b[at++]);
          continue;
        }
        const len = unchar(b[at + 1] & this.mask);
        if (len < 3 || len > 94) {
          st.junk.push(b[at++]);
          continue;
        }
        const end = at + 2 + len;
        // A mark inside the span: this one was cut off (kermit-proto's Deframer does the same).
        let resync = -1;
        for (let k = at + 1; k < Math.min(end, b.length); k++) {
          if ((b[k] & this.mask) === SOH) {
            resync = k;
            break;
          }
        }
        if (resync >= 0) {
          this.flushJunk(dir);
          const part = b.subarray(at, resync);
          this.emit({ dir, proto: "bytes", type: "…", len: part.length, desc: "cut off: an incomplete packet", text: bytesText(part), hex: bytesHex(part), bad: true });
          at = resync;
          continue;
        }
        if (b.length < end) break;
        this.flushJunk(dir);
        this.kermitPacket(dir, b.subarray(at, end));
        at = end;
        continue;
      }
      if (c === CR || c === LF || c === 0) {
        at++;
        continue;
      }
      if (st.junk.length === 0 && [ACK, NAK, EOT, CAN, 0x43, 0x44].includes(c) && this.xmodemByte(dir, c)) {
        at++;
        continue;
      }
      st.junk.push(b[at++]);
    }
    st.buf = at ? b.slice(at) : b;
    // What is outside any packet comes out once a packet or the other side follows, or when it is long.
    if (st.junk.length >= 64) this.flushJunk(dir);
  }

  // ------------------------------------------------------------ Kermit

  kermitPacket(dir, frame) {
    const m = this.mask;
    const bytes = Array.from(frame, (x) => x & m);
    const len = unchar(bytes[1]);
    const seq = unchar(bytes[2]);
    const type = String.fromCharCode(bytes[3]);
    const rest = bytes.slice(4);
    // The check type that fits: a NAK's length says it; an init and its ack use type 1; else the
    // agreed one first (a type-3 packet passes the type-1 test one time in 64), then the others.
    const body = bytes.slice(1);
    const initAck = type === "Y" && Boolean(this.dirs[dir === "out" ? "in" : "out"].init);
    const first = type === "S" || type === "I" || initAck ? 1 : (this.check ?? 1);
    let check = 0;
    for (const t of type === "N" ? [len - 2] : [first, ...[1, 3, 2].filter((x) => x !== first)]) {
      if (t < 1 || t > 3 || rest.length < t) continue;
      const want = kermitCheck(t, body.slice(0, body.length - t));
      if (want.every((x, i) => x === body[body.length - t + i])) {
        check = t;
        break;
      }
    }
    const bad = check === 0;
    const data = rest.slice(0, rest.length - (check || Math.min(1, rest.length)));
    const st = this.dirs[dir];
    const key = `${seq}${type}${bytes.join(",")}`;
    const resent = st.last === key;
    st.last = key;
    const line = { dir, proto: "kermit", type, seq, len: frame.length, bad: bad || resent || type === "N" || type === "E" };
    // The payload before the description: an init's ack changes the quoting from the next packet on.
    const shown = this.payload(dir, type, data);
    line.desc = this.kermitDesc(dir, type, seq, data, resent);
    if (bad) line.desc += " (bad check)";
    if (shown.length) {
      line.text = bytesText(shown);
      line.hex = bytesHex(shown);
    }
    this.emit(line);
  }

  /** The data a line shows: decoded where the packet carries file bytes or names, else as sent. */
  payload(dir, type, data) {
    if (["D", "F", "X", "E", "R", "G", "C", "A", "Y"].includes(type)) {
      if (type === "Y" && this.dirs[dir === "out" ? "in" : "out"].init) return data;
      return unquote(data, { qctl: this.dirs[dir].qctl, ...this.quoting }) ?? data;
    }
    return data;
  }

  kermitDesc(dir, type, seq, data, resent) {
    const other = this.dirs[dir === "out" ? "in" : "out"];
    const st = this.dirs[dir];
    const name = KERMIT[type] ?? `type ${type}`;
    const again = resent ? ", resent" : "";
    switch (type) {
      case "S":
      case "I": {
        const p = initParams(data);
        st.init = p;
        st.qctl = p.qctl;
        this.check = null;
        if (type === "S") this.fileBytes = 0;
        return `${name}: up to ${p.maxl} bytes a packet, check type ${String.fromCharCode(p.chkt)}${again}`;
      }
      case "Y": {
        if (other.init) {
          const p = initParams(data);
          st.init = null;
          st.qctl = p.qctl;
          this.quoting = negotiate(other.init, p);
          this.check = other.init.chkt === p.chkt && [0x31, 0x32, 0x33].includes(p.chkt) ? p.chkt - 0x30 : 1;
          other.init = null;
          return `ack of the init: up to ${p.maxl} bytes a packet, check type ${String.fromCharCode(p.chkt)}${again}`;
        }
        return `ack ${seq}${again}`;
      }
      case "N":
        return `nak: send ${seq} again${again}`;
      case "F":
      case "X":
        this.fileBytes = 0;
        return `${type === "F" ? "file name" : "text header"} ${bytesText(this.payload(dir, type, data))}${again}`;
      case "R":
        this.fileBytes = 0;
        return `receive request ${bytesText(this.payload(dir, type, data))}${again}`;
      case "D": {
        const n = (this.payload(dir, type, data) ?? []).length;
        if (!resent) this.fileBytes += n;
        const total = this.expected?.() ?? null;
        const sofar = total ? `${grouped(Math.min(this.fileBytes, total))} of ${grouped(total)} bytes` : `${grouped(this.fileBytes)} bytes so far`;
        return `data, ${n} bytes (${sofar})${again}`;
      }
      case "E":
        return `error: ${bytesText(this.payload(dir, type, data), 80)}${again}`;
      case "G": {
        const d = this.payload(dir, type, data);
        const sub = d.length ? String.fromCharCode(d[0]) : "";
        return `server command: ${GENERIC[sub] ?? sub}${again}`;
      }
      case "C":
        return `host command: ${bytesText(this.payload(dir, type, data), 80)}${again}`;
      default:
        return `${name}${again}`;
    }
  }

  // ------------------------------------------------------------ XModem

  xmodemBlock(dir, block) {
    const big = block[0] === STX;
    const n = block[1];
    const data = block.subarray(3, 3 + (big ? 1024 : 128));
    const st = this.dirs[dir];
    let bad = false;
    if (!this.crc) {
      let s = 0;
      for (const b of data) s += b;
      bad = (s & 0xff) !== block[block.length - 1];
    }
    const key = `x${n}`;
    const resent = st.last === key;
    st.last = key;
    this.blocks++;
    this.emit({
      dir, proto: "xmodem", type: big ? "STX" : "SOH", seq: n, len: block.length, bad: bad || resent,
      desc: `block ${n}, ${data.length} bytes${this.crc ? "" : ", checksum"}${bad ? " (bad checksum)" : ""}${resent ? ", resent" : ""}`,
      text: bytesText(data), hex: bytesHex(data),
    });
  }

  /** One of XModem's single bytes, between blocks; false when it is no such byte here. */
  xmodemByte(dir, c) {
    const line = { dir, proto: "xmodem", len: 1, hex: c.toString(16).padStart(2, "0") };
    switch (c) {
      case 0x43:
      case 0x44:
        this.crc = true;
        this.blocks = 0;
        this.emit({ ...line, type: String.fromCharCode(c), desc: c === 0x43 ? "ready, with CRC" : "ready, with HP's CRC" });
        return true;
      case ACK:
        this.emit({ ...line, type: "ACK", desc: "ack" });
        return true;
      case NAK:
        if (this.blocks === 0) {
          this.crc = false;
          this.emit({ ...line, type: "NAK", desc: "ready, with checksum" });
        } else this.emit({ ...line, type: "NAK", desc: "nak: resend the block", bad: true });
        return true;
      case EOT:
        this.emit({ ...line, type: "EOT", desc: "end of transfer" });
        return true;
      case CAN:
        this.emit({ ...line, type: "CAN", desc: "cancelled", bad: true });
        return true;
      default:
        return false;
    }
  }
}

/**
 * The lines, the newest `capacity`. `onAdd` (the open panel's) is called
 * after each line, nothing else; `seq` counts every line ever added, so a
 * reader asks for what came after the last one it drew.
 */
export class CableLog {
  constructor({ capacity = RING, now = () => (globalThis.performance?.now() ?? Date.now()) } = {}) {
    this.capacity = capacity;
    this.now = now;
    this.start = null;
    this.buf = new Array(capacity);
    this.head = 0;
    this.size = 0;
    this.seq = 0;
    /** Set by the open panel; null while it is closed. */
    this.onAdd = null;
  }

  add(line) {
    const at = this.now();
    this.start ??= at;
    const l = { ...line, n: this.seq++, t: at - this.start };
    this.buf[(this.head + this.size) % this.capacity] = l;
    if (this.size < this.capacity) this.size++;
    else this.head = (this.head + 1) % this.capacity;
    this.onAdd?.(l);
  }

  /** The number of the oldest line kept. */
  get first() { return this.seq - this.size; }

  /** The lines kept whose number is `n` or more, oldest first. */
  since(n = 0) {
    const from = Math.max(n, this.first);
    const out = [];
    for (let k = from; k < this.seq; k++) out.push(this.buf[(this.head + (k - this.first)) % this.capacity]);
    return out;
  }

  clear() {
    this.buf = new Array(this.capacity);
    this.head = 0;
    this.size = 0;
    this.start = null;
  }

  /** The lines kept as plain text, one per line (Copy log). */
  text() { return this.since(0).map(lineText).join("\n") + (this.size ? "\n" : ""); }

  /** A link that writes its traffic here as `source` (see `TappedLink`). */
  tap(link, { source, expected } = {}) {
    return new TappedLink(link, new CableDecoder({ source, log: this, expected }));
  }
}

/** The arrow of a line's direction: → to the calculator, ← from it. */
export const arrow = (dir) => (dir === "out" ? "→" : dir === "in" ? "←" : "·");

/** A line's time as seconds since the log began: "12.345". */
export const timeText = (t) => (t / 1000).toFixed(3).padStart(8);

/** A line's packet column: "D 5", "SOH 3", "ACK". */
export const packetText = (l) => (l.type ? (l.seq !== undefined ? `${l.type} ${l.seq}` : l.type) : "");

/** One line as plain text. */
export function lineText(l) {
  const src = { left: "L", right: "R", serve: "S" }[l.source] ?? (l.source ?? "");
  const parts = [timeText(l.t), src, arrow(l.dir), packetText(l).padEnd(6), l.len !== undefined ? `${String(l.len).padStart(4)} B` : "      ", l.bad ? `! ${l.desc}` : l.desc];
  if (l.text) parts.push(`|${l.text}|`);
  if (l.hex) parts.push(l.hex);
  return parts.join("  ");
}

/**
 * A link whose traffic goes to a decoder on its way: the same Link to the
 * protocol (open, write, close, `data`, `lost`), nothing changed or
 * delayed. A decoder that throws never breaks the link.
 */
export class TappedLink extends Link {
  constructor(inner, decoder) {
    super();
    this.inner = inner;
    this.decoder = decoder;
    this.label = inner.label;
    const safe = (fn) => {
      try {
        fn();
      } catch {
        // The console is for looking; the transfer goes on without it.
      }
    };
    this.safe = safe;
    inner.addEventListener("data", (e) => {
      safe(() => decoder.feed("in", e.detail));
      this.emitData(e.detail);
    });
    inner.addEventListener("lost", (e) => {
      safe(() => decoder.lost(String(e.detail)));
      this.emitLost(e.detail);
    });
  }

  /** The inner link's port (Web Serial's, the desktop app's): the page tells held ports apart by it. */
  get port() { return this.inner.port; }

  async open(settings) {
    this.safe(() => this.decoder.open(settings));
    await this.inner.open(settings);
  }

  async write(bytes) {
    this.safe(() => this.decoder.feed("out", bytes));
    await this.inner.write(bytes);
  }

  async close() {
    this.safe(() => this.decoder.close());
    await this.inner.close();
  }
}
