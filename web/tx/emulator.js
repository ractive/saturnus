// The emulated calculator as a device's other end: saturnus's core in its
// own Worker (emulator-worker.js, loaded only when the user adds one),
// booted from a ROM, started in server mode by typing SERVER, its serial
// port then the link (EmulatorLink, links.js). The same transfer code
// talks to it as to a cable (kb/docs/saturnus-tx.md, "Devices").
//
// Events: `serial` (`detail.bytes`), `frame` (the display, for the
// page's small screen), `gone` (the Worker failed, or its core: `fatal`).

import { SERIAL_MODELS } from "./kept-roms.js";

const PROTOCOL = 1;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Why `model` cannot be an emulated device, or null. */
export function emulatedRefusal(model) {
  if (SERIAL_MODELS.includes(model)) return null;
  if (model === "42s") return "The 42S has no serial port: it cannot be a device here.";
  if (["38g", "39g", "40g"].includes(model)) return `The ${model.toUpperCase()} sends and receives aplets itself; it comes to this page later.`;
  return "This ROM is not one of a 48SX, 48GX or 49G.";
}

export class EmulatedCalculator extends EventTarget {
  constructor({ worker = () => new Worker(new URL("./emulator-worker.js", import.meta.url), { type: "module" }) } = {}) {
    super();
    this.worker = worker();
    this.nextId = 1;
    this.waiting = new Map();
    this.model = null;
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
   * second in a row; resolves to the model. Refuses a model without a
   * serial port, after booting it.
   */
  async boot(rom, name, model = "48gx", { timeoutMs = 120_000 } = {}) {
    await this.request("hello");
    const booted = await this.request("boot", { model, rom, romName: name });
    this.model = booted?.model ?? model;
    const refusal = emulatedRefusal(this.model);
    if (refusal) throw new Error(refusal);
    const end = Date.now() + timeoutMs;
    for (let idle = 0; idle < 5; ) {
      let up = false;
      try {
        await this.request("stack");
        up = (await this.request("stats")).loop === "sleep";
      } catch {
        up = false;
      }
      idle = up ? idle + 1 : 0;
      if (Date.now() > end) throw new Error("The emulated calculator did not start.");
      await sleep(200);
    }
    return this.model;
  }

  /** `speed`: "1", "2", "4" or "max" (emulated time per wall time while it computes). */
  setSpeed(speed) { return this.request("setSpeed", { speed }); }

  /** Type SERVER and ENTER (the port is taken first, so nothing it says is lost). */
  async startServer() {
    await this.request("serialOpen");
    const r = await this.request("run", { text: "SERVER" });
    if (r?.error) throw new Error(`SERVER: ${r.error}`);
  }

  /** Type `text` and ENTER (a fast transfer's `'NAME' XRECV`): resolves once typed, the command possibly still running. */
  async typeLine(text) {
    const r = await this.request("run", { text });
    if (r?.error) throw new Error(`${text}: ${r.error}`);
    return r;
  }

  /** Stop the Worker. */
  close() {
    for (const w of this.waiting.values()) w.reject(new Error("closed"));
    this.waiting.clear();
    this.worker.terminate();
  }
}
