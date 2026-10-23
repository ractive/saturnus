// AI access's sidebar entry and "The AI's calls" (iteration 56) in the
// real page in headless Chrome, without a relay or a ROM: the page is
// pointed at a development relay nobody runs (localStorage
// `saturnus.aiRelay` on 127.0.0.1), and the states and the client's
// events are played into its `AiAccess` (web/ai.js). The entry says the
// state (off, connected with its dot, lost with the warning dot) with
// Calls and Disconnect or Turn off below it and a live region; Calls opens
// the drawer, which shows the calls as they start and end ("running",
// "waiting for you" while tab-mcp's confirmation asks about the call,
// then the outcome and the duration, a long result folded), the
// connection's events, Clear, and stays open across a reload (empty: the
// calls are never stored). The palette offers it. Skipped without Chrome
// or the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

async function page(t) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 860, deviceScaleFactor: 1, mobile: false });
  const until = async (expr, what, ms = 10_000) => {
    for (let i = 0; i < ms / 50; i++) {
      const v = await ev(expr).catch(() => null);
      if (v) return v;
      await sleep(50);
    }
    assert.fail(`timed out waiting for ${what}`);
  };
  const load = async () => {
    await until("!!window.saturnus && !!window.saturnusApp", "the page");
    await ev("window.saturnus.started.then(() => true)");
    await until("!document.getElementById('ai-connect').hidden", "the AI access entry");
    // The client's core is loaded with the entry: its TabMcp is there, disconnected.
    await until("window.saturnusApp.ai.then((a) => !!a.mcp)", "the client loaded");
  };
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  await until("!!window.saturnus", "the page");
  // Port 9 (discard): nothing answers there, nothing is connected.
  await ev(`localStorage.setItem("saturnus.aiRelay", "http://127.0.0.1:9"), true`);
  await send("Page.reload");
  await sleep(300);
  await load();
  return { ...c, until, load };
}

/** The entry, the line below it and the live region. */
const SIDEBAR = `(() => {
  const b = document.getElementById("ai-connect"), chip = document.getElementById("ai-chip"), dot = b.querySelector(".ai-dot");
  return { entry: b.querySelector(".ai-label").textContent, dot: dot.hidden ? null : getComputedStyle(dot).backgroundColor,
    line: chip.hidden ? null : chip.textContent, live: document.getElementById("ai-live").textContent };
})()`;

/** Play a state into AI access: the client's state and apps, then `change`. */
const STATE = (state, grants, lost = false) => `window.saturnusApp.ai.then((a) => {
  // Lost: the client reconnecting by itself (its \`reconnecting\`), until Turn off stops it.
  a.mcp = { state: ${JSON.stringify(state)}, grants: ${JSON.stringify(grants)}, reconnecting: ${lost ? '{ reason: "shutdown", attempt: 1, at: Date.now() + 5000 }' : "undefined"}, disconnect() { this.reconnecting = undefined; }, startPairing() {}, stopPairing() {} };
  a.error = ${lost ? JSON.stringify("The relay restarted.") : "null"};
  a.changed();
  return true;
})`;

/** An event of the client, as AI access hands it on. */
const EMIT = (type, detail) => `window.saturnusApp.ai.then((a) => (a.dispatchEvent(new CustomEvent(${JSON.stringify(type)}, { detail: ${JSON.stringify(detail)} })), true))`;

/** The drawer's rows: their text, and the outcome cell's. */
const ROWS = `[...document.querySelectorAll("#ai-log .al-entry")].map((li) => ({ text: li.textContent, out: li.querySelector(".al-out")?.textContent ?? null }))`;

const CLAUDE = { grantId: "g1", id: "https://claude.ai/cimd.json", name: "Claude", host: "claude.ai" };

test("the sidebar says AI access's state; Calls opens the AI's calls, which follow each call to its end", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const { ev, until, send } = p;

  assert.deepEqual(await ev(SIDEBAR), { entry: "Connect your AI app…", dot: null, line: null, live: "" }, "off");
  assert.equal(await ev("document.getElementById('ai-log').hidden"), true, "the drawer is closed by default");

  await ev(STATE("connected", []));
  assert.deepEqual(await ev(SIDEBAR), { entry: "AI access on · no app yet", dot: null, line: "Show", live: "AI access on. No app connected yet." });

  await ev(STATE("connected", [{ grantId: "g1", client: { id: CLAUDE.id, name: "Claude", host: "claude.ai" } }]));
  const on = await ev(SIDEBAR);
  assert.equal(on.entry, "Claude is connected");
  assert.equal(on.line, "Calls · Disconnect");
  assert.equal(on.live, "Claude is connected.");
  const accent = await ev("(() => { const d = document.createElement('i'); d.style.color = 'var(--accent)'; document.getElementById('panel').append(d); const c = getComputedStyle(d).color; d.remove(); return c; })()");
  assert.equal(on.dot, accent, "the dot in the accent colour");
  assert.equal(await ev("document.getElementById('ai-live').getAttribute('aria-live')"), "polite");

  // Calls opens the drawer, kept open in this browser.
  await ev("[...document.querySelectorAll('#ai-chip button')].find((b) => b.textContent === 'Calls').click(), true");
  await until("!document.getElementById('ai-log').hidden", "the drawer open");
  assert.equal(await ev("localStorage.getItem('saturnus.aiLog')"), "open");
  assert.equal(await ev("document.querySelector('#ai-log h2').textContent"), "The AI's calls");
  assert.equal(await ev("document.querySelector('#ai-log .al-empty').hidden"), false, "nothing yet");
  // Beside the calculator, not over it: the stage keeps the drawer's part free, the calculator fits in the rest.
  const place = `(() => {
    const st = document.getElementById("stage"), cs = getComputedStyle(st), d = document.getElementById("ai-log").getBoundingClientRect();
    const r = st.getBoundingClientRect(), calc = document.querySelector("sat-calculator").getBoundingClientRect();
    const skin = [...document.querySelectorAll("sat-calculator svg, sat-calculator .no-rom, sat-calculator [class*=norom]")].map((e) => e.getBoundingClientRect()).filter((x) => x.width);
    return { beside: document.getElementById("calculator-view").classList.contains("ai-log-beside"), contentRight: r.right - parseFloat(cs.paddingRight),
      left: d.left, right: d.right, top: d.top, bottom: d.bottom, w: innerWidth, h: innerHeight, skinRight: Math.max(0, ...skin.map((x) => x.right)), calcRight: calc.right };
  })()`;
  let at = await ev(place);
  assert.equal(at.beside, true);
  assert.ok(at.left >= at.contentRight, JSON.stringify(at));
  assert.ok(at.skinRight <= at.left, `nothing of the calculator under the drawer: ${JSON.stringify(at)}`);
  // A phone: a sheet along the bottom.
  await send("Emulation.setDeviceMetricsOverride", { width: 390, height: 800, deviceScaleFactor: 1, mobile: true });
  await until(`!document.getElementById("calculator-view").classList.contains("ai-log-beside")`, "the sheet on a phone");
  at = await ev(place);
  assert.deepEqual([at.left, at.right, Math.round(at.bottom)], [0, 390, 800], JSON.stringify(at));
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 860, deviceScaleFactor: 1, mobile: false });
  await until(`document.getElementById("calculator-view").classList.contains("ai-log-beside")`, "beside again");

  // A call that asks first: running, waiting for you while tab-mcp's confirmation asks about it, then its outcome.
  await ev(EMIT("note", { what: "connected", host: "127.0.0.1:9" }));
  await ev(EMIT("toolcall", { id: 1, tool: "store_variable", title: "Store a variable", arguments: { name: "AIT", text: "« 2 * »" }, client: CLAUDE }));
  let rows = await ev(ROWS);
  assert.equal(rows.length, 2);
  assert.match(rows[0].text, /Connected to the relay 127\.0\.0\.1:9/);
  assert.match(rows[1].text, /ClaudeStore a variable/);
  assert.match(rows[1].text, /name: AIT/);
  assert.match(rows[1].text, /text: « 2 \* »/);
  assert.equal(rows[1].out, "running", "store_variable asks first, but no confirmation is open");
  await ev(`(() => { const d = document.createElement("dialog"); d.className = "tab-mcp-dialog tab-mcp-confirm"; const h = document.createElement("h2"); h.textContent = "Allow Store a variable?"; d.append(h); document.body.append(d); return true; })()`);
  await until(`document.querySelector('#ai-log .al-call .al-out').textContent === 'waiting for you'`, "waiting for you");
  await ev("document.querySelector('dialog.tab-mcp-confirm').remove(), true");
  await until(`document.querySelector('#ai-log .al-call .al-out').textContent === 'running'`, "running again");
  const result = Array.from({ length: 12 }, (_, i) => `line ${i + 1}`).join("\n");
  await ev(EMIT("toolcallend", { id: 1, tool: "store_variable", client: CLAUDE, outcome: "ok", result: { content: [{ type: "text", text: result }] }, durationMs: 1234 }));
  rows = await ev(ROWS);
  assert.equal(rows[1].out, "done · 1.2 s");
  // The result's first lines; the rest behind "9 more lines".
  assert.equal(await ev("document.querySelector('#ai-log .al-result .al-head').textContent"), "line 1\nline 2\nline 3");
  assert.equal(await ev("document.querySelector('#ai-log .al-result .al-more').textContent"), "9 more lines");
  await ev("document.querySelector('#ai-log .al-result .al-more').click(), true");
  assert.equal(await ev("document.querySelector('#ai-log .al-result').textContent"), result);

  // A declined call and a failed one; then the connection lost.
  await ev(EMIT("toolcall", { id: 2, tool: "purge_variable", title: "Purge a variable", arguments: { name: "AIT" }, client: CLAUDE }));
  await ev(EMIT("toolcallend", { id: 2, tool: "purge_variable", client: CLAUDE, outcome: "declined", result: { content: [{ type: "text", text: "declined" }], isError: true }, durationMs: 3000 }));
  await ev(EMIT("toolcall", { id: 3, tool: "run_rpl", title: "Run RPL", arguments: { text: "1 0 /" }, client: CLAUDE }));
  await ev(EMIT("toolcallend", { id: 3, tool: "run_rpl", client: CLAUDE, outcome: "error", result: { content: [{ type: "text", text: "Infinite Result" }], isError: true }, durationMs: 80 }));
  await ev(EMIT("note", { what: "lost", reason: "shutdown", text: "The relay restarted." }));
  await ev(EMIT("note", { what: "wait", seconds: 5 }));
  rows = await ev(ROWS);
  assert.deepEqual(rows.slice(2).map((r) => r.out), ["declined · 3.0 s", "error · 80 ms", null, null]);
  assert.match(rows[4].text, /Connection lost\. The relay restarted\./);
  assert.match(rows[5].text, /Next try in 5 s/);
  assert.equal(await ev("document.querySelectorAll('#ai-log .al-entry.bad').length"), 2, "the error and the loss in red");

  // Lost: the warning dot, Calls and Turn off; Turn off ends it.
  await ev(STATE("disconnected", [], true));
  const lost = await ev(SIDEBAR);
  assert.equal(lost.entry, "AI access lost · reconnecting…");
  assert.equal(lost.line, "Calls · Turn off");
  assert.notEqual(lost.dot, accent, "the warning colour");
  await ev("[...document.querySelectorAll('#ai-chip button')].find((b) => b.textContent === 'Turn off').click(), true");
  assert.equal((await ev(SIDEBAR)).entry, "Connect your AI app…");
  assert.match((await ev(ROWS)).at(-1).text, /AI access turned off/);

  // Clear; a reload keeps the drawer open, the calls are gone.
  await ev("document.querySelector('#ai-log .al-clear').click(), true");
  assert.equal((await ev(ROWS)).length, 0);
  await ev(EMIT("note", { what: "off" }));
  await send("Page.reload");
  await sleep(300);
  await p.load();
  await until("!document.getElementById('ai-log').hidden", "the drawer open again after the reload");
  assert.equal((await ev(ROWS)).length, 0, "nothing kept");

  // The close button closes it, and the browser remembers.
  await ev("document.querySelector('#ai-log .al-close').click(), true");
  assert.equal(await ev("document.getElementById('ai-log').hidden"), true);
  assert.equal(await ev("localStorage.getItem('saturnus.aiLog')"), null);
});

test("the palette offers the AI's calls", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const { ev, until } = p;
  // Search for it as a person would, then run it.
  await ev("document.getElementById('palette-show').click(), true");
  await until("document.querySelector('sat-palette input')", "the palette");
  await ev(`(() => { const i = document.querySelector('sat-palette input'); i.value = "AI's calls"; i.dispatchEvent(new Event('input', { bubbles: true })); return true; })()`);
  await until(`[...document.querySelectorAll('sat-palette [role=option]')].some((o) => o.textContent.includes("Show the AI's calls"))`, "the entry");
  await ev(`[...document.querySelectorAll('sat-palette [role=option]')].find((o) => o.textContent.includes("Show the AI's calls")).click(), true`);
  await until("!document.getElementById('ai-log').hidden", "the drawer open from the palette");
});
