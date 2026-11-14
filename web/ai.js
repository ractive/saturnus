// AI access (iteration 55): the app's calculators offered to the user's
// AI app (Claude, ChatGPT, Claude Code, …) through a tab-mcp relay
// (https://github.com/ractive/tab-mcp). The tab connects to the relay and
// gets an MCP address; the AI app calls the tools (web/ai-tools.js)
// through the relay while the tab is open.
//
// The client library is vendored (web/vendor/tab-mcp-client/, `just
// vendor-tab-mcp`) and loaded only when AI access is used. One `AiAccess`
// for the document (web/main.js), whichever view is shown. Not in the
// desktop app, which serves MCP itself (web/ai-local.js, the same tools).
//
// Calculators: the one Calculator shows, through its own backend; the
// other running ones (kept calculators Files shows) through a hub
// connection of their own (`join {who: "ai"}`, `attach` with the kept
// ROM's bytes, as Files does), which never says hidden. While a session is
// connected a hidden tab keeps its calculators running (owner, option 1):
// Calculator stops sending `visibility` hidden (app.js, on `change`).
//
// A broken connection (the relay restarted, the network went) is tried
// again by the client library (tab-mcp 0.4 on; its state `reconnecting` since 0.16): 5 s
// up to every 60 s, at once when the tab is back in front or online, until
// it is back, the user turns AI access off, or the relay refuses it. The
// page only shows it: the panel's "lost", the log's notes. The calls and
// the connection's events go to `log` (web/ai-log.js, "The AI's calls");
// never the address, a token or a pairing code.
//
// Where the browser has WebMCP, the same tools go to the browser's own AI
// agent as well (web/ai-webmcp.js, iteration 71), side by side with
// tab-mcp: `tools` makes them once for both, and WebMCP's calls go to the
// same log. Then `AiAccess` is made even without a relay (a page served
// from this computer), `available` false.

import { aiTools, INSTRUCTIONS, titleOf } from "./ai-tools.js";
import { AiLog, appLabel } from "./ai-log.js";
import { HubClient, hubRunning } from "./shared/calc-hub.js";
import { KEYS, aiSecretKey } from "./shared/storage-names.js";
import { relayUrl } from "./ai-relay.js";
import { keptRomBytes, keptRoms } from "./tx/kept-roms.js";
import { lookOf, screenPng as screenPngBytes } from "./screenshot.js";
import { prefs } from "./shared/prefs.js";
import { AT_DEFAULT } from "./contrast.js";
import { pngBase64 } from "./shared/png.js";
import "./shared/models.js";

/** The app's version for the MCP server's info. */
const VERSION = "1";

/**
 * A promise of the display as a PNG, base64, at 3× in `look` ("lcd": the
 * LCD's colours, "bw": black on white; web/screenshot.js, as Copy screen
 * makes it); null where none can be made. The LCD's colours are those of
 * the power-on contrast whatever the calculator's (`AT_DEFAULT`): turned
 * light (ON-), the ink would be nearly gone, and on the key-driven models
 * the image is all the AI reads of the screen.
 */
export async function screenPng(frame, model, look = "lcd") {
  try {
    return await displayPng(frame, model, lookOf(look));
  } catch {
    return null;
  }
}

/** The display as a PNG, a promise of base64; throws or rejects where it cannot be made. */
function displayPng(frame, model, look) {
  const bin = atob(frame.pixels);
  const row = Math.ceil(frame.width / 8);
  const px = new Uint8Array(frame.width * frame.height);
  for (let y = 0; y < frame.height; y++) {
    for (let x = 0; x < frame.width; x++) px[y * frame.width + x] = (bin.charCodeAt(y * row + (x >> 3)) >> (7 - (x & 7))) & 1;
  }
  return pngBase64(screenPngBytes(frame, px, model, look, 3, { darkness: AT_DEFAULT }));
}

/** The app's "Screen images" setting (the store's `screenLook`, kept in prefs as it changes). */
export function settingLook() {
  return lookOf(prefs.get("screenLook"));
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
      // A cancelled call's keys not played yet (web/ai-tools.js).
      releaseAll: () => { r("releaseAll").catch(() => {}); },
      stats: () => r("stats"),
      run: (text, { codes = false } = {}) => r("run", { text, ...(codes ? { codes } : {}) }),
      stack: () => r("stack"),
      commandLine: () => r("commandLine"),
      entryModes: () => r("entryModes"),
      uiState: () => r("uiState"),
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
    client = () => import("./vendor/tab-mcp-client/tab-mcp-client.min.js"),
    now = () => Date.now(),
    look = settingLook,
  } = {}) {
    super();
    /** The clock, for the log and the waits it notes (tests give a fake). */
    this.now = now;
    /** The look of the tools' screen images when a call names none: the "Screen images" setting (tests give a fake). */
    this.look = look;
    /** Counts the user's Turn offs: a connect that outlived one says nothing. */
    this.offs = 0;
    /** The page lost its address to another tab (`{sibling}`: one of this page in this browser, else a tab or program with the address's secret), until Connect or a new address. */
    this.replaced = null;
    /** The tools that ask the user first (`consequentialHint`). */
    this.consequential = new Set();
    /** The connected apps of the live session, by grant id (their names), to say who came and went. */
    this.apps = null;
    this.calculatorView = calculatorView;
    /** The calculators' hub (tests give a fake): whether it runs, a new connection. */
    this.hub = hub;
    /** The kept ROMs (web/tx/kept-roms.js): `kept(models)`, `bytes(sha256)`. */
    /** Loads tab-mcp's client (the vendored module; tests give a fake). */
    this.client = client;
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
    /** The words of the last refused Disconnect of an AI app (`revoke`), which a retry that works clears from `error`. */
    this.revokeError = null;
    /** "The AI's calls" (web/ai-log.js): from now on, in memory only; tab-mcp's calls and WebMCP's. */
    this.log = new AiLog(this, { now: () => this.now() });
    /** The tools through WebMCP (web/ai-webmcp.js `WebMcp`), where the browser has it; set by web/main.js. */
    this.webmcp = null;
    this.toolList = null;
  }

  /** The relay's host name, for the log. */
  get relayHost() {
    try {
      return new URL(this.relay.replace(/^ws/, "http")).host;
    } catch {
      return "";
    }
  }

  /** Something for the log (`what`: connected, lost, wait, paired, unpaired, off). */
  note(what, detail = {}) {
    this.dispatchEvent(new CustomEvent("note", { detail: { what, ...detail } }));
  }

  /** The connection broke and the client is connecting again by itself (its state `reconnecting`, tab-mcp 0.16). */
  get lost() {
    return this.mcp?.state === "reconnecting";
  }

  /** Whether AI access can be offered here (a relay). */
  get available() {
    return Boolean(this.relay);
  }

  /** The client's state: disconnected, connecting, connected, or reconnecting (`lost`). */
  get state() {
    return this.mcp?.state ?? "disconnected";
  }

  /** Whether the calculators keep running in a hidden tab now (`keepsRunning`). */
  get keepsRunning() {
    return keepsRunning(this);
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

  /**
   * The tools (web/ai-tools.js), made once: tab-mcp and WebMCP
   * (web/ai-webmcp.js) register the same ones, so their calls queue per
   * calculator together.
   */
  get tools() {
    if (!this.toolList) {
      this.toolList = aiTools({ calculators: () => this.calculators(), target: (id) => this.target(id), existing: (id) => this.existing(id), reference: { index: (m) => this.reference().then((r) => r.index(m)) }, png: (frame, model, look) => this.png(frame, model, look) });
      for (const tool of this.toolList) if (tool.annotations?.consequentialHint) this.consequential.add(tool.name);
    }
    return this.toolList;
  }

  /** The client loaded and the tools registered (once). */
  load() {
    this.loading ??= (async () => {
      const { TabMcp } = await this.client();
      const origin = new URL(this.relay.replace(/^ws/, "http")).origin;
      const mcp = new TabMcp({
        relay: this.relay,
        appName: "saturnus",
        server: { name: "saturnus", title: "saturnus calculators", version: VERSION, instructions: INSTRUCTIONS },
        storageKey: aiSecretKey(origin),
        // The client's WebMCP mirror (tab-mcp 0.13) stays off: web/ai-webmcp.js registers the tools
        // with WebMCP itself, without the page's question; the mirror would register them twice.
        webmcp: false,
      });
      for (const tool of this.tools) await mcp.registerTool(tool);
      // `statechange` (tab-mcp 0.16) carries `reason` when a session ends (connected to disconnected
      // or, broken, to reconnecting) and when the page ends connecting or reconnecting; `reconnecting`
      // comes again for each attempt the client schedules.
      mcp.addEventListener("statechange", (e) => {
        const { state, reason } = e.detail;
        if (state === "connected") {
          this.error = null;
          this.apps = new Map(mcp.grants.map((g) => [g.grantId, appLabel(g.client)]));
          this.note("connected", { host: this.relayHost, apps: mcp.grants.length });
        }
        // Connecting again (Connect, a new address): no longer replaced.
        if (state === "connecting") this.replaced = null;
        if (reason !== undefined) {
          this.release();
          this.apps = null;
          const ours = reason === "user" || reason === "new_address";
          if (!ours) this.note("lost", { reason, text: lostText(reason) });
          // What to do; when the client tries again (reconnecting), why it was lost (below).
          this.error = ours ? null : disconnectText(reason);
        }
        const next = mcp.reconnecting;
        if (state === "reconnecting" && next) {
          // Tried again until it is back, whether or not "Keep AI access on" is set: the user had it on.
          this.error = lostText(next.reason);
          this.note("wait", { attempt: next.attempt, seconds: Math.max(0, Math.round((next.at - this.now()) / 1000)) });
        }
        this.changed();
      });
      // After the `statechange` to disconnected with reason replaced (tab-mcp 0.8): who took the address, up to a second later.
      mcp.addEventListener("replaced", (e) => {
        if (this.state !== "disconnected") return;
        const sibling = e.detail?.sibling === true;
        this.replaced = { sibling };
        this.error = replacedText(sibling);
        this.changed();
      });
      mcp.addEventListener("pairingcode", () => this.changed());
      for (const type of ["toolcall", "toolcallend"]) mcp.addEventListener(type, (e) => this.dispatchEvent(new CustomEvent(type, { detail: e.detail })));
      mcp.addEventListener("grantschange", () => {
        this.noteApps(mcp.grants);
        this.changed();
      });
      mcp.addEventListener("error", (e) => {
        const message = e.detail?.error?.message ?? null;
        // Already disconnected: the relay refused an attempt of the client's reconnecting, which ends
        // it (tab-mcp 0.16: `error` before the `statechange` to disconnected, also when a Connect of
        // ours waited on that attempt). The relay's words, as a refused Connect says them.
        this.error = message !== null && mcp.state === "disconnected" ? `Could not connect to the relay: ${message}` : message;
        this.changed();
      });
      this.mcp = mcp;
      return mcp;
    })();
    this.loading.catch(() => { this.loading = null; });
    return this.loading;
  }

  /**
   * Connect to the relay (from a click, or `resume` for "Keep AI access
   * on"). While the client reconnects by itself it makes the next attempt
   * at once; a failure that may pass leaves the rest to the client.
   */
  async connect({ resume = false } = {}) {
    if (!this.lost) this.error = null;
    const offs = this.offs;
    try {
      const mcp = await this.load();
      return await mcp.connect({ resume });
    } catch (err) {
      // Turned off meanwhile: that is what happened, not an error.
      if (offs !== this.offs) throw err;
      // Still reconnecting: the client tries again later.
      if (this.lost) throw err;
      // Refused without a click (the click's activation ran out while the client loaded): say what to do.
      this.error = err?.name === "NotAllowedError" ? "Press Connect to turn on AI access." : `Could not connect to the relay: ${err?.message ?? err}`;
      this.changed();
      throw err;
    }
  }

  /** Who came and went among the connected apps of the live session (the first list on connecting is not news). */
  noteApps(grants) {
    if (!this.apps || this.state !== "connected") return;
    const now = new Map(grants.map((g) => [g.grantId, appLabel(g.client)]));
    for (const [id, name] of now) if (!this.apps.has(id)) this.note("paired", { app: name });
    for (const [id, name] of this.apps) if (!now.has(id)) this.note("unpaired", { app: name });
    this.apps = now;
  }

  /** On load, with "Keep AI access on": connect without a click. */
  resume() {
    if (!this.available || !this.keepOn) return;
    this.connect({ resume: true }).catch(() => {});
  }

  /** Turn AI access off in this tab (the connected apps stay paired; Connect brings it back). Ends the client's reconnecting. */
  disconnect() {
    const on = this.lost || this.state !== "disconnected";
    this.offs++;
    this.error = null;
    this.mcp?.disconnect();
    this.release();
    if (on) this.note("off");
    this.changed();
  }

  /**
   * A new address: the apps connected to the old one lose access. After
   * the address was taken over, the client takes it back first (tab-mcp
   * 0.8), so those apps are disconnected; taken over again meanwhile, it
   * rejects with reason `replaced`, and the dialog shows what to do.
   */
  newAddress() {
    this.release();
    this.replaced = null;
    // The takeover's words go while the address is taken back; a failure brings its own.
    this.error = null;
    // Resolves with nothing (tab-mcp 0.16): the new address is `mcp.session.address` once connected.
    const made = this.mcp?.newAddress();
    made?.catch((err) => {
      this.error = err?.reason === "replaced" ? "It was taken over again before its AI apps were disconnected. Try again." : `Could not connect to the relay: ${err?.message ?? err}`;
      this.changed();
    });
    return made;
  }

  /**
   * Disconnect one AI app (tab-mcp 0.16: a promise that resolves once the
   * relay removed its grant). Rejects with the reason in the dialog's
   * words as its message: not connected, the relay could not save it, or
   * no answer in time.
   */
  async revoke(grantId) {
    try {
      if (!this.mcp) throw new DOMException("TabMcp: not connected", "InvalidStateError");
      await this.mcp.revoke(grantId);
    } catch (err) {
      // In place of the client's `error`, which carried the relay's own words for a refusal.
      const text = revokeText(err);
      this.error = text;
      this.revokeError = text;
      this.changed();
      throw new Error(text, { cause: err });
    }
    // Done: a refusal said before (a retry that worked) goes, but not words set meanwhile
    // for something else (the connection lost while the relay answered).
    if (this.revokeError !== null && this.error === this.revokeError) this.error = null;
    this.revokeError = null;
    this.changed();
  }

  /** The other calculators' connections let go: the session ended. */
  release() {
    this.session++;
    for (const p of this.attached.values()) p.then((t) => t.client.close(), () => {});
    this.attached.clear();
    this.lobby?.close();
    this.lobby = null;
  }

  /** A promise of the display as a PNG, base64 (null where none can be made) in `look` (else the setting's). */
  png(frame, model, look) {
    return screenPng(frame, model, look ?? this.look());
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

  /** The target of `id` only if there is one already (Calculator's, or one attached): never attaches; null otherwise. */
  async existing(id) {
    const v = await this.view();
    const shown = v?.store.state.booted ?? null;
    if (shown && id === shown) return { id: shown, model: shown, backend: v.backend, store: v.store };
    return this.attached.get(id) ?? null;
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

/**
 * Whether the calculators keep running in a hidden tab (owner, option 1):
 * while an AI app is connected, not for the relay's session alone.
 */
export function keepsRunning(ai) {
  return ai.state === "connected" && (ai.mcp?.grants?.length ?? 0) > 0;
}

/** Why a session ended, without what to do: the log's words, and the dialog's while it reconnects. */
export function lostText(reason) {
  switch (reason) {
    case "replaced": return "Another tab took over this address. AI access is on there now.";
    case "expired": return "The session reached its time limit.";
    case "idle": return "No AI app used the calculators for a long time, so the session ended.";
    case "shutdown": return "The relay restarted.";
    case "network": case "heartbeat": return "The connection to the relay broke.";
    case "user": return "AI access was turned off.";
    default: return "The relay ended the session.";
  }
}

/** Why disconnecting an AI app failed (tab-mcp 0.16 `revoke()`'s rejection), in the dialog's words. */
export function revokeText(err) {
  if (err?.name === "InvalidStateError") return "Connect again first: only a connected tab can disconnect an app.";
  if (err?.code === "store_unavailable") return "The relay could not save this. Try again.";
  if (err?.name === "TimeoutError") return "The relay did not answer. Try again.";
  // tab-mcp 0.16.1: the session ended after the request went out; the relay may have done it.
  if (err?.name === "NetworkError") return "The connection ended before the relay answered. Connect again to see whether the app is still connected.";
  // Relays before 0.10.2 (from 0.16.1 the client drops such a grant and resolves).
  if (err?.code === "unknown_id") return "The relay does not know this app any more. Connect again to see the apps it knows.";
  return `Could not disconnect the app: ${err?.message ?? err}`;
}

/** Why a session ended, in the dialog's words: what to do as well. */
export function disconnectText(reason) {
  return reason === "replaced" ? lostText(reason) : `${lostText(reason)} Connect again.`;
}

/**
 * The address was taken, and by whom (tab-mcp's `replaced`, its default
 * dialog's words): a sibling tab of this page, or another tab or program.
 */
export function replacedText(sibling) {
  return sibling ? "Opened in another tab. Your AI apps use that one now; connect again to use them here." : "Another tab or program took over this page.";
}
