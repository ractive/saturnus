// The emulated calculator as a device's other end: saturnus's core,
// started in server mode by typing SERVER, its serial port then the link
// (EmulatorLink, links.js). The same transfer code talks to it as to a
// cable (kb/docs/saturnus-tx.md, "Devices"). Two kinds:
// - `shared` (a ROM the emulator keeps): the emulator's own live
//   calculator of that model, in the calculators' SharedWorker
//   (web/shared/calc-hub.js, web/hub.js, iteration 46), shown by the
//   emulator page at the same time; joined, attached, and left on close
//   (the hub ends the server and lets the line go).
// - otherwise a private one in its own Worker (emulator-worker.js, loaded
//   only when the user adds one), booted from a ROM file, keeping nothing.
//
// Events: `serial` (`detail.bytes`), `frame` (the display, for the
// page's small screen), `gone` (the Worker failed, or its core: `fatal`;
// for a shared one also the emulator starting it afresh or its ROM
// removed: the hub's `gone`), `keysElsewhere` (a shared one: ATTN
// pressed on it in the emulator while this page holds its line; the
// server may have ended, kermit-device.js checks it).

import { canShare, joinHub, NO_SHARING } from "../shared/calc-hub.js";
import { APLET_MODELS, NO_SERIAL, SERIAL_MODELS } from "./kept-roms.js";

const PROTOCOL = 1;

/** Why the page does not start the server of a calculator in use again by itself. */
export const IN_USE = "it is in use (a command line open, or a program running). Run SERVER on it in the emulator, or disconnect here and connect again.";
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
  if (NO_SERIAL.includes(model)) return `The ${model.toUpperCase()} has no serial port: it cannot be a device here.`;
  if (APLET_MODELS.includes(model)) return `The ${model.toUpperCase()} sends and receives aplets itself: serve a folder to it instead.`;
  return "This ROM is not one of a 48SX, 48GX or 49G.";
}

export class EmulatedCalculator extends EventTarget {
  /**
   * `shared`: the emulator's calculator of a kept ROM, through the
   * SharedWorker (`join` the hub's way in; tests give their own);
   * otherwise a private Worker (`worker`).
   */
  constructor({ shared = false, join = joinHub, worker = () => new Worker(new URL("./emulator-worker.js", import.meta.url), { type: "module" }) } = {}) {
    super();
    this.nextId = 1;
    this.waiting = new Map();
    this.model = null;
    /** The emulator's live calculator, shown there at the same time. */
    this.shared = shared;
    if (shared) {
      this.port = null;
      this.release = () => {};
      this.joined = (canShare() || join !== joinHub ? join("tx", (m) => this.receive(m)) : Promise.reject(new Error(NO_SHARING))).then((j) => {
        this.port = j.port;
        this.release = j.release;
        j.worker.onerror = (e) => this.fail(e?.message || "the calculators' worker failed");
      });
      this.joined.catch(() => {});
    } else {
      this.worker = worker();
      this.port = this.worker;
      this.joined = Promise.resolve();
      this.worker.onmessage = (e) => this.receive(e.data);
      this.worker.onerror = (e) => this.fail(e.message || "the emulator failed to start");
    }
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
    if (m?.type === "fatal" || m?.type === "gone") {
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
      this.joined.then(
        () => this.port.postMessage({ v: PROTOCOL, id, cmd, ...args }),
        (err) => {
          this.waiting.delete(id);
          reject(err);
        },
      );
    });
  }

  /**
   * Boot `rom` (bytes; `name` its file name; `model` a preference, the
   * ROM picks its own) and wait until the calculator waits for a key, a
   * second in a row (a restored state: 0.2 s); resolves to the model. Refuses a model without a
   * serial port (with `aplets`, one that is not a 38G, 39G or 40G),
   * after booting it. A shared one is the emulator's calculator of
   * `model`: the one running, or booted as it was left.
   */
  async boot(rom, name, model = "48gx", { timeoutMs = 120_000, aplets = false } = {}) {
    await this.request("hello");
    const booted = await this.request(this.shared ? "attach" : "boot", { model, rom, romName: name });
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
   * Resolves to whether SERVER was typed. `gentle` (the page starting it
   * again on its own, kermit-device.js): a calculator with a command line
   * open or a program running is the user's, left alone (an error says
   * so); a calculator turned off is turned on.
   */
  async startServer({ gentle = false } = {}) {
    if (gentle) {
      if ((await this.request("commandLine"))?.active || !(await this.idle())) throw new Error(IN_USE);
      if (this.lastFrame && blank(this.lastFrame)) await this.tap("on");
    } else if (this.shared) {
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

  /** Whether the calculator waits for a key (its loop asleep) within half a second: no program running. */
  async idle() {
    for (let i = 0; i < 5; i++) {
      if ((await this.request("stats"))?.loop === "sleep") return true;
      await sleep(100);
    }
    return false;
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
   * Stop: a shared calculator is left (the hub ends a server still
   * running and lets the line go; the emulator page goes on showing it,
   * or it is saved and stopped when no page shows it), a private Worker
   * ends.
   */
  async close() {
    if (this.closed) return;
    this.closed = true;
    if (this.shared) {
      // Longer than the hub's own wait for a send to end (20 s).
      await Promise.race([this.request("leave").catch(() => {}), sleep(25_000)]);
    }
    for (const w of this.waiting.values()) w.reject(new Error("closed"));
    this.waiting.clear();
    if (this.shared) {
      this.port?.close();
      this.release();
    } else {
      this.worker.terminate();
    }
  }
}
