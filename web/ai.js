// AI access (iteration 55): the app's calculators offered to the user's
// AI app (Claude, ChatGPT, Claude Code, Cursor, …) through a tab-mcp relay
// (https://github.com/ractive/tab-mcp). The tab connects to the relay and
// gets an MCP address; the AI app calls the tools (web/ai-tools.js)
// through the relay while the tab is open.
//
// The client library is vendored (web/vendor/tab-mcp-client/, `just
// vendor-tab-mcp`) and loaded only when AI access is used. One `AiAccess`
// for the document (web/main.js), whichever view is shown. Not in the
// desktop app, which gets an MCP server of its own.
//
// Calculators: the one Calculator shows, through its own backend; the
// other running ones (kept calculators Files shows) through a hub
// connection of their own (`join {who: "ai"}`, `attach` with the kept
// ROM's bytes, as Files does), which never says hidden. While a session is
// connected a hidden tab keeps its calculators running (owner, option 1):
// Calculator stops sending `visibility` hidden (app.js, on `change`).

import { aiTools, INSTRUCTIONS, titleOf } from "./ai-tools.js";
import { HubClient, hubRunning } from "./shared/calc-hub.js";
import { KEYS, aiSecretKey } from "./shared/storage-names.js";
import { relayUrl } from "./ai-relay.js";
import { keptRomBytes, keptRoms } from "./tx/kept-roms.js";
import { screenRgba } from "./screenshot.js";
import "./shared/models.js";

/** The app's version for the MCP server's info. */
const VERSION = "1";

/** The display as a PNG (base64), black on white at 3× (web/screenshot.js). */
function screenPng(frame, model) {
  const bin = atob(frame.pixels);
  const row = Math.ceil(frame.width / 8);
  const px = new Uint8Array(frame.width * frame.height);
  for (let y = 0; y < frame.height; y++) {
    for (let x = 0; x < frame.width; x++) px[y * frame.width + x] = (bin.charCodeAt(y * row + (x >> 3)) >> (7 - (x & 7))) & 1;
  }
  const img = screenRgba(frame, px, model, "bw", 3);
  const canvas = document.createElement("canvas");
  canvas.width = img.width;
  canvas.height = img.height;
  canvas.getContext("2d").putImageData(new ImageData(img.data, img.width, img.height), 0, 0);
  return canvas.toDataURL("image/png").split(",")[1];
}

/**
 * A kept calculator other than Calculator's, for the tools: a hub
 * connection attached to it, with the state the tools read (`frame`,
 * `keysDown`, `busy`) kept from its events.
 */
class HubTarget {
  constructor(model, client) {
    this.id = model;
    this.model = model;
    this.client = client;
    this.store = { state: { frame: null, keysDown: [], busy: false, booted: null } };
    client.addEventListener("frame", (e) => { this.store.state.frame = e.detail; });
    client.addEventListener("keys", (e) => { this.store.state.keysDown = e.detail.down; });
    client.addEventListener("status", (e) => {
      this.store.state.busy = Boolean(e.detail.busy);
      this.store.state.booted = e.detail.model || null;
    });
    const r = (cmd, args) => client.request(cmd, args);
    this.backend = {
      skin: (m) => r("skin", { model: m }),
      // As the page's backend: no answer awaited (the names were checked).
      typeKeys: (keys) => { r("typeKeys", { keys }).catch(() => {}); },
      stats: () => r("stats"),
      run: (text) => r("run", { text }),
      stack: () => r("stack"),
      commandLine: () => r("commandLine"),
      entryModes: () => r("entryModes"),
      flags: () => r("flags"),
      memoryTree: () => r("memoryTree"),
      editText: (where) => r("editText", where),
      storeText: ({ text, ...where }) => r("storeText", { ...where, text }),
      purge: (dir, name) => r("purge", { dir, name }),
    };
  }
}

/**
 * The document's AI access. `calculatorView()` gives Calculator's mounted
 * handle (web/app.js `mountCalculator`; null before its first visit).
 * Events: `change` whenever the state, the apps or the setting changed.
 */
export class AiAccess extends EventTarget {
  constructor({
    calculatorView,
    relay = relayUrl(),
    storage = globalThis.localStorage,
    hub = { running: hubRunning, client: (who) => new HubClient({ who }) },
    roms = { kept: (models) => keptRoms(undefined, models), bytes: (sha256) => keptRomBytes(sha256) },
  } = {}) {
    super();
    this.calculatorView = calculatorView;
    /** The calculators' hub (tests give a fake): whether it runs, a new connection. */
    this.hub = hub;
    /** The kept ROMs (web/tx/kept-roms.js): `kept(models)`, `bytes(sha256)`. */
    this.roms = roms;
    /** Counts the sessions' ends: an attach that outlived its session lets go. */
    this.session = 0;
    this.relay = relay;
    this.storage = storage;
    /** The `TabMcp`, once loaded. */
    this.mcp = null;
    this.loading = null;
    /** The hub connections to calculators other than Calculator's, by model: a promise of each, shared by parallel calls. */
    this.attached = new Map();
    this.lobby = null;
    this.error = null;
  }

  /** Whether AI access can be offered here (a relay). */
  get available() {
    return Boolean(this.relay);
  }

  get state() {
    return this.mcp?.state ?? "disconnected";
  }

  get keepOn() {
    try {
      return this.storage?.getItem(KEYS.aiKeepOn) === "on";
    } catch {
      return false;
    }
  }

  set keepOn(on) {
    try {
      if (on) this.storage?.setItem(KEYS.aiKeepOn, "on");
      else this.storage?.removeItem(KEYS.aiKeepOn);
    } catch {
      // Storage blocked: the setting lasts for this visit only.
    }
    this.changed();
  }

  changed() {
    this.dispatchEvent(new Event("change"));
  }

  /** The client loaded and the tools registered (once). */
  load() {
    this.loading ??= (async () => {
      const { TabMcp } = await import("./vendor/tab-mcp-client/tab-mcp-client.min.js");
      const origin = new URL(this.relay.replace(/^ws/, "http")).origin;
      const mcp = new TabMcp({
        relay: this.relay,
        appName: "saturnus",
        server: { name: "saturnus", title: "saturnus calculators", version: VERSION, instructions: INSTRUCTIONS },
        storageKey: aiSecretKey(origin),
      });
      for (const tool of aiTools({ calculators: () => this.calculators(), target: (id) => this.target(id), reference: { index: (m) => this.reference().then((r) => r.index(m)) }, png: screenPng })) {
        await mcp.registerTool(tool);
      }
      for (const type of ["statechange", "grantschange", "pairingcode", "connected"]) mcp.addEventListener(type, () => this.changed());
      mcp.addEventListener("error", (e) => {
        this.error = e.detail?.error?.message ?? null;
        this.changed();
      });
      mcp.addEventListener("disconnected", (e) => {
        this.release();
        const reason = e.detail?.reason;
        this.error = reason && reason !== "user" && reason !== "new_address" ? disconnectText(reason) : null;
        this.changed();
        // "Keep AI access on": once more after a broken connection (never in a loop: a failed connect ends it).
        if (["network", "heartbeat", "shutdown"].includes(reason)) setTimeout(() => this.resume(), 5000);
      });
      this.mcp = mcp;
      return mcp;
    })();
    this.loading.catch(() => { this.loading = null; });
    return this.loading;
  }

  /** Connect to the relay (from a click, or `resume` for "Keep AI access on"). */
  async connect({ resume = false } = {}) {
    this.error = null;
    try {
      const mcp = await this.load();
      return await mcp.connect({ resume });
    } catch (err) {
      this.error = `Could not connect to the relay: ${err?.message ?? err}`;
      this.changed();
      throw err;
    }
  }

  /** On load, with "Keep AI access on": connect without a click. */
  resume() {
    if (!this.available || !this.keepOn) return;
    this.connect({ resume: true }).catch(() => {});
  }

  disconnect() {
    this.mcp?.disconnect();
    this.release();
  }

  /** A new address: the apps connected to the old one lose access. */
  newAddress() {
    this.release();
    return this.mcp?.newAddress();
  }

  revoke(grantId) {
    return this.mcp?.revoke(grantId) ?? false;
  }

  /** The other calculators' connections let go: the session ended. */
  release() {
    this.session++;
    for (const p of this.attached.values()) p.then((t) => t.client.close(), () => {});
    this.attached.clear();
    this.lobby?.close();
    this.lobby = null;
  }

  /** Calculator's parts for the tools (`{backend, store, reference}`), or null before it is mounted. */
  async view() {
    const handle = await this.calculatorView();
    return handle ? handle.ai() : null;
  }

  /** The command reference: Calculator's loader, or one of our own before it is mounted. */
  async reference() {
    const v = await this.view();
    if (v?.reference) return v.reference;
    this.ownReference ??= import("./palette.js").then(({ ReferenceLoader }) => new ReferenceLoader(new URL("./commands.json", import.meta.url)));
    return this.ownReference;
  }

  /** The running calculators: Calculator's first, then the others the hub runs. */
  async calculators() {
    const v = await this.view();
    const shown = v?.store.state.booted ?? null;
    const out = [];
    if (shown) out.push({ id: shown, model: shown, title: titleOf(shown), rom: v.store.state.romName || null, shown: true });
    if (this.hub.running()) {
      this.lobby ??= this.hub.client("ai");
      const engines = await this.lobby.request("engines");
      for (const e of engines) {
        if (e.model === shown || out.some((c) => c.id === e.model)) continue;
        out.push({ id: e.model, model: e.model, title: titleOf(e.model), rom: e.romName || null, shown: false });
      }
    }
    return out;
  }

  /** The calculator `id` (null: Calculator's) for a tool. */
  async target(id) {
    const v = await this.view();
    const shown = v?.store.state.booted ?? null;
    if (!id || id === shown) {
      if (!shown) throw new Error(id ? `No calculator ${id} is running. list_calculators names them.` : "The app's Calculator view shows no calculator. list_calculators names the running ones; or ask the user to open one.");
      return { id: shown, model: shown, backend: v.backend, store: v.store };
    }
    const have = this.attached.get(id);
    if (have) return have;
    // One attach per calculator, whoever asks meanwhile; forgotten when it fails.
    const attach = this.attach(id, this.session, () => this.attached.get(id) === attach && this.attached.delete(id));
    this.attached.set(id, attach);
    attach.catch(() => {
      if (this.attached.get(id) === attach) this.attached.delete(id);
    });
    return attach;
  }

  /**
   * A hub connection attached to the running calculator `id` (session
   * `session`): closed again when the attach fails, when the calculator
   * goes (`forget` drops it from `attached`), or when the session ended
   * while it waited.
   */
  async attach(id, session, forget) {
    const running = (await this.calculators()).find((c) => c.id === id);
    if (!running) throw new Error(`No calculator ${id} is running. list_calculators names them.`);
    const kept = (await this.roms.kept([...globalThis.SATURNUS_MODEL_ORDER])).find((r) => r.model === id);
    const rom = kept ? await this.roms.bytes(kept.sha256) : null;
    if (!rom) throw new Error(`The ${titleOf(id)} cannot be reached: its ROM is not kept in this browser.`);
    if (session !== this.session) throw new Error("AI access was turned off.");
    const client = this.hub.client("ai");
    const t = new HubTarget(id, client);
    client.addEventListener("gone", () => {
      forget();
      client.close();
    });
    try {
      await client.request("attach", { model: id, rom, romName: kept.name });
    } catch (err) {
      client.close();
      throw err;
    }
    if (session !== this.session) {
      client.close();
      throw new Error("AI access was turned off.");
    }
    return t;
  }
}

/** Why a session ended, in the dialog's words. */
export function disconnectText(reason) {
  switch (reason) {
    case "replaced": return "Another tab took over this address. AI access is on there now.";
    case "expired": return "The session reached its time limit. Connect again.";
    case "idle": return "No AI app used the calculators for a long time, so the session ended. Connect again.";
    case "shutdown": return "The relay restarted. Connect again.";
    case "network": case "heartbeat": return "The connection to the relay broke. Connect again.";
    default: return "The relay ended the session. Connect again.";
  }
}
