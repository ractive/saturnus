// The browser's "Connect your AI app" (web/ai-dialog.js openAiDialog) in
// headless Chrome, with a fake AI access (no relay, no ROM): while AI
// access is on and no AI app is connected, Done and the backdrop do not
// close it (that would end the pairing code) and a line says so; the
// pairing code with Copy code; Copy for Codex with the command that turns
// on Codex's mcp_2026_07_28 first; Done once an app is connected, and
// when AI access is off; Close anyway while it waits. Skipped without
// Chrome.
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

const ADDRESS = "https://tab-mcp.example/a/abc/mcp";

/** The fake AI access and the dialog, at `window.t`. */
const SETUP = `(async () => {
  document.body.replaceChildren();
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = "/shared/components.css";
  document.head.append(link);
  const t = { copied: [] };
  Object.defineProperty(navigator, "clipboard", { value: { writeText: async (text) => { t.copied.push(text); } } });
  class FakeAi extends EventTarget {
    state = "connected";
    lost = false;
    error = null;
    keepOn = false;
    local = false;
    mcp = { address: ${JSON.stringify(ADDRESS)}, pairingCode: undefined, grants: [], startPairing() {}, stopPairing() {} };
    changed() { this.dispatchEvent(new Event("change")); }
    async connect() { this.state = "connected"; this.changed(); }
    disconnect() { this.state = "disconnected"; this.changed(); }
    revoke() { return true; }
    async newAddress() {}
  }
  const { openAiDialog } = await import("/ai-dialog.js");
  t.ai = new FakeAi();
  t.closed = openAiDialog(t.ai).then(() => { t.done = true; });
  window.t = t;
  return true;
})()`;

const DIALOG = `(() => {
  const d = document.querySelector("dialog.ai");
  if (!d) return null;
  const q = (s) => d.querySelector(s);
  const shown = (e) => !!e && !e.closest("[hidden]");
  const button = (text) => [...d.querySelectorAll("button")].find((b) => b.textContent === text);
  return {
    open: d.open,
    doneDisabled: button("Done").disabled,
    doneNote: shown(q(".ai-done-note")),
    code: shown(q(".ai-code")) ? q(".ai-code").textContent : null,
    codexNote: [...d.querySelectorAll(".ai-note")].some((p) => shown(p) && p.textContent.includes("mcp_2026_07_28")),
  };
})()`;

/** Polls `expr` in the page until it is truthy (10 s). */
async function until(ev, expr, what) {
  for (let i = 0; i < 200; i++) {
    const v = await ev(expr).catch(() => null);
    if (v) return v;
    await sleep(50);
  }
  assert.fail(`timed out waiting for ${what}`);
}

/** A page with the dialog open on the fake AI access. */
async function open(t) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/shared/mcp-install.js` });
  await sleep(300);
  assert.equal(await ev(SETUP), true);
  await until(ev, DIALOG, "the dialog");
  return ev;
}

test("the browser dialog: Done waits for an AI app, the code has Copy, Codex gets its feature", { timeout: 60_000 }, async (t) => {
  const ev = await open(t);
  if (!ev) return;
  const click = (text) => ev(`[...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`);

  let d = await ev(DIALOG);
  assert.deepEqual(d, { open: true, doneDisabled: true, doneNote: true, code: null, codexNote: true }, "on, no app yet: Done waits");
  // A click on the backdrop leaves it open while it waits.
  await ev(`document.querySelector("dialog.ai").click(), true`);
  await sleep(100);
  assert.equal((await ev(DIALOG)).open, true, "the backdrop does not close it");

  // The code, and Copy code.
  await ev(`window.t.ai.mcp.pairingCode = { code: "482913", expiresAt: "2099-01-01T00:00:00Z" }, window.t.ai.changed(), true`);
  d = await until(ev, `(${DIALOG})?.code && (${DIALOG})`, "the code");
  assert.match(d.code, /When your AI app asks for a code, enter 482913\./);
  await click("Copy code");
  await until(ev, `window.t.copied.length === 1`, "the code copied");
  assert.deepEqual(await ev(`window.t.copied`), ["482913"]);
  assert.equal(await ev(`document.querySelector("dialog.ai .ai-code button").textContent`), "Copied");

  // Copy for Codex: the feature first, then the address.
  await click("Copy for Codex");
  await until(ev, `window.t.copied.length === 2`, "the Codex commands copied");
  assert.equal(await ev(`window.t.copied[1]`), `codex features enable mcp_2026_07_28\ncodex mcp add saturnus --url ${ADDRESS}`);

  // An app connects: Done closes it.
  await ev(`window.t.ai.mcp.grants = [{ grantId: "g1", client: { name: "Codex", host: "chatgpt.com" } }], window.t.ai.mcp.pairingCode = undefined, window.t.ai.changed(), true`);
  d = await until(ev, `(${DIALOG})?.doneDisabled === false && (${DIALOG})`, "Done enabled");
  assert.equal(d.doneNote, false);
  await click("Done");
  await until(ev, `window.t.done === true`, "closed by Done");
});

test("the browser dialog: with AI access off, Done closes it", { timeout: 60_000 }, async (t) => {
  const ev = await open(t);
  if (!ev) return;
  assert.equal((await ev(DIALOG)).doneDisabled, true);
  await ev(`[...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Turn off").click(), true`);
  await until(ev, `(${DIALOG})?.doneDisabled === false && !(${DIALOG}).doneNote`, "Done enabled with AI access off");
  await ev(`[...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Done").click(), true`);
  await until(ev, `window.t.done === true`, "closed by Done");
});

test("the browser dialog: Close anyway closes it while it waits (a phone has no Esc)", { timeout: 60_000 }, async (t) => {
  const ev = await open(t);
  if (!ev) return;
  assert.equal((await ev(DIALOG)).doneNote, true);
  await ev(`[...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Close anyway").click(), true`);
  await until(ev, `window.t.done === true`, "closed by Close anyway");
});
