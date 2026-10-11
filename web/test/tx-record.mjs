// Records the transfer page's test session (tx-chrome.mjs `pageSession`)
// against the emulated 48GX (gxrom-r from SATURNUS_ROM_DIR, read only)
// into the trace file named by the first argument
// (web/test/fixtures/tx-page.trace, committed): what the page wrote and
// read on the emulated serial port, with the timing, through a
// RecordingLink around the EmulatorLink. Serial bytes only: protocol
// packets, the listings and the session's own files, no ROM content.
// `just tx-page-trace DIR`. Not a test (the runner takes *.test.mjs).

import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { chrome, findChrome, serve } from "./chrome.mjs";
import { PAGE_SEED, pageSession, txBuilt, txPage } from "./tx-chrome.mjs";

const out = process.argv[2];
const dir = process.env.SATURNUS_ROM_DIR;
const rom = dir && join(dir, "gxrom-r");
if (!out || !rom || !existsSync(rom)) {
  console.error("usage: SATURNUS_ROM_DIR=DIR node web/test/tx-record.mjs OUT.trace (DIR holds gxrom-r)");
  process.exit(2);
}
if (!txBuilt()) {
  console.error("build both wasm packages first (just web tx-web)");
  process.exit(2);
}
const binary = findChrome();
if (!binary) {
  console.error("no Chrome found (SATURNUS_CHROME=/path/to/chrome)");
  process.exit(2);
}

const { server, port } = await serve({ "/rom.bin": rom });
const c = await chrome(binary);
try {
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  const p = txPage(c);
  await p.open(port);
  await c.ev(`tx.app.deviceOptions = { seed: ${PAGE_SEED} }; tx.app.wrapLink = (l) => (window.__rec = new tx.links.RecordingLink(l)); true`);
  await c.ev(`fetch("/rom.bin").then((r) => r.arrayBuffer()).then((b) => { tx.panes[0].addEmulated(new Uint8Array(b), "gxrom-r", "48gx"); return true; })`);
  await p.until("!!tx.panes[0].place?.device?.connection", 120_000, "connected to the emulated 48GX");
  const seen = await pageSession(c);
  const trace = await c.ev(`__rec.trace([
    "saturnus tx trace: the page session of web/test/tx-chrome.mjs on the",
    "emulated 48GX (ROM file gxrom-r), recorded by \`just tx-page-trace\`.",
    "Serial bytes only; times in ms since the link was made; seed ${PAGE_SEED}.",
  ])`);
  writeFileSync(out, trace);
  console.log(`wrote ${out}: ${trace.split("\n").length} lines; saw ${JSON.stringify({ ...seen, download: { name: seen.download.name, length: seen.download.bytes.length } })}`);
} finally {
  await c.close();
  server.closeAllConnections();
  server.close();
}
