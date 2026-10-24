// "Connect your AI app" (iteration 55): the dialog of AI access
// (web/ai.js) in the page's own look (shared/components.css, `dialog.ai`):
// the MCP address with Copy, links and commands that add it to the
// common AI apps (shared/mcp-install.js), the state, the pairing code
// once the relay sends one (tab-mcp's OAuth, its iteration 3), the AI
// apps connected with Disconnect each, "Keep AI access on", and Connect, Turn off or New
// address. Opened from Search and from Calculator's panel; one at a time.
// While it is open the relay is asked for pairing codes (tab-mcp
// `startPairing`).
//
// The desktop app's own (iteration 58, `openLocalDialog`, for web/ai-local.js):
// the setting "Let AI apps on this computer use the calculators", the local
// address with Copy, the same links and commands with the Authorization
// header (shared/mcp-install.js), the state (off, on, the AI apps seen
// lately), the port, and no pairing code.

import { iconEl } from "./shared/components/icons.js";
import { el } from "./shared/dom.js";
import { externalLink } from "./shared/ext-links.js";
import { CODEX_FEATURE, installTargets } from "./shared/mcp-install.js";

const COPY_FAILED = "Press ⌘C or Ctrl+C";

/** A Copy button: the copy icon, then `label` (in a span copyText changes). */
function copyButton(label, cls = null) {
  return el("button", { type: "button", class: cls }, iconEl("copy", "ic-sm"), el("span", { text: label }));
}

/**
 * Copy `text` from a copyButton's click: "Copied" on it for two seconds,
 * or, when the browser refuses, `select()` (which shows and selects the
 * text) and the keys to press.
 */
async function copyText(button, label, text, select) {
  const words = button.querySelector("span");
  try {
    await navigator.clipboard.writeText(text);
    words.textContent = "Copied";
  } catch {
    select();
    words.textContent = COPY_FAILED;
  }
  setTimeout(() => { words.textContent = label; }, 2000);
}

/**
 * Open the dialog for `ai` (an `AiAccess`), from a click: connects first
 * when not connected (the click is the user's choice). Resolves when it
 * closes.
 */
export async function openAiDialog(ai, doc = document) {
  if (ai.local) return openLocalDialog(ai, doc);
  doc.querySelector("dialog.ai")?.close();
  const opener = doc.activeElement;
  const dialog = el("dialog", { class: "ai", "aria-labelledby": "ai-title" });
  const status = el("p", { class: "ai-status", role: "status" });
  const address = el("input", { class: "input ai-address", type: "text", readonly: true, "aria-label": "MCP address", spellcheck: "false" });
  const copy = copyButton("Copy");
  const addressRow = el("div", { class: "ai-address-row" }, address, copy);
  const block = installBlock({ newOnly: true });
  const install = block.node;
  const codeText = el("span");
  const codeValue = el("strong", { class: "ai-code-value" });
  const copyCode = copyButton("Copy code", "small");
  const code = el("p", { class: "ai-code", hidden: true }, codeText, " ", copyCode);
  const apps = el("ul", { class: "ai-apps" });
  const appsNote = el("p", { class: "ai-note" });
  const keep = el("input", { type: "checkbox", id: "ai-keep" });
  const keepRow = el("label", { class: "switch ai-keep" }, keep, el("span", {}, "Keep AI access on", el("small", { text: "Connect again when the app opens." })));
  const connect = el("button", { type: "button", class: "primary", text: "Connect" });
  const off = el("button", { type: "button", text: "Turn off" });
  const fresh = el("button", { type: "button", class: "link", text: "New address" });
  const done = el("button", { type: "button", text: "Done" });
  const closeAnyway = el("button", { type: "button", class: "link", text: "Close anyway" });
  const doneNote = el("p", { class: "ai-note ai-done-note", hidden: true }, "Done works once your AI app is connected: closing now ends the pairing code. ", closeAnyway);
  dialog.append(el("div", { class: "ai-box" },
    el("h2", { id: "ai-title", text: "Connect your AI app" }),
    el("p", { text: "Your AI app (Claude, ChatGPT, Claude Code, …) can use the calculators in this tab: read the display, press keys, run programs and look up commands. Add this address to it as an MCP server." }),
    addressRow,
    install,
    code,
    status,
    el("h3", { text: "Connected apps" }),
    apps,
    appsNote,
    el("p", { class: "ai-note", text: "The AI app can press any key and run any command, as you can. Storing or purging a variable through its store and purge tools asks you here first. While an AI app is connected, the calculators keep running when this tab is in the background." }),
    el("p", { class: "ai-note ai-phone", hidden: !matchMedia("(pointer: coarse) and (max-width: 999px)").matches, text: "On a phone, keep this tab in front while your AI app uses it: the phone pauses tabs in the background. Open both apps in split screen, or use your AI app on another device." }),
    el("p", { class: "ai-note" }, "What passes through the relay, and what it keeps: ", externalLink("https://ractive.ch/tab-mcp/", "privacy notice"), "."),
    keepRow,
    el("div", { class: "ai-actions" }, fresh, el("span", { class: "ai-gap" }), off, connect, done),
    doneNote,
  ));

  // AI access is on but no AI app is connected yet: closing would end the
  // pairing code the app is about to ask for, so Done and the backdrop
  // wait; Close anyway (and Esc) close on purpose.
  let waiting = false;
  const render = () => {
    const state = ai.state;
    const mcp = ai.mcp;
    address.value = state === "connected" ? mcp.address : "";
    addressRow.hidden = state !== "connected";
    install.hidden = state !== "connected";
    if (state === "connected") block.set({ url: mcp.address, headers: mcp.headers ?? {} });
    const pc = mcp?.pairingCode;
    code.hidden = !pc;
    if (pc && codeValue.textContent !== pc.code) {
      codeValue.textContent = pc.code;
      codeText.replaceChildren("When your AI app asks for a code, enter ", codeValue, ".");
      copyCode.querySelector("span").textContent = "Copy code";
    }
    // Lost: why, and that it connects again by itself (the client library reconnects).
    status.classList.toggle("error", Boolean(ai.error) && !ai.lost);
    status.textContent = ai.lost
      ? `${ai.error ?? "The connection to the relay broke."} Reconnecting…`
      : ai.error ?? (state === "connected" ? "AI access is on. Your AI app can connect now." : state === "connecting" ? "Connecting…" : "AI access is off.");
    const grants = state === "connected" ? mcp.grants : [];
    apps.replaceChildren(...grants.map((g) => {
      const b = el("button", { type: "button", class: "small", text: "Disconnect" });
      b.addEventListener("click", () => {
        if (!ai.revoke(g.grantId)) {
          ai.error = "Connect again first: only a connected tab can disconnect an app.";
          render();
        }
      });
      return el("li", {}, el("span", { class: "ai-app" }, el("strong", { text: g.client.name || g.client.host }), g.client.host ? el("small", { text: ` ${g.client.host}` }) : null), b);
    }));
    appsNote.textContent = grants.length ? "" : "No AI app is connected yet.";
    appsNote.hidden = grants.length > 0;
    keep.checked = ai.keepOn;
    connect.hidden = state !== "disconnected";
    off.hidden = state === "disconnected" && !ai.lost;
    fresh.hidden = state !== "connected";
    waiting = state === "connected" && grants.length === 0;
    done.disabled = waiting;
    doneNote.hidden = !waiting;
  };

  copy.addEventListener("click", () => copyText(copy, "Copy", address.value, () => address.select()));
  connect.addEventListener("click", () => {
    ai.connect().catch(() => {});
    render();
  });
  off.addEventListener("click", () => {
    ai.disconnect();
    render();
  });
  fresh.addEventListener("click", () => {
    ai.newAddress()?.catch(() => {});
    render();
  });
  keep.addEventListener("change", () => {
    ai.keepOn = keep.checked;
  });
  copyCode.addEventListener("click", () => copyText(copyCode, "Copy code", codeValue.textContent, () => doc.getSelection()?.selectAllChildren(codeValue)));
  done.addEventListener("click", () => dialog.close());
  closeAnyway.addEventListener("click", () => dialog.close());
  // A click on the backdrop (the dialog itself, outside its box) closes it,
  // unless that would end the code an AI app is about to ask for.
  dialog.addEventListener("click", (e) => {
    if (e.target === dialog && !waiting) dialog.close();
  });

  ai.addEventListener("change", render);
  doc.body.append(dialog);
  render();
  dialog.showModal();
  const closed = new Promise((resolve) => dialog.addEventListener("close", resolve, { once: true }));
  if (ai.state === "disconnected" && !ai.lost) {
    try {
      await ai.connect();
    } catch {
      // The dialog shows why.
    }
  }
  ai.mcp?.startPairing();
  render();
  await closed;
  ai.mcp?.stopPairing();
  ai.removeEventListener("change", render);
  dialog.remove();
  if (opener && opener !== doc.body && opener.isConnected && typeof opener.focus === "function") opener.focus();
}

/**
 * The links and commands that add an MCP address to the common AI apps
 * (shared/mcp-install.js): links that open Cursor and VS Code, commands
 * to copy for Claude Code and Codex. A command shows in its field only
 * when the browser refuses the clipboard, selected for ⌘C. `set({url,
 * headers})` builds them again when either changed; `connector` replaces
 * the line about Claude and ChatGPT. `newOnly`: the server speaks only MCP
 * 2026-07-28 (the relay), so Codex's command also turns on its feature
 * for it, and a line says so.
 */
export function installBlock({ connector: connectorText = null, newOnly = false } = {}) {
  const cursor = el("a", { class: "button small", text: "Add to Cursor" });
  const vscode = el("a", { class: "button small", text: "Add to VS Code" });
  const forClaude = copyButton("Copy for Claude Code", "small");
  const forCodex = copyButton("Copy for Codex", "small");
  const cmd = el("textarea", { class: "input ai-cmd", readonly: true, rows: 1, hidden: true, "aria-label": "Command", spellcheck: "false" });
  const connector = el("p", { class: "ai-note" });
  const codexNote = el("p", { class: "ai-note", hidden: !newOnly });
  codexNote.append("Copy for Codex also turns on Codex's feature ", el("code", { text: CODEX_FEATURE }), ", which this address needs. It applies to all of Codex's MCP servers; ", el("code", { text: `codex features disable ${CODEX_FEATURE}` }), " turns it off. Start a new chat in Codex once it is connected.");
  const node = el("div", { class: "ai-install" },
    el("div", { class: "ai-install-row" }, cursor, vscode, forClaude, forCodex),
    cmd,
    codexNote,
    connector);
  let targets = [];
  let built = null;
  const set = ({ url, headers = {} }) => {
    // The desktop app's local address comes with headers (a token); the relay's has none.
    const key = JSON.stringify([url, headers]);
    if (built === key) return;
    built = key;
    targets = installTargets({ name: "saturnus", url, headers, newOnly });
    const t = Object.fromEntries(targets.map((x) => [x.id, x]));
    cursor.href = t.cursor.href;
    vscode.href = t.vscode.href;
    // Fixed words: the command may carry a token, never in an attribute.
    forCodex.title = t.codex.kind === "config" ? "Copies the lines for ~/.codex/config.toml" : newOnly ? "Copies the codex features enable and codex mcp add commands" : "Copies the codex mcp add command";
    forClaude.title = "Copies the claude mcp add command";
    connector.textContent = connectorText ?? t.connector.text;
    cmd.hidden = true;
    cmd.value = "";
  };
  const copyFor = (button, label, id) => button.addEventListener("click", () => {
    const text = targets.find((x) => x.id === id)?.text ?? "";
    // The field shows a command only after a refused copy, never an older one.
    cmd.hidden = true;
    cmd.value = "";
    copyText(button, label, text, () => {
      cmd.value = text;
      cmd.rows = text.split("\n").length;
      cmd.hidden = false;
      cmd.select();
    });
  });
  copyFor(forClaude, "Copy for Claude Code", "claude-code");
  copyFor(forCodex, "Copy for Codex", "codex");
  return { node, set, cursor, vscode, forClaude, forCodex, cmd };
}

/** "just now", "3 min ago". */
export function agoText(seconds) {
  return seconds < 60 ? "just now" : `${Math.floor(seconds / 60)} min ago`;
}

/**
 * The desktop app's "Connect your AI app" for `ai` (web/ai-local.js
 * `LocalAiAccess`). Resolves when it closes.
 */
export async function openLocalDialog(ai, doc = document) {
  doc.querySelector("dialog.ai")?.close();
  const opener = doc.activeElement;
  const dialog = el("dialog", { class: "ai ai-local", "aria-labelledby": "ai-title" });
  const on = el("input", { type: "checkbox", id: "ai-on" });
  const onRow = el("label", { class: "switch ai-keep" }, on, el("span", {}, "Let AI apps on this computer use the calculators", el("small", { text: "Off by default. Turning it off stops access at once." })));
  const status = el("p", { class: "ai-status", role: "status" });
  const address = el("input", { class: "input ai-address", type: "text", readonly: true, "aria-label": "MCP address", spellcheck: "false" });
  const copy = copyButton("Copy");
  const addressRow = el("div", { class: "ai-address-row" }, address, copy);
  const block = installBlock({ connector: "Claude and ChatGPT connect from the internet, so they cannot use this address." });
  const keyNote = el("p", { class: "ai-note", text: "The links and commands include a key that lets the AI app in. The AI app keeps it in its settings file. Turn this off to shut every AI app out." });
  const onPart = el("div", { class: "ai-install ai-on-part" }, addressRow, block.node, keyNote);
  const apps = el("ul", { class: "ai-apps" });
  const appsNote = el("p", { class: "ai-note" });
  const appsPart = el("div", { class: "ai-apps-part" }, el("h3", { text: "Connected apps" }), apps, appsNote);
  const port = el("input", { class: "input ai-port", type: "number", min: "1024", max: "65535", id: "ai-port" });
  const usePort = el("button", { type: "button", class: "small", text: "Use this port" });
  const portRow = el("div", { class: "ai-address-row ai-port-row" }, el("label", { for: "ai-port", text: "Port" }), port, usePort);
  const done = el("button", { type: "button", text: "Done" });
  dialog.append(el("div", { class: "ai-box" },
    el("h2", { id: "ai-title", text: "Connect your AI app" }),
    el("p", { text: "AI apps on this computer (Claude Code, Codex, VS Code, Cursor, …) can use the calculators in this app: read the display, press keys, run programs and look up commands. Nothing goes over the internet." }),
    onRow,
    status,
    onPart,
    appsPart,
    el("p", { class: "ai-note", text: "The AI app can press any key and run any command, as you can. Storing or purging a variable through its store and purge tools asks you here first. While an AI app is connected, the calculator keeps running when this window is in the background." }),
    portRow,
    el("div", { class: "ai-actions" }, el("span", { class: "ai-gap" }), done),
  ));
  let busy = false;
  const render = () => {
    const s = ai.status;
    const running = ai.state === "connected";
    on.checked = Boolean(s.enabled);
    on.disabled = busy;
    if (doc.activeElement !== port) port.value = String(s.port ?? s.defaultPort ?? 4842);
    usePort.disabled = busy;
    onPart.hidden = !running;
    appsPart.hidden = !running;
    address.value = running ? s.address ?? "" : "";
    if (running) block.set({ url: s.address, headers: ai.mcp.headers });
    status.classList.toggle("error", Boolean(ai.error));
    status.textContent = busy ? "Working…" : ai.error ?? (running ? "AI access is on. Add the address to your AI app." : "AI access is off.");
    const grants = running ? ai.mcp.grants : [];
    apps.replaceChildren(...grants.map((g) => el("li", {}, el("span", { class: "ai-app" }, el("strong", { text: g.client.name }), el("small", { text: ` ${agoText(g.secondsAgo ?? 0)}` })))));
    appsNote.textContent = grants.length ? "" : "No AI app has connected yet.";
    appsNote.hidden = grants.length > 0;
  };
  const apply = async (enabled, p = undefined) => {
    busy = true;
    render();
    try {
      await ai.set(enabled, p);
    } finally {
      busy = false;
      render();
    }
  };
  on.addEventListener("change", () => apply(on.checked));
  usePort.addEventListener("click", () => {
    const n = Number(port.value);
    if (!Number.isInteger(n) || n < 1024 || n > 65535) {
      ai.error = "Choose a port from 1024 to 65535.";
      render();
      port.focus();
      return;
    }
    apply(on.checked, n);
  });
  copy.addEventListener("click", () => copyText(copy, "Copy", address.value, () => address.select()));
  done.addEventListener("click", () => dialog.close());
  dialog.addEventListener("click", (e) => {
    if (e.target === dialog) dialog.close();
  });
  ai.addEventListener("change", render);
  doc.body.append(dialog);
  render();
  dialog.showModal();
  const closed = new Promise((resolve) => dialog.addEventListener("close", resolve, { once: true }));
  await ai.load?.().catch(() => {});
  render();
  await closed;
  ai.removeEventListener("change", render);
  dialog.remove();
  if (opener && opener !== doc.body && opener.isConnected && typeof opener.focus === "function") opener.focus();
}
