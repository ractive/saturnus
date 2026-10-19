// "Connect your AI app" (iteration 55): the dialog of AI access
// (web/ai.js) in the page's own look (shared/components.css, `dialog.ai`):
// the MCP address with Copy, the state, the pairing code once the relay
// sends one (tab-mcp's OAuth, its iteration 3), the AI apps connected with
// Disconnect each, "Keep AI access on", and Connect, Turn off or New
// address. Opened from Search and from Calculator's panel; one at a time.
// While it is open the relay is asked for pairing codes (tab-mcp
// `startPairing`).

import { el } from "./shared/dom.js";

/**
 * Open the dialog for `ai` (an `AiAccess`), from a click: connects first
 * when not connected (the click is the user's choice). Resolves when it
 * closes.
 */
export async function openAiDialog(ai, doc = document) {
  doc.querySelector("dialog.ai")?.close();
  const opener = doc.activeElement;
  const dialog = el("dialog", { class: "ai", "aria-labelledby": "ai-title" });
  const status = el("p", { class: "ai-status", role: "status" });
  const address = el("input", { class: "input ai-address", type: "text", readonly: true, "aria-label": "MCP address", spellcheck: "false" });
  const copy = el("button", { type: "button", text: "Copy" });
  const addressRow = el("div", { class: "ai-address-row" }, address, copy);
  const code = el("p", { class: "ai-code", hidden: true });
  const apps = el("ul", { class: "ai-apps" });
  const appsNote = el("p", { class: "ai-note" });
  const keep = el("input", { type: "checkbox", id: "ai-keep" });
  const keepRow = el("label", { class: "switch ai-keep" }, keep, el("span", {}, "Keep AI access on", el("small", { text: "Connect again when the app opens." })));
  const connect = el("button", { type: "button", class: "primary", text: "Connect" });
  const off = el("button", { type: "button", text: "Turn off" });
  const fresh = el("button", { type: "button", class: "link", text: "New address" });
  const done = el("button", { type: "button", text: "Done" });
  dialog.append(el("div", { class: "ai-box" },
    el("h2", { id: "ai-title", text: "Connect your AI app" }),
    el("p", { text: "Your AI app (Claude, ChatGPT, Claude Code, Cursor, …) can use the calculators in this tab: read the display, press keys, run programs and look up commands. Add this address to it as an MCP server, or custom connector." }),
    addressRow,
    code,
    status,
    el("h3", { text: "Connected apps" }),
    apps,
    appsNote,
    el("p", { class: "ai-note", text: "The AI app can press any key and run any command, as you can. Storing or purging a variable through its store and purge tools asks you here first. While an AI app is connected, the calculators keep running when this tab is in the background." }),
    keepRow,
    el("div", { class: "ai-actions" }, fresh, el("span", { class: "ai-gap" }), off, connect, done),
  ));

  const render = () => {
    const state = ai.state;
    const mcp = ai.mcp;
    address.value = state === "connected" ? mcp.address : "";
    addressRow.hidden = state !== "connected";
    const pc = mcp?.pairingCode;
    code.hidden = !pc;
    if (pc) code.textContent = `When your AI app asks for a code, enter ${pc.code}.`;
    status.classList.toggle("error", Boolean(ai.error));
    status.textContent = ai.error ?? (state === "connected" ? "AI access is on. Your AI app can connect now." : state === "connecting" ? "Connecting…" : "AI access is off.");
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
    off.hidden = state === "disconnected";
    fresh.hidden = state !== "connected";
  };

  copy.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(address.value);
      copy.textContent = "Copied";
    } catch {
      address.select();
      copy.textContent = "Press ⌘C or Ctrl+C";
    }
    setTimeout(() => { copy.textContent = "Copy"; }, 2000);
  });
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
  done.addEventListener("click", () => dialog.close());
  // A click on the backdrop (the dialog itself, outside its box) closes it.
  dialog.addEventListener("click", (e) => {
    if (e.target === dialog) dialog.close();
  });

  ai.addEventListener("change", render);
  doc.body.append(dialog);
  render();
  dialog.showModal();
  const closed = new Promise((resolve) => dialog.addEventListener("close", resolve, { once: true }));
  if (ai.state === "disconnected") {
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
