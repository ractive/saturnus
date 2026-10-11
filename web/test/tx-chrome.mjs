// The transfer page in headless Chrome, for its tests (not a test
// itself): open web/tx.html, wait for conditions, answer its questions,
// and the page test's session, which web/test/tx-record.mjs records
// against the emulated 48GX (web/test/fixtures/tx-page.trace) and
// web/test/tx-page.test.mjs replays with a FakeLink: the same steps in the
// same order, so the page writes exactly the recorded bytes.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { WEB, sleep } from "./chrome.mjs";

/** The device seed of the recorded page session (its sync markers). */
export const PAGE_SEED = 0x7e57_0037;

/** Whether both wasm packages are built. */
export const txBuilt = () => existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm")) && existsSync(join(WEB, "tx", "pkg", "saturnus_tx_web_bg.wasm"));

/** Helpers over a Chrome session (`chrome.mjs`'s `session`). */
export function txPage(c) {
  const { ev } = c;
  /** Poll `expression` until it is true; fail after `ms` naming `what`. */
  const until = async (expression, ms = 30_000, what = expression) => {
    const end = Date.now() + ms;
    for (;;) {
      if (await ev(expression).catch(() => false)) return;
      if (Date.now() > end) {
        const status = await ev(`document.getElementById("pane-status")?.textContent`).catch(() => "?");
        assert.fail(`${what}: not true within ${ms} ms (status: ${status}; errors: ${c.errors.join(" | ")})`);
      }
      await sleep(100);
    }
  };
  const open = async (port, query = "") => {
    await c.send("Page.navigate", { url: `http://127.0.0.1:${port}/tx.html${query}` });
    await until("!!window.tx && !!document.querySelector('svg.icon-sprite')", 30_000, "the page started");
  };
  /** Answer the open question (confirm.js) with its action, after checking its title. */
  const answer = async (title) => {
    await until(`!!document.querySelector("dialog.confirm[open]")`, 30_000, `the question "${title}"`);
    assert.equal(await ev(`document.querySelector("dialog.confirm h2").textContent`), title);
    await ev(`document.querySelector("dialog.confirm .confirm-actions button:last-child").click()`);
  };
  const idle = (ms = 60_000, what = "the page is idle") => until("tx.idle()", ms, what);
  const names = () => ev("tx.names()");
  return { until, open, answer, idle, names };
}

/** The files of the page session. */
export const PAGE_FILES = {
  program: "%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \\>>\r\n",
  program2: "%%HP: T(3)A(R)F(.);\r\n\\<< 3 4 * \\>>\r\n",
  note: "hello, calculator",
};

/**
 * The page session, once connected: HOME listed; a text file put (it
 * compiles to a program), a plain file put as a string after "Store it
 * as a string?", the text file again after "Replace PRG?"; NOTE selected
 * and previewed; PRG got as a download; disconnected. Returns what it saw.
 */
export async function pageSession(c, { ms = 60_000 } = {}) {
  const { ev } = c;
  const p = txPage(c);
  const put = (text, name) => ev(`tx.putFiles([new File([${JSON.stringify(text)}], ${JSON.stringify(name)})]).then(() => true)`);
  await p.idle(ms, "HOME listed after connecting");
  const seen = { home: await p.names() };
  await put(PAGE_FILES.program, "PRG.txt");
  await p.idle(ms, "PRG put");
  const note = put(PAGE_FILES.note, "NOTE.md");
  await p.answer("Store it as a string?");
  await note;
  await p.idle(ms, "NOTE put");
  const again = put(PAGE_FILES.program2, "PRG.txt");
  await p.answer("Replace PRG?");
  await again;
  await p.idle(ms, "PRG replaced");
  seen.after = await p.names();
  await ev(`tx.select("NOTE")`);
  await p.until(`!!document.querySelector("#preview pre.obj-string")`, ms, "NOTE previewed");
  seen.preview = await ev(`document.querySelector("#preview pre.obj-string").textContent`);
  await p.idle(ms, "the preview read");
  await ev(`tx.getVariable(tx.ui.list.listing.entries.find((e) => e.name === "PRG"))`);
  seen.download = await ev(`({ name: tx.state.lastDownload.name, bytes: [...tx.state.lastDownload.bytes] })`);
  await ev(`tx.disconnect()`);
  await p.until(`!tx.state.device`, ms, "disconnected");
  seen.note = await ev(`document.getElementById("last-note").textContent`);
  return seen;
}
