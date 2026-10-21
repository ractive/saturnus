// AI access (iteration 55): the app's calculators offered to the user's
// AI app (Claude, ChatGPT, Claude Code, …) through a tab-mcp relay
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
//
// A broken connection (the relay restarted, the network went) is tried
// again with a growing wait, 5 s up to every 60 s, until it is back, the
// user turns AI access off, or the relay refuses it (iteration 56). The
// calls and the connection's events go to `log` (web/ai-log.js, "The AI's
// calls"); never the address, a token or a pairing code.

import { aiTools, INSTRUCTIONS, titleOf } from "./ai-tools.js";
import { AiLog, appLabel } from "./ai-log.js";
import { HubClient, hubRunning } from "./shared/calc-hub.js";
import { KEYS, aiSecretKey } from "./shared/storage-names.js";
import { relayUrl } from "./ai-relay.js";
import { keptRomBytes, keptRoms } from "./tx/kept-roms.js";
import { screenRgba } from "./screenshot.js";
import "./shared/models.js";

/** The app's version for the MCP server's info. */
const VERSION = "1";

/** Why a session ended that is worth connecting again for: the relay or the network went away for a while. */
const TRANSIENT = new Set(["network", "heartbeat", "shutdown"]);

/** The wait before retry `attempt` (1, 2, …) after a broken connection: 5, 10, 20, 40 s, then every 60 s. */
export function retryDelay(attempt) {
  return [5000, 10000, 20000, 40000][attempt - 1] ?? 60000;
}

/**
 * Whether a failed connect may work later (the relay down or restarting,
 * no answer in time), not refused for good: the relay said no (an
 * address it refuses, its message limit), the browser said no, or it was
 * cancelled. The client gives no codes, so its messages are read.
 */
export function transientFailure(err) {
  if (!err || err.name === "NotAllowedError" || err.name === "AbortError") return false;
  const m = String(err.message ?? "");
  if (m.startsWith("The relay refused the connection") || m.includes("message limit")) return false;
  const ended = /^The relay ended the connection: (\w+)/.exec(m);
  if (ended) return TRANSIENT.has(ended[1]);
  return true;
}

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
    client = () => import("./vendor/tab-mcp-client/tab-mcp-client.min.js"),
    timers = { set: (fn, ms) => setTimeout(fn, ms), clear: (t) => clearTimeout(t), now: () => Date.now() },
  } = {}) {
    super();
    /** The clock and timers of the retries (tests give fakes). */
    this.timers = timers;
    /** The connection broke and is being tried again (`retrying` the next try's time, or null while it runs). */
    this.lost = false;
    this.retrying = null;
    /** The retries since the connection broke. */
    this.attempt = 0;
    this.retryTimer = null;
    /** Counts the user's Turn offs: a connect that outlived one says nothing. */
    this.offs = 0;
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
    /** "The AI's calls" (web/ai-log.js): from now on, in memory only. */
    this.log = new AiLog(this, { now: () => this.timers.now() });
  }

  /** The relay's host name, for the log. */
  get relayHost() {
    try {
      return new URL(this.relay.replace(/^ws/, "http")).host;
    } catch {
      return "";
    }
  }

  /** Something for the log (`what`: connected, lost, retrying, wait, paired, unpaired, off). */
  note(what, detail = {}) {
    this.dispatchEvent(new CustomEvent("note", { detail: { what, ...detail } }));
  }

  /** Whether AI access can be offered here (a relay). */
  get available() {
    return Boolean(this.relay);
  }

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
      });
      for (const tool of aiTools({ calculators: () => this.calculators(), target: (id) => this.target(id), reference: { index: (m) => this.reference().then((r) => r.index(m)) }, png: screenPng })) {
        await mcp.registerTool(tool);
        if (tool.annotations?.consequentialHint) this.consequential.add(tool.name);
      }
      for (const type of ["statechange", "pairingcode"]) mcp.addEventListener(type, () => this.changed());
      for (const type of ["toolcall", "toolcallend"]) mcp.addEventListener(type, (e) => this.dispatchEvent(new CustomEvent(type, { detail: e.detail })));
      mcp.addEventListener("grantschange", () => {
        this.noteApps(mcp.grants);
        this.changed();
      });
      mcp.addEventListener("connected", () => {
        this.stopRetrying();
        this.error = null;
        this.apps = new Map(mcp.grants.map((g) => [g.grantId, appLabel(g.client)]));
        this.note("connected", { host: this.relayHost, apps: mcp.grants.length });
        this.changed();
      });
      mcp.addEventListener("error", (e) => {
        this.error = e.detail?.error?.message ?? null;
        this.changed();
      });
      mcp.addEventListener("disconnected", (e) => {
        this.release();
        this.apps = null;
        const reason = e.detail?.reason;
        if (reason !== "user" && reason !== "new_address") this.note("lost", { reason, text: lostText(reason) });
        if (TRANSIENT.has(reason)) {
          // Tried again until it is back, whether or not "Keep AI access on" is set: the user had it on.
          this.lost = true;
          this.error = lostText(reason);
          this.attempt = 0;
          this.scheduleRetry();
        } else {
          this.error = reason && reason !== "user" && reason !== "new_address" ? disconnectText(reason) : null;
        }
        this.changed();
      });
      this.mcp = mcp;
      return mcp;
    })();
    this.loading.catch(() => { this.loading = null; });
    return this.loading;
  }

  /**
   * Connect to the relay (from a click, `resume` for "Keep AI access on",
   * or a retry). After a broken connection a failure that may pass
   * (`transientFailure`) schedules the next retry; any other ends them.
   */
  async connect({ resume = false } = {}) {
    if (this.retryTimer !== null) this.timers.clear(this.retryTimer);
    this.retryTimer = null;
    this.retrying = null;
    if (!this.lost) this.error = null;
    const offs = this.offs;
    try {
      const mcp = await this.load();
      return await mcp.connect({ resume });
    } catch (err) {
      // Turned off meanwhile: that is what happened, not an error.
      if (offs !== this.offs) throw err;
      if (this.lost && transientFailure(err)) {
        this.scheduleRetry();
        throw err;
      }
      this.stopRetrying();
      // Refused without a click (the click's activation ran out while the client loaded): say what to do.
      this.error = err?.name === "NotAllowedError" ? "Press Connect to turn on AI access." : `Could not connect to the relay: ${err?.message ?? err}`;
      this.changed();
      throw err;
    }
  }

  /** The next retry after a broken connection, `retryDelay` from now. */
  scheduleRetry() {
    this.attempt++;
    const ms = retryDelay(this.attempt);
    this.retrying = this.timers.now() + ms;
    this.note("wait", { seconds: Math.round(ms / 1000) });
    this.retryTimer = this.timers.set(() => {
      this.retryTimer = null;
      this.retrying = null;
      this.note("retrying", { attempt: this.attempt });
      this.changed();
      this.connect({ resume: true }).catch(() => {});
    }, ms);
    this.changed();
  }

  /**
   * Try the waiting retry now (the tab came back to the front, or the
   * network came back): a phone pauses a background tab, so its connection
   * breaks while the user is in the AI app, and the backoff may still have
   * a minute to go when they return.
   */
  retryNow() {
    if (this.retryTimer === null) return;
    this.timers.clear(this.retryTimer);
    this.retryTimer = null;
    this.retrying = null;
    this.note("retrying", { attempt: this.attempt });
    this.changed();
    this.connect({ resume: true }).catch(() => {});
  }

  /** No more retries: connected again, turned off, or refused. */
  stopRetrying() {
    if (this.retryTimer !== null) this.timers.clear(this.retryTimer);
    this.retryTimer = null;
    this.retrying = null;
    this.lost = false;
    this.attempt = 0;
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

  /** Turn AI access off in this tab (the connected apps stay paired; Connect brings it back). Ends the retries. */
  disconnect() {
    const on = this.lost || this.state !== "disconnected";
    this.offs++;
    this.stopRetrying();
    this.error = null;
    this.mcp?.disconnect();
    this.release();
    if (on) this.note("off");
    this.changed();
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

/** Why a session ended, in the dialog's words: what to do as well. */
export function disconnectText(reason) {
  return reason === "replaced" ? lostText(reason) : `${lostText(reason)} Connect again.`;
}
