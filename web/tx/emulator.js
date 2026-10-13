// The emulated calculator as a device's other end: saturnus's core in its
// own Worker (emulator-worker.js, loaded only when the user adds one),
// booted from a ROM, started in server mode by typing SERVER, its serial
// port then the link (EmulatorLink, links.js). The same transfer code
// talks to it as to a cable (kb/docs/saturnus-tx.md, "Devices"). Booted
// `shared` (a ROM the emulator keeps), it is the emulator's own
// calculator of that model, as the user left it (iteration 41).
//
// Events: `serial` (`detail.bytes`), `frame` (the display, for the
// page's small screen), `gone` (the Worker failed, or its core: `fatal`),
// and for a shared one `wanted` (another page asks for it: leave SERVER,
// then `close`) and `taken` (another page took it without asking;
// `detail.by`).

import { APLET_MODELS, PIONEERS, SERIAL_MODELS } from "./kept-roms.js";

const PROTOCOL = 1;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Whether a `frame` event's display is blank (base64 pixels, all zero): a calculator turned off. */
export function blank(frame) {
  try {
    return !/[^\0]/.test(atob(frame.pixels ?? ""));
  } catch {
    return false;
  }
}

/**
 * Why `model` cannot be an emulated device, or null; with `aplets`, the
 * device a folder is served to (a 38G, 39G or 40G).
 */
export function emulatedRefusal(model, { aplets = false } = {}) {
  if (aplets) return APLET_MODELS.includes(model) ? null : "This ROM is not one of a 38G, 39G or 40G.";
  if (SERIAL_MODELS.includes(model)) return null;
  if (PIONEERS.includes(model)) return `The ${model.toUpperCase()} has no serial port: it cannot be a device here.`;
  if (APLET_MODELS.includes(model)) return `The ${model.toUpperCase()} sends and receives aplets itself: serve a folder to it instead.`;
  return "This ROM is not one of a 48SX, 48GX or 49G.";
}

export class EmulatedCalculator extends EventTarget {
  constructor({ worker = () => new Worker(new URL("./emulator-worker.js", import.meta.url), { type: "module" }) } = {}) {
    super();
    this.worker = worker();
    this.nextId = 1;
    this.waiting = new Map();
    this.model = null;
    /** Booted from the emulator's state, saved back there. */
    this.shared = false;
    this.worker.onmessage = (e) => this.receive(e.data);
    this.worker.onerror = (e) => this.fail(e.message || "the emulator failed to start");
  }

  receive(m) {
    if (m?.type === "reply") {
      const w = this.waiting.get(m.id);
      if (!w) return;
      this.waiting.delete(m.id);
      if (m.ok) w.resolve(m.result);
      else w.reject(new Error(m.error));
      return;
    }
    if (m?.type === "fatal") {
      this.fail(m.message);
      return;
    }
    if (m?.type === "frame") this.lastFrame = m;
    if (m?.type) this.dispatchEvent(new CustomEvent(m.type, { detail: m }));
  }

  /** The Worker cannot go on: every waiting request fails, and `gone`. */
  fail(message) {
    for (const w of this.waiting.values()) w.reject(new Error(message));
    this.waiting.clear();
    this.dispatchEvent(new CustomEvent("gone", { detail: message }));
  }

  request(cmd, args = {}) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      this.worker.postMessage({ v: PROTOCOL, id, cmd, ...args });
    });
  }

  /**
   * Boot `rom` (bytes; `name` its file name; `model` a preference, the
   * ROM picks its own) and wait until the calculator waits for a key, a
   * second in a row (a restored state: 0.2 s); resolves to the model. Refuses a model without a
   * serial port (with `aplets`, one that is not a 38G, 39G or 40G),
   * after booting it. `shared`: the emulator's calculator of `model`
   * (asked for if the emulator runs it), restored as it was left.
   */
  async boot(rom, name, model = "48gx", { timeoutMs = 120_000, aplets = false, shared = false } = {}) {
    await this.request("hello");
    // Set first: a close while it boots still hands it back.
    this.shared = shared;
    const booted = await this.request(shared ? "bootShared" : "boot", { model, rom, romName: name });
    this.model = booted?.model ?? model;
    const refusal = emulatedRefusal(this.model, { aplets });
    if (refusal) throw new Error(refusal);
    const end = Date.now() + timeoutMs;
    // A restored calculator was left waiting: it settles at once.
    const [polls, every] = booted?.restored ? [2, 100] : [5, 200];
    for (let idle = 0; idle < polls; ) {
      let up = false;
      try {
        // The 38G, 39G and 40G have no stack to read.
        if (!aplets) await this.request("stack");
        up = (await this.request("stats")).loop === "sleep";
      } catch {
        up = false;
      }
      idle = up ? idle + 1 : 0;
      if (Date.now() > end) throw new Error("The emulated calculator did not start.");
      await sleep(every);
    }
    return this.model;
  }

  /** `speed`: "1", "2", "4" or "max" (emulated time per wall time while it computes). */
  async setSpeed(speed) {
    await this.request("setSpeed", { speed });
    this.speed = speed;
  }

  /**
   * Run `fn` with the calculator at `speed`, then at its own again: the
   * fast transfer runs at 1x, as XSEND and XRECV time their waits by
   * computing, not sleeping (at Max they give up before the page answers).
   */
  async atSpeed(speed, fn) {
    const was = this.speed ?? "1";
    if (was === speed) return fn();
    await this.setSpeed(speed);
    try {
      return await fn();
    } finally {
      if (!this.closed) await this.setSpeed(was).catch(() => {});
    }
  }

  /**
   * Start the Kermit server: asked first whether it runs already (one I
   * packet, `probeServer`; a state saved in SERVER), else SERVER and
   * ENTER are typed (the port is taken first, so nothing it says is
   * lost). A calculator left off is turned on, and a command line left
   * open is cancelled first (a shared one comes as the user left it).
   * Resolves to whether SERVER was typed.
   */
  async startServer() {
    if (this.shared) {
      // Turned off in the emulator: its display is blank (the stack always shows something).
      if (this.lastFrame && blank(this.lastFrame)) await this.tap("on");
      if ((await this.request("commandLine"))?.active) await this.tap("on");
    }
    // The question needs the port free (a reconnect still holds it).
    await this.request("serialClose");
    // Not asked, it is not typed: SERVER typed into a running server fails.
    const asked = await this.request("probeServer");
    await this.request("serialOpen");
    if (asked?.server) return false;
    const r = await this.request("run", { text: "SERVER" });
    if (r?.error) throw new Error(`SERVER: ${r.error}`);
    return true;
  }

  /** Type `text` and ENTER (a fast transfer's `'NAME' XRECV`): resolves once typed, the command possibly still running. */
  async typeLine(text) {
    const r = await this.request("run", { text });
    if (r?.error) throw new Error(`${text}: ${r.error}`);
    return r;
  }

  /** Press `key` (a key name) and release it after `ms`. */
  async tap(key, ms = 80) {
    await this.request("keyDown", { key });
    await sleep(ms);
    await this.request("keyUp", { key });
  }

  /**
   * Stop the Worker; a shared calculator is saved and let go first (leave
   * SERVER before: the emulator gets it back as it is).
   */
  async close() {
    if (this.shared && !this.closed) {
      this.shared = false;
      // Longer than the Worker's own wait for a send to end (20 s).
      await Promise.race([this.request("handOver").catch(() => {}), sleep(25_000)]);
    }
    this.closed = true;
    for (const w of this.waiting.values()) w.reject(new Error("closed"));
    this.waiting.clear();
    this.worker.terminate();
  }
}
