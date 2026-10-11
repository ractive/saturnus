// The hidden-tab measurement of kb/docs/saturnus-tx.md ("Timing in the
// browser"): the transfer page in headless Chrome gets a 30 KB variable
// from the emulated 48GX (gxrom-r from SATURNUS_ROM_DIR, at 1x, as a real
// one) once while visible, then again and again for six minutes while its
// tab is hidden behind another one, then once visible again. Beside it a
// chain of 10 ms timers on the page's main thread counts its wake-ups a
// minute, to show the tab was throttled. Prints one line a get and a
// summary. Not a test (minutes long): `node web/test/tx-hidden.mjs`.

import { existsSync } from "node:fs";
import { join } from "node:path";
import { chrome, findChrome, serve, sleep } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";

const dir = process.env.SATURNUS_ROM_DIR;
const rom = dir && join(dir, "gxrom-r");
if (!rom || !existsSync(rom) || !txBuilt() || !findChrome()) {
  console.error("needs SATURNUS_ROM_DIR with gxrom-r, Chrome and both wasm packages");
  process.exit(2);
}
const HIDDEN_MS = Number(process.env.TX_HIDDEN_MS ?? 6 * 60_000);
const SIZE = 30 * 1024;

const { server, port } = await serve({ "/rom.bin": rom });
const c = await chrome(findChrome());
try {
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  const p = txPage(c);
  await p.open(port);
  console.log(`Chrome ${(await c.send("Browser.getVersion")).product}`);
  await c.ev(`fetch("/rom.bin").then((r) => r.arrayBuffer()).then((b) => { tx.addEmulated(new Uint8Array(b), "gxrom-r", "48gx"); return true; })`);
  await p.until("!!tx.state.device?.connection", 240_000, "connected");
  await p.idle();
  // A 30 KB string: a file that is no object, stored as a string.
  const text = "0123456789abcdef".repeat(SIZE / 16);
  // Long calls run in the page and are polled: one DevTools call may take 30 s at most.
  await c.ev(`(() => { const t0 = performance.now(); window.__put = null; tx.putFiles([new File([${JSON.stringify(text)}], "BIG.txt")]).then(() => { __put = performance.now() - t0; }); return true; })()`);
  await p.answer("Store it as a string?");
  await p.until("__put !== null", 300_000, "the 30 KB put");
  console.log(`put 30 KB: ${Math.round((await c.ev("__put")) / 1000)} s`);
  await p.idle(120_000);
  // The page's main-thread timers, counted: a chain of 10 ms timeouts.
  await c.ev(`(() => { window.__wakes = []; const tick = () => { __wakes.push(performance.now()); setTimeout(tick, 10); }; tick(); return true; })()`);
  const get = async () => {
    await c.ev(`(() => {
      const t0 = performance.now();
      const w0 = __wakes.length;
      window.__got = null;
      tx.state.device.read(["HOME"], "BIG", { size: ${SIZE} }).then(
        (f) => { __got = { ms: performance.now() - t0, bytes: f.data.length, state: document.visibilityState, wakes: __wakes.length - w0 }; },
        (e) => { __got = { error: String(e.message ?? e), ms: performance.now() - t0, state: document.visibilityState }; });
      return true;
    })()`);
    await p.until("__got !== null", 600_000, "the get");
    return c.ev("__got");
  };
  const rows = [];
  const show = (r, at) => {
    rows.push(r);
    const rate = r.wakes !== undefined ? `, page timers ${(r.wakes / (r.ms / 60_000)).toFixed(0)}/min` : "";
    console.log(`${String(Math.round(at / 1000)).padStart(4)} s ${r.state.padEnd(7)} ${r.error ? `FAILED ${r.error}` : `${r.bytes} bytes in ${(r.ms / 1000).toFixed(1)} s`}${rate}`);
  };
  const start = Date.now();
  show(await get(), 0);
  // Hidden: another tab in front of it.
  const { targetId } = await c.send("Target.createTarget", { url: "about:blank" });
  await c.send("Target.activateTarget", { targetId });
  await sleep(1000);
  console.log(`page is ${await c.ev("document.visibilityState")}`);
  const hiddenAt = Date.now();
  while (Date.now() - hiddenAt < HIDDEN_MS) show(await get(), Date.now() - start);
  // Visible again.
  const pages = await c.send("Target.getTargets");
  const ours = pages.targetInfos.find((t) => t.type === "page" && t.targetId !== targetId);
  await c.send("Target.activateTarget", { targetId: ours.targetId });
  await sleep(1000);
  show(await get(), Date.now() - start);
  const hidden = rows.filter((r) => r.state === "hidden" && !r.error);
  const visible = rows.filter((r) => r.state === "visible" && !r.error);
  const s = (xs) => xs.map((r) => r.ms / 1000);
  console.log(`summary: ${visible.length} visible gets ${s(visible).map((x) => x.toFixed(1)).join(", ")} s; ${hidden.length} hidden gets ${Math.min(...s(hidden)).toFixed(1)} to ${Math.max(...s(hidden)).toFixed(1)} s; failures ${rows.filter((r) => r.error).length}`);
} finally {
  await c.close();
  server.closeAllConnections();
  server.close();
}
