// The views' way to the calculators' hub (web/hub.js in the plain module
// Worker web/worker.js, iteration 50d): one Worker for the document,
// started by the first view that needs it, and one MessagePort per view
// (`hubPort`), so the Calculator view and Files share one live calculator
// per model. A view joins first and says who it is (`join {who}`). The
// connections live for the document.

/**
 * What a page says when the calculators' worker stopped answering (it
 * crashed, or the calculator's engine failed): nothing more reaches it.
 */
export const CALC_STOPPED = "The calculator stopped; reload the page.";

/** What a view hears when it opens the serial port of a calculator another view shares (web/hub.js, web/native-hub.js). */
export const SERIAL_SHARED = "This calculator is shared with another view, so its serial port cannot be opened here. Use a calculator of its own (a ROM file chosen in Files).";

/** How often a page with requests waiting asks the hub whether it still runs (`ping`). */
export const HEARTBEAT_MS = 5000;

/**
 * A page's watch on the hub while it waits for replies: every `every` ms
 * with `busy()` true it sends `ping()` (a promise of the hub's answer);
 * one still unanswered at the next turn, or refused, calls `stopped()`
 * once. Long commands (a send typing, a write, a transfer) keep their
 * own time: the hub answers pings between them. Idle, it sets no timer.
 */
export class Heartbeat {
  constructor({ ping, busy, stopped, every = HEARTBEAT_MS, timers = globalThis }) {
    this.ping = ping;
    this.busy = busy;
    this.stopped = stopped;
    this.every = every;
    this.timers = timers;
    this.timer = 0;
    this.waiting = false;
    this.dead = false;
  }

  /** A request went out: watch until none waits. */
  kick() {
    if (this.timer || this.dead) return;
    this.timer = this.timers.setTimeout(() => this.turn(), this.every);
    // Node's tests: the watch alone does not keep the process running.
    this.timer?.unref?.();
  }

  turn() {
    this.timer = 0;
    if (this.dead) return;
    if (this.waiting) {
      this.die();
      return;
    }
    if (!this.busy()) return;
    this.waiting = true;
    Promise.resolve()
      .then(() => this.ping())
      .then(() => {
        this.waiting = false;
      }, () => this.die());
    this.kick();
  }

  die() {
    if (this.dead) return;
    this.dead = true;
    this.timers.clearTimeout(this.timer);
    this.timer = 0;
    this.stopped();
  }

  /** No more watching (the page let go). */
  stop() {
    this.dead = true;
    this.timers.clearTimeout(this.timer);
    this.timer = 0;
  }
}

/** The document's hub Worker, once a view asked for it (`hubWorker`). */
let worker = null;

/** The hub's Worker, started on the first call (web/worker.js). */
export function hubWorker() {
  worker ??= new Worker(new URL("../worker.js", import.meta.url), { type: "module", name: "saturnus" });
  return worker;
}

/** Whether the hub's Worker runs (no Worker: no calculator runs either; the home page). */
export function hubRunning() {
  return worker !== null;
}

/** A new connection to the hub: a MessagePort whose other end the Worker has (one per view). */
export function hubPort() {
  const channel = new MessageChannel();
  hubWorker().postMessage({ connect: true }, [channel.port2]);
  return channel.port1;
}

/**
 * Connect to the hub and join as `who` ("emulator", "tx" or "home"):
 * resolves to `{worker, port}` once joined. `onMessage` gets every
 * message but the join's reply, from the start.
 */
export async function joinHub(who, onMessage, { open = hubPort, worker: w = () => hubWorker() } = {}) {
  const port = open();
  let joined;
  const answer = new Promise((resolve, reject) => {
    joined = { resolve, reject };
  });
  port.onmessage = (e) => {
    const m = e.data;
    if (m?.type === "reply" && m.id === "join") {
      if (m.ok) joined.resolve(m.result);
      else joined.reject(new Error(m.error));
      return;
    }
    onMessage(m);
  };
  const theWorker = w();
  const failed = (e) => joined.reject(new Error(e?.message || "the calculators' worker failed to start"));
  theWorker?.addEventListener?.("error", failed);
  port.postMessage({ v: 1, id: "join", cmd: "join", who });
  try {
    await answer;
  } finally {
    theWorker?.removeEventListener?.("error", failed);
  }
  return { worker: theWorker, port };
}

/**
 * A view's connection to the calculators' hub (Files' way to the
 * Calculator view's calculator, iteration 47; the home page's): `join`,
 * then the protocol's commands (`request(cmd, args)`, web/protocol.md)
 * and its events as DOM events of their `type` (`frame`, `status`,
 * `memoryChanged`, ...). `gone` (detail: why) when the hub stopped
 * answering (the heartbeat), its worker failed, or the hub said so (the
 * Calculator view started the calculator afresh, its ROM removed).
 * `close` leaves the calculator (the Calculator view goes on showing it).
 */
export class HubClient extends EventTarget {
  /** `who` the view ("tx", "home"); `join` the hub's way in (tests give their own). */
  constructor({ who = "tx", join = joinHub } = {}) {
    super();
    this.nextId = 1;
    this.waiting = new Map();
    this.pings = new Map();
    this.closed = false;
    this.heartbeat = new Heartbeat({
      ping: () => this.ping(),
      busy: () => this.waiting.size > 0,
      stopped: () => this.fail(CALC_STOPPED),
    });
    this.port = null;
    this.worker = null;
    this.onWorkerError = (e) => this.fail(e?.message || "the calculators' worker failed");
    this.joined = join(who, (m) => this.receive(m)).then((j) => {
      this.port = j.port;
      this.worker = j.worker ?? null;
      this.worker?.addEventListener?.("error", this.onWorkerError);
    });
    this.joined.catch(() => {});
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
    if (m?.type === "fatal" || m?.type === "gone") {
      this.fail(m.message);
      return;
    }
    if (m?.type) this.dispatchEvent(new CustomEvent(m.type, { detail: m }));
  }

  /** The hub cannot go on: every waiting request fails, and `gone`. */
  fail(message) {
    this.heartbeat.stop();
    for (const w of [...this.waiting.values(), ...this.pings.values()]) w.reject(new Error(message));
    this.waiting.clear();
    this.pings.clear();
    this.dispatchEvent(new CustomEvent("gone", { detail: message }));
  }

  /** The command `cmd` with `args`; resolves to its result, rejects with its error. */
  request(cmd, args = {}) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      if (this.closed) {
        reject(new Error("closed"));
        return;
      }
      this.waiting.set(id, { resolve, reject });
      this.joined.then(
        () => {
          // Watched once joined: a slow join is no stopped worker.
          if (!this.closed) this.heartbeat.kick();
          this.port.postMessage({ v: 1, id, cmd, ...args });
        },
        (err) => {
          this.waiting.delete(id);
          reject(err);
        },
      );
    });
  }

  /** The heartbeat's question (web/protocol.md, `ping`), answered by the hub at once. */
  ping() {
    const id = `ping:${this.nextId++}`;
    return new Promise((resolve, reject) => {
      this.pings.set(id, { resolve, reject });
      this.joined.then(
        () => this.port.postMessage({ v: 1, id, cmd: "ping" }),
        (err) => {
          this.pings.delete(id);
          reject(err);
        },
      );
    });
  }

  /** Leave the calculator and the hub; the Calculator view goes on showing it. */
  async close() {
    if (this.closed) return;
    const leave = this.request("leave").catch(() => {});
    this.closed = true;
    this.heartbeat.stop();
    // Longer than the hub's own wait for a send to end (20 s).
    let timer = 0;
    await Promise.race([leave, new Promise((r) => {
      timer = setTimeout(r, 25_000);
    })]);
    clearTimeout(timer);
    for (const w of this.waiting.values()) w.reject(new Error("closed"));
    this.waiting.clear();
    this.port?.close?.();
    this.worker?.removeEventListener?.("error", this.onWorkerError);
  }
}
