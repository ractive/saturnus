// The links that carry a calculator's bytes, all behind one interface
// (kb/docs/saturnus-tx.md, "Devices"):
//
//   open({baud, parity, dataBits})   (re)open at these settings
//   write(bytes)                     resolves once handed on
//   close()
//   "data" event, detail a Uint8Array   bytes read
//   "lost" event, detail the reason     the link went away (unplugged, ...)
//
// - `WebSerialLink`: a port of the Web Serial API (Chrome, Edge), chosen
//   by the user in the browser's chooser.
// - `EmulatorLink`: the emulated calculator's serial port (emulator.js,
//   the saturnus Worker's serialOpen, serialWrite, serial event,
//   serialClose: web/protocol.md, "The serial port").
// - `FakeLink`: a recorded trace's calculator (trace.js), for the tests.
// - `RecordingLink`: any link, its traffic written down as a trace.

import { formatTrace, parseTrace, sameBytes } from "./trace.js";

export class Link extends EventTarget {
  /** A short name for the status line: "USB serial port". */
  label = "link";
  emitData(bytes) { this.dispatchEvent(new CustomEvent("data", { detail: bytes })); }
  emitLost(reason) { this.dispatchEvent(new CustomEvent("lost", { detail: reason })); }
  async open() {}
  async write() {}
  async close() {}
}

// ------------------------------------------------------------ Web Serial

/** Whether this browser has the Web Serial API. */
export const hasWebSerial = (nav = globalThis.navigator) => Boolean(nav?.serial?.requestPort);

/** Read errors after which the port goes on reading (Web Serial's recoverable ones). */
const RECOVERABLE = new Set(["BreakError", "FramingError", "ParityError", "BufferOverrunError"]);

/** A Web Serial port: one stop bit, no flow control; reopened to change speed or parity. */
export class WebSerialLink extends Link {
  constructor(port, { serial = globalThis.navigator?.serial } = {}) {
    super();
    this.port = port;
    const info = port.getInfo?.() ?? {};
    this.label = info.usbVendorId !== undefined ? "USB serial port" : "serial port";
    this.reader = null;
    this.reading = null;
    this.stopping = false;
    this.onDisconnect = (e) => {
      if (e.target === port || e.port === port) this.emitLost("The cable or adapter was unplugged.");
    };
    serial?.addEventListener?.("disconnect", this.onDisconnect);
    this.serial = serial;
  }

  async open({ baud, parity = "none", dataBits = 8 }) {
    await this.stop();
    await this.port.open({ baudRate: baud, dataBits, parity, stopBits: 1, flowControl: "none", bufferSize: 4096 });
    this.reading = this.readLoop();
  }

  async readLoop() {
    while (this.port.readable && !this.stopping) {
      const reader = this.port.readable.getReader();
      this.reader = reader;
      try {
        for (;;) {
          const { value, done } = await reader.read();
          if (done) break;
          if (value?.length) this.emitData(value);
        }
      } catch (err) {
        if (this.stopping) break;
        if (!RECOVERABLE.has(err?.name)) {
          this.emitLost(`The serial port failed: ${err?.message ?? err}`);
          break;
        }
      } finally {
        reader.releaseLock();
        this.reader = null;
      }
    }
  }

  async write(bytes) {
    if (!this.port.writable) throw new Error("the serial port is not open");
    const writer = this.port.writable.getWriter();
    try {
      await writer.write(bytes);
    } finally {
      writer.releaseLock();
    }
  }

  /** Stop reading and close the port, if open. */
  async stop() {
    this.stopping = true;
    try {
      await this.reader?.cancel().catch(() => {});
      await this.reading?.catch(() => {});
      if (this.port.readable || this.port.writable) await this.port.close().catch(() => {});
    } finally {
      this.stopping = false;
      this.reading = null;
    }
  }

  async close() {
    await this.stop();
    this.serial?.removeEventListener?.("disconnect", this.onDisconnect);
  }
}

// ------------------------------------------------------------ the emulator

/**
 * The emulated calculator's serial port. Its speed is the emulated
 * calculator's own (IOPAR); the settings asked for are taken as they
 * come, as the bytes reach the emulated wire unframed.
 */
export class EmulatorLink extends Link {
  constructor(emulator) {
    super();
    this.emulator = emulator;
    this.label = "emulated serial port";
    this.opened = false;
    this.onSerial = (e) => this.emitData(e.detail.bytes);
    this.onGone = () => this.emitLost("The emulated calculator stopped.");
  }

  async open() {
    if (this.opened) return;
    this.emulator.addEventListener("serial", this.onSerial);
    this.emulator.addEventListener("gone", this.onGone);
    await this.emulator.request("serialOpen");
    this.opened = true;
  }

  async write(bytes) {
    // A full input (more than 16 KB waiting) clears at about 840 bytes a
    // second: wait and try again, for at most 30 s.
    for (let i = 0; ; i++) {
      try {
        await this.emulator.request("serialWrite", { bytes });
        return;
      } catch (err) {
        if (!/input is full/.test(err?.message ?? "") || i >= 300) throw err;
        await new Promise((r) => setTimeout(r, 100));
      }
    }
  }

  async close() {
    if (!this.opened) return;
    this.opened = false;
    this.emulator.removeEventListener("serial", this.onSerial);
    this.emulator.removeEventListener("gone", this.onGone);
    await this.emulator.request("serialClose").catch(() => {});
  }
}

// ------------------------------------------------------------ tests

/**
 * A trace's calculator: each `open` and `write` must be the trace's next
 * `=` or `>` step (else `failure` says how they differ, and the link is
 * lost), and the `<` steps after it arrive as long after it as they did
 * when recorded, divided by `speed`, or at the latest just before the
 * next step. `notes` are skipped; `done` once every step was played.
 */
export class FakeLink extends Link {
  constructor(trace, { speed = 1, timers = globalThis } = {}) {
    super();
    this.label = "test link";
    this.steps = (typeof trace === "string" ? parseTrace(trace) : trace).filter((s) => s.kind !== "note");
    this.next = 0;
    this.speed = speed;
    this.timers = timers;
    /** Reads on their way: timer, bytes (in order). */
    this.pending = new Map();
    this.failure = null;
    this.writes = 0;
  }

  get done() { return this.next >= this.steps.length && this.pending.size === 0; }

  fail(message) {
    this.failure ??= message;
    this.emitLost(message);
    throw new Error(message);
  }

  /**
   * Reads still on their way arrive now, in order: in the trace they came
   * before this step (an idle server's NAK while the page waited longer
   * than the recording did).
   */
  flush() {
    const due = [...this.pending.values()];
    for (const t of this.pending.keys()) this.timers.clearTimeout(t);
    this.pending.clear();
    for (const bytes of due) this.emitData(bytes);
  }

  /** Take the step expected now, then play the reads after it. */
  take(kind, check) {
    this.flush();
    const step = this.steps[this.next];
    if (!step || step.kind !== kind) this.fail(`step ${this.next}: expected ${step ? step.kind : "the end"}, got ${kind}`);
    const problem = check(step);
    if (problem) this.fail(`step ${this.next} (${kind} at ${step.at} ms): ${problem}`);
    this.next++;
    while (this.steps[this.next]?.kind === "read") {
      const read = this.steps[this.next++];
      const t = this.timers.setTimeout(() => {
        this.pending.delete(t);
        this.emitData(read.bytes);
      }, Math.max(read.at - step.at, 0) / this.speed);
      this.pending.set(t, read.bytes);
    }
  }

  async open({ baud, parity = "none" }) {
    this.take("open", (s) => (s.baud !== baud || s.parity !== parity ? `opened at ${baud} ${parity}, the trace at ${s.baud} ${s.parity}` : null));
  }

  async write(bytes) {
    this.writes++;
    this.take("write", (s) => (sameBytes(s.bytes, bytes) ? null : `wrote ${JSON.stringify(String.fromCharCode(...bytes))}, the trace ${JSON.stringify(String.fromCharCode(...s.bytes))}`));
  }

  async close() {
    for (const t of this.pending.keys()) this.timers.clearTimeout(t);
    this.pending.clear();
  }
}

/** Any link, its traffic kept as trace steps (`trace()` for the text). */
export class RecordingLink extends Link {
  constructor(inner, { now = () => performance.now() } = {}) {
    super();
    this.inner = inner;
    this.label = inner.label;
    this.now = now;
    this.start = now();
    this.steps = [];
    inner.addEventListener("data", (e) => {
      this.steps.push({ at: this.now() - this.start, kind: "read", bytes: new Uint8Array(e.detail) });
      this.emitData(e.detail);
    });
    inner.addEventListener("lost", (e) => this.emitLost(e.detail));
  }

  note(text) { this.steps.push({ kind: "note", text }); }

  async open(settings) {
    this.steps.push({ at: this.now() - this.start, kind: "open", baud: settings.baud, parity: settings.parity ?? "none" });
    await this.inner.open(settings);
  }

  async write(bytes) {
    this.steps.push({ at: this.now() - this.start, kind: "write", bytes: new Uint8Array(bytes) });
    await this.inner.write(bytes);
  }

  close() { return this.inner.close(); }

  /** The trace, with reads that came in pieces joined up to the next write. */
  trace(header = []) {
    const merged = [];
    for (const s of this.steps) {
      const last = merged[merged.length - 1];
      if (s.kind === "read" && last?.kind === "read" && s.at - last.at < 2) {
        last.bytes = new Uint8Array([...last.bytes, ...s.bytes]);
      } else merged.push({ ...s });
    }
    return formatTrace([...header.map((text) => ({ kind: "note", text })), ...merged]);
  }
}
