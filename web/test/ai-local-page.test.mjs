// The desktop app's "Connect your AI app" (web/ai-dialog.js
// openLocalDialog, iteration 58) and the confirmation of its calls in
// headless Chrome, with web/ai-local.js's LocalAiAccess on a fake Tauri
// bridge (no app, no ROM): the setting off by default, turned on (the
// local address, the Cursor and VS Code links and the Claude Code and
// Codex commands with the Authorization header), Add to Claude Desktop,
// the AI apps seen, a busy
// port's error, another port, turned off. A store_variable call opens
// tab-mcp's own confirmation (the vendored dialog); Always allow lets the
// next one run without asking. Skipped without Chrome or the wasm package
// (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

/** The fake bridge and the AI access in the page, at `window.t`. */
const SETUP = `(async () => {
  document.body.replaceChildren();
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = "/shared/components.css";
  document.head.append(link);
  const handlers = new Map();
  const replies = new Map();
  const t = { calls: [], status: { enabled: false, running: false, port: 4842, defaultPort: 4842, address: null, error: null, apps: [] }, busy: null };
  t.tauri = {
    core: { invoke: async (cmd, args) => {
      t.calls.push([cmd, args ?? null]);
      if (cmd === "mcp_status") return { ...t.status };
      if (cmd === "mcp_set") {
        const port = args.port ?? t.status.port;
        if (args.enabled && port === t.busy) {
          t.status = { ...t.status, enabled: true, port, running: false, address: null, error: "Port " + port + " is in use by pid 7 (other). Choose another port." };
          delete t.status.authorization;
        } else {
          t.status = { ...t.status, enabled: args.enabled, port, running: args.enabled, error: null, address: args.enabled ? "http://127.0.0.1:" + port + "/mcp" : null };
          if (args.enabled) t.status.authorization = "Bearer " + "c".repeat(64); else delete t.status.authorization;
        }
        return { ...t.status };
      }
      if (cmd === "mcpb_open") {
        if (t.mcpbError) throw new Error(t.mcpbError);
        return "/cache/saturnus.mcpb";
      }
      if (cmd === "mcp_reply") replies.get(args.id)?.(args);
      return null;
    } },
    event: { listen: async (name, fn) => { handlers.set(name, fn); return () => {}; } },
  };
  t.emit = (name, payload) => handlers.get(name)?.({ payload });
  t.call = (id, params, client, clientStable = true) => new Promise((resolve) => { replies.set(id, resolve); t.emit("mcp-call", { id, method: "tools/call", params, client, clientStable }); });
  const store = { state: { booted: "48gx", romName: "gxrom-r", frame: null, keysDown: [], busy: false } };
  const backend = { memoryTree: async () => ({ path: ["HOME"], dirs: [] }), storeText: async () => ({ ok: true }) };
  const { LocalAiAccess } = await import("/ai-local.js");
  const { openAiDialog } = await import("/ai-dialog.js");
  t.ai = new LocalAiAccess({ calculatorView: async () => ({ ai: async () => ({ backend, store, reference: null }) }), tauri: t.tauri, allowDelayMs: 500 });
  await t.ai.load();
  t.closed = openAiDialog(t.ai);
  window.t = t;
  return true;
})()`;

const DIALOG = `(() => {
  const d = document.querySelector("dialog.ai");
  if (!d) return null;
  const q = (s) => d.querySelector(s);
  const shown = (e) => !!e && !e.closest("[hidden]");
  return {
    on: q("#ai-on").checked,
    status: q(".ai-status").textContent,
    error: q(".ai-status").classList.contains("error"),
    address: shown(q(".ai-address")) ? q(".ai-address").value : null,
    cursor: shown(q(".ai-install-row a")) ? q(".ai-install-row a").href : null,
    apps: [...d.querySelectorAll(".ai-apps li")].map((li) => li.textContent),
    port: q("#ai-port").value,
  };
})()`;

test("the desktop dialog: the setting, the address with its header, the apps, the port", { timeout: 60_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/shared/mcp-install.js` });
  await sleep(300);
  assert.equal(await ev(SETUP), true);
  const until = async (expr, what) => {
    for (let i = 0; i < 200; i++) {
      const v = await ev(expr).catch(() => null);
      if (v) return v;
      await sleep(50);
    }
    assert.fail(`timed out waiting for ${what}`);
  };
  let d = await until(DIALOG, "the dialog");
  assert.equal(d.on, false, "off by default");
  assert.equal(d.status, "AI access is off.");
  assert.equal(d.address, null);
  assert.equal(d.port, "4842");

  await ev(`document.getElementById("ai-on").click(), true`);
  d = await until(`(${DIALOG}) ?.address && (${DIALOG})`, "the address");
  assert.equal(d.address, "http://127.0.0.1:4842/mcp");
  assert.equal(d.status, "AI access is on. Add the address to your AI app.");
  const config = JSON.parse(Buffer.from(decodeURIComponent(new URL(d.cursor).searchParams.get("config")), "base64").toString());
  assert.deepEqual(config, { url: "http://127.0.0.1:4842/mcp", headers: { Authorization: `Bearer ${"c".repeat(64)}` } });
  assert.deepEqual(await ev(`window.t.calls.filter((c) => c[0] === "mcp_set").map((c) => c[1])`), [{ enabled: true }]);
  // Copy for Claude Code: the command with the header (the clipboard refused here, so it shows in the field).
  await ev(`Object.defineProperty(navigator, "clipboard", { value: { writeText: () => Promise.reject(new Error("no")) } }), true`);
  await ev(`[...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Copy for Claude Code").click(), true`);
  const cmd = await until(`document.querySelector("dialog.ai .ai-cmd:not([hidden])")?.value`, "the command");
  assert.equal(cmd, `claude mcp add --transport http saturnus http://127.0.0.1:4842/mcp --header "Authorization: Bearer ${"c".repeat(64)}"`);
  // No token in any attribute of the dialog (titles, labels), only in the links' hrefs.
  const attrs = await ev(`[...document.querySelectorAll("dialog.ai *")].flatMap((e) => [...e.attributes].filter((a) => a.name !== "href").map((a) => a.value)).join(" ")`);
  assert.ok(!attrs.includes("c".repeat(64)));

  // Add to Claude Desktop: the app opens its extension; a failure says why.
  await ev(`[...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Add to Claude Desktop").click(), true`);
  d = await until(`(${DIALOG}).status.includes("Install") && (${DIALOG})`, "the extension opened");
  assert.equal(d.status, "Choose Install in Claude Desktop. If it does not open, open /cache/saturnus.mcpb with it.");
  assert.deepEqual(await ev(`window.t.calls.filter((c) => c[0] === "mcpb_open").length`), 1);
  await ev(`window.t.mcpbError = "Claude Desktop did not open the extension.", [...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Add to Claude Desktop").click(), true`);
  d = await until(`(${DIALOG}).error && (${DIALOG})`, "the extension's error");
  assert.equal(d.status, "Claude Desktop did not open the extension.");
  await ev(`window.t.mcpbError = null, true`);

  // An AI app seen.
  await ev(`window.t.status.apps = [{ name: "Claude Code", secondsAgo: 130 }], window.t.emit("mcp-status", null), true`);
  d = await until(`(${DIALOG}).apps.length && (${DIALOG})`, "the app");
  assert.deepEqual(d.apps, ["Claude Code 2 min ago"]);

  // A busy port, then another.
  await ev(`window.t.busy = 4900, document.getElementById("ai-port").value = "4900", [...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Use this port").click(), true`);
  d = await until(`(${DIALOG}).error && (${DIALOG})`, "the busy port");
  assert.equal(d.status, "Port 4900 is in use by pid 7 (other). Choose another port.");
  assert.equal(d.address, null);
  await ev(`document.getElementById("ai-port").value = "4901", [...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Use this port").click(), true`);
  d = await until(`(${DIALOG}).address?.includes("4901") && (${DIALOG})`, "the other port");
  assert.equal(d.error, false);
  await ev(`document.getElementById("ai-port").value = "80", [...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Use this port").click(), true`);
  d = await until(`(${DIALOG}).error && (${DIALOG})`, "the refused port");
  assert.equal(d.status, "Choose a port from 1024 to 65535.");

  // Turned off: the address goes.
  await ev(`document.getElementById("ai-on").click(), true`);
  d = await until(`!(${DIALOG}).on && (${DIALOG}).address === null && (${DIALOG})`, "off");
  assert.equal(d.status, "AI access is off.");
  await ev(`[...document.querySelectorAll("dialog.ai button")].find((b) => b.textContent === "Done").click(), true`);
  await until(`!document.querySelector("dialog.ai")`, "closed");
});

test("a consequential call asks in tab-mcp's dialog; Always allow lets the next one run", { timeout: 60_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/shared/mcp-install.js` });
  await sleep(300);
  assert.equal(await ev(SETUP), true);
  await ev(`window.t.ai.connect().then(() => true)`);
  await ev(`document.querySelector("dialog.ai")?.close(), true`);
  await ev(`window.t.first = window.t.call(1, { name: "store_variable", arguments: { name: "X", text: "1" } }, "Cursor"), true`);
  const until = async (expr, what) => {
    for (let i = 0; i < 200; i++) {
      const v = await ev(expr).catch(() => null);
      if (v) return v;
      await sleep(50);
    }
    assert.fail(`timed out waiting for ${what}`);
  };
  const title = await until(`document.querySelector("dialog.tab-mcp-confirm h2")?.textContent`, "the confirmation");
  assert.equal(title, "Allow Store a variable?");
  assert.match(await ev(`document.querySelector("dialog.tab-mcp-confirm p").textContent`), /^Cursor \(this computer\) wants to run Store a variable\.$/);
  await sleep(700);
  await ev(`[...document.querySelectorAll("dialog.tab-mcp-confirm button")].find((b) => b.textContent === "Always allow").click(), true`);
  const first = await until(`window.t.first.then((r) => JSON.stringify(r))`, "the first call");
  assert.ok(!JSON.parse(first).result.content[0].text.includes("declined"), first);
  const second = await ev(`window.t.call(2, { name: "store_variable", arguments: { name: "Y", text: "2" } }, "Cursor").then((r) => JSON.stringify(r))`);
  assert.ok(!JSON.parse(second).result.content[0].text.includes("declined"), second);
  assert.equal(await ev(`document.querySelectorAll("dialog.tab-mcp-confirm").length`), 0, "not asked again");
  // The AI's calls has both, done.
  assert.deepEqual(await ev(`window.t.ai.log.entries.filter((e) => e.kind === "call").map((e) => e.outcome)`), ["ok", "ok"]);
  // An AI app named only by its User-Agent: no Always allow offered.
  await ev(`window.t.third = window.t.call(3, { name: "store_variable", arguments: { name: "Z", text: "3" } }, "node", false), true`);
  await until(`document.querySelector("dialog.tab-mcp-confirm h2")?.textContent`, "the confirmation for node");
  const offered = await ev(`[...document.querySelectorAll("dialog.tab-mcp-confirm button")].filter((b) => !b.hidden).map((b) => b.textContent)`);
  assert.deepEqual(offered, ["Don't allow", "Allow"]);
  assert.equal(await ev(`document.querySelector("dialog.tab-mcp-confirm .tab-mcp-always-note").hidden`), true);
  await ev(`[...document.querySelectorAll("dialog.tab-mcp-confirm button")].find((b) => b.textContent === "Don't allow").click(), true`);
  const third = await until(`window.t.third.then((r) => JSON.stringify(r))`, "the third call");
  assert.match(JSON.parse(third).result.content[0].text, /declined/);
});
