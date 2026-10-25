// The browser's "Connect your AI app" (web/ai-dialog.js openAiDialog) in
// headless Chrome, with a fake AI access (no relay, no ROM): while AI
// access is on and no AI app is connected, Done and the backdrop do not
// close it (that would end the pairing code) and a line says so; the
// pairing code with Copy code, in a box that keeps its size whether or
// not a code is there; Copy for Codex with the command that turns on
// Codex's mcp_2026_07_28 first; each Copy with the copy icon; the state
// under the title with its dot; one primary button; Done once an app is
// connected, and when AI access is off; Close anyway while it waits.
// Skipped without Chrome.
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

const ADDRESS = "https://tab-mcp.example/a/abc/mcp";

/** The fake AI access and the dialog, at `window.t`. */
const SETUP = `(async () => {
  document.body.replaceChildren();
  for (const href of ["/shared/tokens.css", "/shared/components.css"]) {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.href = href;
    const loaded = new Promise((resolve) => { link.onload = resolve; });
    document.head.append(link);
    await loaded;
  }
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
  const box = q(".ai-code-value").getBoundingClientRect();
  return {
    open: d.open,
    doneDisabled: button("Done").disabled,
    doneNote: shown(q(".ai-done-note")),
    code: shown(q(".ai-code")) && !q(".ai-code-value").hasAttribute("data-empty") ? q(".ai-code-value").textContent : null,
    copyCodeDisabled: button("Copy code").disabled,
    codeBox: shown(q(".ai-code")) ? [Math.round(box.width), Math.round(box.height)] : null,
    codexNote: [...d.querySelectorAll(".ai-note")].some((p) => shown(p) && p.textContent.includes("mcp_2026_07_28")),
    status: [q(".ai-status").dataset.tone, q(".ai-status").textContent],
    primary: [...d.querySelectorAll("button.primary")].filter((b) => shown(b)).map((b) => b.textContent),
    address: shown(q(".ai-address")) ? q(".ai-address").value : null,
    dim: [...d.querySelectorAll(".ai-step.ai-dim")].length,
    copyDisabled: [...d.querySelectorAll("dialog.ai button")].filter((b) => b.textContent.startsWith("Copy")).map((b) => b.disabled),
    linksDisabled: [...d.querySelectorAll(".ai-install-row a")].map((a) => a.getAttribute("aria-disabled") === "true"),
    replaced: shown(q(".ai-replaced")),
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
  // Its entrance (a scale) settled: the sizes measured below are the ones it keeps.
  await until(ev, `document.querySelector("dialog.ai").getAnimations({ subtree: true }).every((a) => a.playState === "finished")`, "the dialog's entrance");
  return ev;
}

test("the browser dialog: Done waits for an AI app, the code has Copy, Codex gets its feature", { timeout: 60_000 }, async (t) => {
  const ev = await open(t);
  if (!ev) return;
  const click = (text) => ev(`[...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`);

  let d = await ev(DIALOG);
  assert.equal(d.open, true);
  assert.deepEqual([d.doneDisabled, d.doneNote, d.code, d.copyCodeDisabled, d.codexNote], [true, true, null, true, true], "on, no app yet: Done waits, no code yet");
  assert.deepEqual(d.status, ["on", "AI access is on. Your AI app can connect now."]);
  assert.deepEqual(d.primary, ["Done"], "one primary button");
  const emptyBox = d.codeBox;
  assert.ok(emptyBox[0] > 100 && emptyBox[1] >= 40, `the code's box is there before a code is: ${emptyBox}`);
  // A click on the backdrop leaves it open while it waits.
  await ev(`document.querySelector("dialog.ai").click(), true`);
  await sleep(100);
  assert.equal((await ev(DIALOG)).open, true, "the backdrop does not close it");

  // The code, and Copy code.
  await ev(`window.t.ai.mcp.pairingCode = { code: "482913", expiresAt: "2099-01-01T00:00:00Z" }, window.t.ai.changed(), true`);
  d = await until(ev, `(${DIALOG})?.code && (${DIALOG})`, "the code");
  assert.equal(d.code, "482913");
  assert.equal(d.copyCodeDisabled, false);
  assert.deepEqual(d.codeBox, emptyBox, "the box does not move when the code comes");
  assert.equal(await ev(`[...document.querySelectorAll("dialog.ai h3")].map((h) => h.textContent)`).then((h) => h[1]), "2Step 2: When your AI app asks for a code, enter this one");
  // Another code: the same box.
  await ev(`window.t.ai.mcp.pairingCode = { code: "000001", expiresAt: "2099-01-01T00:00:00Z" }, window.t.ai.changed(), true`);
  d = await until(ev, `(${DIALOG})?.code === "000001" && (${DIALOG})`, "the next code");
  assert.deepEqual(d.codeBox, emptyBox, "nor when it changes");
  await ev(`window.t.ai.mcp.pairingCode = { code: "482913", expiresAt: "2099-01-01T00:00:00Z" }, window.t.ai.changed(), true`);
  await until(ev, `(${DIALOG})?.code === "482913"`, "the code again");
  // Every Copy button carries the copy icon.
  const copies = await ev(`[...document.querySelectorAll("dialog.ai button")].filter((b) => b.textContent.startsWith("Copy")).map((b) => [b.textContent, b.querySelector("svg.ic use")?.getAttribute("href") ?? null])`);
  assert.deepEqual(copies, [["Copy", "#ic-copy"], ["Copy for Claude Code", "#ic-copy"], ["Copy for Codex", "#ic-copy"], ["Copy code", "#ic-copy"]]);
  await click("Copy code");
  await until(ev, `window.t.copied.length === 1`, "the code copied");
  assert.deepEqual(await ev(`window.t.copied`), ["482913"]);
  assert.equal(await ev(`document.querySelector("dialog.ai .ai-code button").textContent`), "Copied");

  // Copy for Codex: the feature first, then the address.
  await click("Copy for Codex");
  await until(ev, `window.t.copied.length === 2`, "the Codex commands copied");
  assert.equal(await ev(`window.t.copied[1]`), `codex features enable mcp_2026_07_28\ncodex mcp add saturnus --url ${ADDRESS}`);

  // An app connects: a row with its name and host and Disconnect; Done closes it.
  await ev(`window.t.ai.mcp.grants = [{ grantId: "g1", client: { name: "Codex", host: "chatgpt.com" } }], window.t.ai.mcp.pairingCode = undefined, window.t.ai.changed(), true`);
  d = await until(ev, `(${DIALOG})?.doneDisabled === false && (${DIALOG})`, "Done enabled");
  assert.equal(d.doneNote, false);
  assert.equal(d.code, null, "no code: the box stays, empty");
  assert.deepEqual(d.codeBox, emptyBox);
  assert.deepEqual(await ev(`[...document.querySelectorAll("dialog.ai .ai-apps li")].map((li) => [li.querySelector(".row-title").textContent, li.querySelector(".row-sub").textContent.trim(), li.querySelector("button").getAttribute("aria-label")])`), [["Codex", "chatgpt.com", "Disconnect Codex"]]);
  await click("Done");
  await until(ev, `window.t.done === true`, "closed by Done");
});

test("the browser dialog: lost keeps the steps, dimmed, with the address to read; the status is not said again unchanged", { timeout: 60_000 }, async (t) => {
  const ev = await open(t);
  if (!ev) return;
  // The status's text node stays the same node across renders that change nothing in it.
  await ev(`window.t.node = document.querySelector("dialog.ai .ai-status").firstChild, true`);
  await ev(`window.t.ai.mcp.pairingCode = { code: "482913", expiresAt: "2099-01-01T00:00:00Z" }, window.t.ai.changed(), window.t.ai.changed(), true`);
  await until(ev, `(${DIALOG})?.code === "482913"`, "the code");
  assert.equal(await ev(`document.querySelector("dialog.ai .ai-status").firstChild === window.t.node`), true, "the live region was left alone");
  // New address sits under the address, in the DOM where it is seen; the actions are the buttons alone.
  assert.deepEqual(await ev(`[...document.querySelectorAll("dialog.ai .ai-box > .ai-actions button")].map((b) => b.textContent)`), ["Turn off", "Connect", "Done"]);
  assert.equal(await ev(`document.querySelector("dialog.ai .ai-step .ai-address")?.closest(".ai-step").contains([...document.querySelectorAll("dialog.ai button.link")].find((b) => b.textContent === "New address"))`), true);

  // Lost: the client reconnects by itself; the steps stay with the last address, dimmed and off, the code box empty.
  await ev(`window.t.ai.state = "disconnected", window.t.ai.lost = true, window.t.ai.error = "The connection to the relay broke.", window.t.ai.mcp.address = undefined, window.t.ai.mcp.pairingCode = undefined, window.t.ai.changed(), true`);
  let d = await until(ev, `(${DIALOG})?.status[0] === "lost" && (${DIALOG})`, "lost");
  assert.equal(d.status[1], "The connection to the relay broke. Reconnecting…");
  assert.equal(d.address, ADDRESS, "the address stays to read");
  assert.equal(d.dim, 2);
  assert.equal(d.code, null);
  assert.deepEqual(d.copyDisabled, [true, true, true, true], "every Copy off");
  assert.deepEqual(d.linksDisabled, [true, true]);
  assert.equal(await ev(`document.querySelector("dialog.ai .ai-status").firstChild !== window.t.node`), true, "a changed status is said");
  // Back: everything usable again.
  await ev(`window.t.ai.state = "connected", window.t.ai.lost = false, window.t.ai.error = null, window.t.ai.mcp.address = ${JSON.stringify(ADDRESS)}, window.t.ai.changed(), true`);
  d = await until(ev, `(${DIALOG})?.status[0] === "on" && (${DIALOG})`, "back");
  assert.equal(d.dim, 0);
  assert.deepEqual(d.copyDisabled, [false, false, false, true], "Copy code waits for a code");
  assert.deepEqual(d.linksDisabled, [false, false]);
  // Off: the steps go.
  await ev(`window.t.ai.state = "disconnected", window.t.ai.changed(), true`);
  d = await until(ev, `(${DIALOG})?.status[0] === "off" && (${DIALOG})`, "off");
  assert.equal(d.address, null);
  assert.equal(d.codeBox, null);
});

test("the browser dialog: the address taken by another tab (a sibling: a line; else the warning with New address)", { timeout: 60_000 }, async (t) => {
  const ev = await open(t);
  if (!ev) return;
  await ev(`window.t.ai.state = "disconnected", window.t.ai.replaced = { sibling: true }, window.t.ai.error = "Opened in another tab. Your AI apps use that one now; connect again to use them here.", window.t.ai.changed(), true`);
  let d = await until(ev, `(${DIALOG})?.status[0] === "error" && (${DIALOG})`, "replaced by a sibling");
  assert.match(d.status[1], /^Opened in another tab\./);
  assert.equal(d.replaced, false, "nothing to warn about");
  assert.deepEqual(d.primary, ["Connect"]);
  await ev(`window.t.ai.replaced = { sibling: false }, window.t.ai.error = "Another tab or program took over this page.", window.t.ai.changed(), true`);
  d = await until(ev, `(${DIALOG})?.replaced && (${DIALOG})`, "the warning");
  assert.equal(d.status[1], "Another tab or program took over this page.");
  assert.deepEqual(d.primary, ["New address", "Connect"]);
  // New address takes it back: the page's newAddress, then the warning goes with the state.
  await ev(`window.t.ai.newAddress = async () => { window.t.ai.replaced = null; window.t.ai.error = null; window.t.ai.state = "connected"; window.t.ai.changed(); }, true`);
  await ev(`document.querySelector("dialog.ai .ai-replaced button").click(), true`);
  d = await until(ev, `(${DIALOG})?.status[0] === "on" && (${DIALOG})`, "connected again");
  assert.equal(d.replaced, false);
});

test("the browser dialog: with AI access off, Done closes it", { timeout: 60_000 }, async (t) => {
  const ev = await open(t);
  if (!ev) return;
  assert.equal((await ev(DIALOG)).doneDisabled, true);
  await ev(`[...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Turn off").click(), true`);
  const d = await until(ev, `(${DIALOG})?.doneDisabled === false && !(${DIALOG}).doneNote && (${DIALOG})`, "Done enabled with AI access off");
  assert.deepEqual(d.status, ["off", "AI access is off."]);
  assert.equal(d.codeBox, null, "the steps go with AI access off");
  assert.deepEqual(d.primary, ["Connect"], "Connect is the primary button, Done plain");
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
