// The private emulated calculator as a device's other end: saturnus's
// core in its own Worker (emulator-worker.js, loaded only when the user
// adds one), booted from a ROM file chosen here, keeping nothing; started
// in server mode by typing SERVER, its serial port then the link
// (EmulatorLink, links.js). The same transfer code talks to it as to a
// cable (kb/docs/saturnus-tx.md, "Devices"): the honest test of the cable
// path. The emulator's own calculator (a ROM it keeps) is no link: it is
// a HubDevice (web/shared/hub-device.js, iteration 47).
//
// Events: `serial` (`detail.bytes`), `frame` (the display, for the
// page's small screen), `gone` (the Worker failed, or its core: `fatal`).

import { CALC_STOPPED, Heartbeat } from "../shared/calc-hub.js";
import { APLET_MODELS, NO_SERIAL, SERIAL_MODELS } from "./kept-roms.js";
import { modelTitle } from "./flows.js";

const PROTOCOL = 1;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/**
 * Why `model` cannot be an emulated device, or null; with `aplets`, the
 * device a folder is served to (a 38G, 39G or 40G).
 */
export function emulatedRefusal(model, { aplets = false } = {}) {
  if (aplets) return APLET_MODELS.includes(model) ? null : "This ROM is not one of a 38G, 39G or 40G.";
  if (SERIAL_MODELS.includes(model)) return null;
  if (NO_SERIAL.includes(model)) return `The ${modelTitle(model)} has no serial port: it cannot be a device here.`;
  if (APLET_MODELS.includes(model)) return `The ${modelTitle(model)} sends and receives aplets itself: serve a folder to it instead.`;
  return "This ROM is not one of a 48SX, 48GX or 49G.";
}

export class EmulatedCalculator extends EventTarget {
  /** `worker` makes its Worker (tests give their own). */
  constructor({ worker = () => new Worker(new URL("./emulator-worker.js", import.meta.url), { type: "module" }) } = {}) {
    super();
    this.nextId = 1;
    this.waiting = new Map();
    this.pings = new Map();
    this.model = null;
    // A worker that stops answering (crashed) fails the waiting requests, and the calculator is gone.
    this.heartbeat = new Heartbeat({
      ping: () => this.ping(),
      busy: () => this.waiting.size > 0,
      stopped: () => this.fail(CALC_STOPPED),
    });
    this.worker = worker();
    this.port = this.worker;
    this.joined = Promise.resolve();
    this.worker.onmessage = (e) => this.receive(e.data);
    this.worker.onerror = (e) => this.fail(e.message || "the emulator failed to start");
  }

  receive(m) {
    if (m?.type === "reply") {
      const map = this.pings.has(m.id) ? this.pings : this.waiting;
      const w = map.get(m.id);
      if (!w) return;
      map.delete(m.id);
      if (m.ok) w.resolve(m.result);
      else w.reject(new Error(m.error));
      return;
    }
    if (m?.type === "fatal") {
      this.fail(m.message);
      return;
    }
    if (m?.type) this.dispatchEvent(new CustomEvent(m.type, { detail: m }));
  }

  /** The Worker cannot go on: every waiting request fails, and `gone`. */
  fail(message) {
    this.heartbeat.stop();
    for (const w of [...this.waiting.values(), ...this.pings.values()]) w.reject(new Error(message));
    this.waiting.clear();
    this.pings.clear();
    this.dispatchEvent(new CustomEvent("gone", { detail: message }));
  }

  request(cmd, args = {}) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      // Watched once joined: a slow join is no stopped worker.
      this.joined.then(() => {
        if (!this.closed) this.heartbeat.kick();
      }, () => {});
      this.joined.then(
        () => this.port.postMessage({ v: PROTOCOL, id, cmd, ...args }),
        (err) => {
          this.waiting.delete(id);
          reject(err);
        },
      );
    });
  }

  /** The heartbeat's question (web/protocol.md, `ping`), answered by the worker at once. */
  ping() {
    const id = `ping:${this.nextId++}`;
    return new Promise((resolve, reject) => {
      this.pings.set(id, { resolve, reject });
      this.joined.then(
        () => this.port.postMessage({ v: PROTOCOL, id, cmd: "ping" }),
        (err) => {
          this.pings.delete(id);
          reject(err);
        },
      );
    });
  }

  /**
   * Boot `rom` (bytes; `name` its file name; `model` a preference, the
   * ROM picks its own) and wait until the calculator waits for a key, a
   * second in a row; resolves to the model. Refuses a model without a
   * serial port (with `aplets`, one that is not a 38G, 39G or 40G),
   * after booting it.
   */
  async boot(rom, name, model = "48gx", { timeoutMs = 120_000, aplets = false } = {}) {
    await this.request("hello");
    const booted = await this.request("boot", { model, rom, romName: name });
    this.model = booted?.model ?? model;
    const refusal = emulatedRefusal(this.model, { aplets });
    if (refusal) throw new Error(refusal);
    const end = Date.now() + timeoutMs;
    const [polls, every] = [5, 200];
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
   * packet, `probeServer`; after a fast transfer or a restore, say), else
   * SERVER and ENTER are typed (the port is taken first, so nothing it
   * says is lost). Resolves to whether SERVER was typed.
   */
  async startServer() {
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

  /** Stop: the Worker ends, and the calculator with it. */
  async close() {
    if (this.closed) return;
    this.closed = true;
    this.heartbeat.stop();
    for (const w of this.waiting.values()) w.reject(new Error("closed"));
    this.waiting.clear();
    this.worker.terminate();
  }
}
