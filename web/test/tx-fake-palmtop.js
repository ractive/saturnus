// An HP 100LX palmtop in Filer server mode for Files' tests (not a test
// itself): filer-proto's server over its in-memory files, through the
// wasm's `TxFilerServer` (crates/saturnus-web/src/tx/palmtop.rs), on a
// Link (web/tx/links.js) of its own. Loaded by the node tests
// (tx-palmtop.test.mjs) and into the page (tx-page.test.mjs). The
// protocol is the real one both ways: the page's TxPalmtop in its Worker
// writes Filer frames, the server answers them.
//
// `palmtopLink(TxFilerServer, files, {label})` gives a Link with
// `server` (the TxFilerServer: `file(path)`, `paths()`), `opened` (the
// speeds opened at), `silence()` (no more replies), `answer()` (replies
// again) and `unplug(reason)` (the link lost).

import { Link } from "../tx/links.js";

/** The palmtop's files of the tests: `C:\HELLO.TXT`, a 5000-byte `C:\BIG.BIN`, `C:\SUB` with two levels, `D:\ROM.TXT` (D: write-protected). */
export const PALMTOP_FILES = () => [
  { path: "C:\\HELLO.TXT", data: new TextEncoder().encode("Hello, palmtop\r\n") },
  { path: "C:\\BIG.BIN", data: Uint8Array.from({ length: 5000 }, (_, i) => i % 251) },
  { path: "C:\\SUB\\NOTE.TXT", data: new TextEncoder().encode("note") },
  { path: "C:\\SUB\\DEEP\\A.TXT", data: new TextEncoder().encode("a") },
  { path: "D:\\ROM.TXT", data: new TextEncoder().encode("rom") },
];

export function palmtopLink(TxFilerServer, files = PALMTOP_FILES(), { label = "test palmtop", delay = 1 } = {}) {
  const server = new TxFilerServer(files);
  const link = new Link();
  link.label = label;
  link.server = server;
  link.opened = [];
  link.closed = false;
  let silent = false;
  link.open = async ({ baud }) => {
    link.opened.push(baud);
  };
  link.write = async (bytes) => {
    if (link.closed) throw new Error("the link is closed");
    if (silent) return;
    server.input(bytes);
    const out = [...server.output()];
    if (out.length) setTimeout(() => out.forEach((b) => link.emitData(b)), delay);
  };
  link.close = async () => {
    link.closed = true;
  };
  link.silence = () => {
    silent = true;
  };
  link.answer = () => {
    silent = false;
  };
  link.unplug = (reason = "the device has been lost") => link.emitLost(reason);
  return link;
}

/**
 * A Web Serial port (`navigator.serial.requestPort()`'s) with the same
 * server behind it, for the page's real WebSerialLink: `opened` the
 * speeds opened at, `server`, `silence()`, `answer()`.
 */
export function fakeSerialPort(TxFilerServer, files = PALMTOP_FILES()) {
  const server = new TxFilerServer(files);
  let silent = false;
  let feed = null;
  const port = {
    server,
    opened: [],
    readable: null,
    writable: null,
    getInfo: () => ({ usbVendorId: 0x0403, usbProductId: 0x6001 }),
    async open({ baudRate }) {
      if (port.readable) throw Object.assign(new Error("already open"), { name: "InvalidStateError" });
      port.opened.push(baudRate);
      port.readable = new ReadableStream({ start: (c) => { feed = c; } });
      port.writable = new WritableStream({
        write: (bytes) => {
          if (silent) return;
          server.input(bytes);
          const out = [...server.output()];
          setTimeout(() => out.forEach((b) => feed?.enqueue(b)), 1);
        },
      });
    },
    async close() {
      port.readable = null;
      port.writable = null;
      feed = null;
    },
    silence: () => { silent = true; },
    answer: () => { silent = false; },
  };
  return port;
}
