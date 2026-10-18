// The desktop app's calculator shared between the document's views
// (iteration 51): what the hub (web/hub.js) does per connection in the
// browser, in front of the one TauriBackend the document has. The app
// runs one native calculator and keeps no state per view, so this page
// does: Calculator (the TauriBackend itself) and Files (a `client("tx")`
// per HubDevice, web/shared/hub-device.js) each say whether they watch
// the memory and whether they are hidden, and the app is told the merged
// answer (watched while any view watches; hidden only when every view
// is). Everything else passes through unchanged, in the order it was
// sent: one session, one numbering (crates/saturnus-tauri `order.rs`).
//
// Files' client speaks HubClient's language (web/shared/calc-hub.js):
// `attach` shows the calculator Calculator runs (or boots `model` from the
// ROM library when none runs; another model running is refused: the app
// has one calculator), *bytes* travel as base64 over invoke (`storeFile`'s
// `data` out, `fetchFile`'s and `backup`'s `data` back; `fetchFile` goes
// with `inline` so the app answers the bytes instead of opening a save
// dialog), `close` leaves. `gone` when Calculator turns the calculator
// off, starts another model or starts it again (Start fresh, another ROM);
// `memoryChanged` while attached. Calculator hears `peers` (Files is on
// its calculator) as from the hub.

import { TauriBackend, base64, unbase64 } from "./backend.js";
import { SERIAL_SHARED } from "./shared/calc-hub.js";
import "./shared/models.js";

/** The commands of Calculator that boot the calculator again (a client attached before hears `gone`). */
const BOOTS = new Set(["boot", "bootModel", "chooseRom", "downloadRom"]);

/** The serial commands refused while Files is on the calculator (`serialClose` passes). */
const SERIAL_REFUSED = new Set(["serialOpen", "serialWrite"]);

const title = (model) => globalThis.SATURNUS_MODEL_TITLES?.[model] ?? `HP ${model.toUpperCase()}`;

/** The model a boot command's result booted, or null (cancelled, or nothing booted). */
export function bootedModel(cmd, result) {
  if (!result) return null;
  const booted = cmd === "boot" ? result : result.booted;
  return booted?.model ?? null;
}

/** A reply's base64 `data` as a Uint8Array (as the Worker gives it). */
const withBytes = (r) => (r && typeof r.data === "string" ? { ...r, data: unbase64(r.data) } : r);

/** How long `knownRunning` waits for the app's `status` after its `hello`. */
const STATUS_MS = 1000;

export class NativeHub {
  /** `backend` the document's TauriBackend (or a test's fake: `invoke`, events). */
  constructor(backend) {
    this.backend = backend;
    backend.hub = this;
    /** Calculator's own state (the TauriBackend's requests). */
    this.emulator = { who: "emulator", watch: false, hidden: false };
    this.clients = new Set();
    /** The model the app runs (`status`): null none, undefined not heard yet. */
    this.running = undefined;
    this.romName = "";
    this.sentWatch = false;
    this.sentHidden = false;
    this.sentPeers = false;
    backend.addEventListener("status", (e) => this.onStatus(e.detail));
    backend.addEventListener("memoryChanged", (e) => {
      for (const c of this.attached()) c.emit("memoryChanged", e.detail);
    });
  }

  /** The clients on the calculator. */
  attached() {
    return [...this.clients].filter((c) => c.model);
  }

  /** Every view whose watch and visibility count: Calculator and the clients on the calculator. */
  views() {
    return [this.emulator, ...this.attached()];
  }

  /** A command to the app as it is. */
  raw(cmd, args = {}) {
    return this.backend.invoke(cmd, args);
  }

  /** Command `cmd` of view `view` (`this.emulator` or a client); the merged commands are rewritten. */
  request(view, cmd, args = {}) {
    switch (cmd) {
      case "watchMemory":
        view.watch = Boolean(args.on);
        // Always sent: the view wants the app's answer (`{supported, reason}`).
        return this.watch(true);
      case "visibility":
        view.hidden = Boolean(args.hidden);
        this.sentHidden = this.hidden();
        return this.raw("visibility", { ...args, hidden: this.sentHidden });
      case "hello": {
        view.watch = false;
        const answer = this.raw("hello", args);
        // The app's hello ends its memory watch and lets its serial port go: another view's watch is asked for again.
        this.sentWatch = false;
        this.watch(false).catch(() => {});
        return answer;
      }
      default:
        if (SERIAL_REFUSED.has(cmd) && this.attached().length) return Promise.reject(new Error(SERIAL_SHARED));
    }
    const answer = this.raw(cmd, args);
    if (view === this.emulator && BOOTS.has(cmd)) {
      answer.then((r) => {
        const model = bootedModel(cmd, r);
        if (!model) return;
        // Known from the reply already: its `status` may come after it.
        this.running = model;
        this.dropAll(cmd === "bootModel" && args.fresh ? "Calculator restarted this calculator with an empty memory." : "Calculator restarted this calculator.");
      }, () => {});
    }
    return answer;
  }

  /** Whether any view watches the memory. */
  watching() {
    return this.views().some((v) => v.watch);
  }

  /** Hidden only when every view is (Files never says so). */
  hidden() {
    return this.views().every((v) => v.hidden);
  }

  /** Tell the app the merged watch: always (`force`), or when it changed. */
  watch(force) {
    const on = this.watching();
    if (!force && on === this.sentWatch) return Promise.resolve(null);
    this.sentWatch = on;
    return this.raw("watchMemory", { on });
  }

  /** The merged watch, visibility and `peers` after a view came or went. */
  settle() {
    this.watch(false).catch(() => {});
    const hidden = this.hidden();
    if (hidden !== this.sentHidden) {
      this.sentHidden = hidden;
      this.raw("visibility", { hidden }).catch(() => {});
    }
    const tx = this.attached().length > 0;
    if (tx !== this.sentPeers) {
      this.sentPeers = tx;
      this.backend.dispatch({ type: "peers", tx });
    }
  }

  onStatus(s) {
    const model = s?.model ?? null;
    this.running = model;
    this.romName = s?.romName ?? "";
    this.heardStatus?.();
    for (const c of this.attached()) {
      if (c.model !== model) this.drop(c, model ? `Calculator now runs the ${title(model)}.` : "The calculator was turned off in Calculator.");
    }
  }

  /** Client `c` leaves the calculator, told why (`gone`). */
  drop(c, why) {
    c.model = null;
    this.settle();
    c.emit("gone", why);
  }

  dropAll(why) {
    for (const c of this.attached()) this.drop(c, why);
  }

  /**
   * Client `c` shows `model`'s calculator: the one the app runs, or booted
   * from the ROM library when none runs (Calculator shows it too). Another
   * model running is refused. Resolves as the hub's `attach`.
   */
  async attach(c, model) {
    let booted;
    const running = await this.knownRunning();
    if (running === undefined) {
      // Never boot over a calculator that may be running: Calculator owns it.
      throw new Error("Calculator is still starting. Try again in a moment.");
    } else if (running === model) {
      booted = { model, romName: this.romName, restored: true, attached: true };
    } else if (running) {
      throw new Error(`Calculator runs the ${title(running)}, and Files can only reach the calculator shown there. Switch to the ${title(model)} in Calculator first.`);
    } else {
      const r = await this.raw("bootModel", { model });
      booted = r?.booted;
      if (!booted?.model) throw new Error(`The ${title(model)} did not start.`);
    }
    if (c.closed) throw new Error("closed");
    c.model = booted.model;
    this.settle();
    return booted;
  }

  /**
   * The model the app runs; when not heard yet (the window reloaded
   * straight into Files, Calculator not started), asked for with a
   * `hello`, after which the app sends its `status`. Undefined when no
   * status came within `STATUS_MS` (Tauri does not order events before
   * replies).
   */
  async knownRunning() {
    if (this.running !== undefined) return this.running;
    const heard = new Promise((resolve) => { this.heardStatus = resolve; });
    await this.request(this.emulator, "hello", {}).catch(() => {});
    await Promise.race([heard, new Promise((resolve) => setTimeout(resolve, STATUS_MS))]);
    this.heardStatus = null;
    return this.running;
  }

  /** A new client for Files (`who` as HubClient's). */
  client(who = "tx") {
    const c = new NativeClient(this, who);
    this.clients.add(c);
    return c;
  }

  /** Client `c` leaves. */
  leave(c) {
    c.closed = true;
    c.model = null;
    this.clients.delete(c);
    this.settle();
  }
}

/** Files' connection to the app's calculator: HubClient's interface (`request`, events, `close`). */
export class NativeClient extends EventTarget {
  constructor(hub, who) {
    super();
    this.hub = hub;
    this.who = who;
    /** The model shown, while attached. */
    this.model = null;
    this.watch = false;
    this.hidden = false;
    this.closed = false;
  }

  emit(type, detail) {
    this.dispatchEvent(new CustomEvent(type, { detail }));
  }

  /** The command `cmd` with `args`; *bytes* as Uint8Arrays both ways. */
  request(cmd, args = {}) {
    if (this.closed) return Promise.reject(new Error("closed"));
    if (cmd === "attach") return this.hub.attach(this, String(args.model));
    if (cmd === "leave") return this.close();
    let msg = args;
    if (cmd === "storeFile" && msg.data instanceof Uint8Array) msg = { ...msg, data: base64(msg.data) };
    if (cmd === "fetchFile") msg = { ...msg, inline: true };
    const answer = this.hub.request(this, cmd, msg);
    return cmd === "fetchFile" || cmd === "backup" ? answer.then(withBytes) : answer;
  }

  /** Leave the calculator; Calculator goes on showing it. */
  async close() {
    if (!this.closed) this.hub.leave(this);
    return null;
  }
}

/** The document's one hub, over its one TauriBackend (made on the first call). */
let theHub = null;

/** The desktop app's hub: Calculator's backend is `nativeHub().backend`, Files' connections `nativeHub().client()`. */
export function nativeHub(tauri = globalThis.__TAURI__) {
  theHub ??= new NativeHub(new TauriBackend(tauri));
  return theHub;
}
