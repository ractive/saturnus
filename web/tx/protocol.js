// The page's side of the protocol Worker (protocol-worker.js): it holds
// the link (links.js), carries the link's bytes to the Worker and does
// what the Worker asks (open the port with these settings, write these
// bytes), in order, and turns each request into a promise. One request
// at a time: the device queues (kermit-device.js).
//
// Events: `progress`, `item`, `xmodem` (a fast transfer's plan) and
// `search` (`detail` the Worker's event),
// `connection` (`detail` the connected calculator or null), `lost`
// (`detail` why the link went away).

/**
 * A request that failed: `kind` as saturnus-tx-web's `error_json`
 * (`nameClash`, ...), the name it is about as `varName`, and what a
 * question needs (`count` of `notEmpty`, `names` of `namesInUse`, `needed`
 * and `free` of `noRoom`, `directories`, `variables`, `rootMade` and
 * `source` of `partial`).
 */
export class TxFailure extends Error {
  constructor({ kind = "calculator", message = "failed", name = null, ...rest } = {}) {
    super(message);
    this.kind = kind;
    this.varName = name;
    for (const key of ["count", "names", "needed", "free", "directories", "variables", "rootMade", "source"]) {
      if (rest[key] !== undefined) this[key] = rest[key];
    }
  }
}

/** The dedicated Worker the protocol runs in. */
export const protocolWorker = () => new Worker(new URL("./protocol-worker.js", import.meta.url), { type: "module" });

export class ProtocolClient extends EventTarget {
  /**
   * `link` a Link; `worker()` makes the Worker (tests pass one that runs
   * the driver in the page); `seed` fixes the device's seed (tests; a
   * random one otherwise); `listenMs` the speed search's time per speed.
   */
  constructor(link, { worker = protocolWorker, seed = null, listenMs = undefined } = {}) {
    super();
    this.link = link;
    this.worker = worker();
    this.nextId = 1;
    this.waiting = new Map();
    this.connection = null;
    this.closed = false;
    /** The link's operations, one after the other. */
    this.io = Promise.resolve();
    this.ready = new Promise((resolve, reject) => {
      this.readyWaiter = { resolve, reject };
    });
    this.ready.catch(() => {});
    this.worker.onmessage = (e) => this.receive(e.data);
    this.worker.onerror = (e) => this.fatal(e.message || "the protocol worker failed to start");
    this.onData = (e) => this.worker.postMessage({ cmd: "input", bytes: e.detail });
    this.onLost = (e) => {
      this.worker.postMessage({ cmd: "linkError", message: String(e.detail) });
      this.dispatchEvent(new CustomEvent("lost", { detail: String(e.detail) }));
    };
    link.addEventListener("data", this.onData);
    link.addEventListener("lost", this.onLost);
    this.worker.postMessage({ cmd: "init", seed, listenMs });
  }

  failAll(message, kind = "failed") {
    this.readyWaiter.reject(new Error(message));
    for (const w of this.waiting.values()) w.reject(new TxFailure({ kind, message }));
    this.waiting.clear();
  }

  /** The Worker failed: nothing more goes through it; the device is gone. */
  fatal(message) {
    this.failAll(message);
    if (this.closed) return;
    this.closed = true;
    this.dispatchEvent(new CustomEvent("lost", { detail: `the transfer part of the page stopped (${message})` }));
  }

  receive(m) {
    switch (m?.type) {
      case "ready":
        this.readyWaiter.resolve();
        break;
      case "failed":
        this.fatal(m.message);
        break;
      case "open":
        this.linkDo(() => this.link.open({ baud: m.baud, parity: m.parity, dataBits: m.dataBits }));
        break;
      case "write":
        this.linkDo(() => this.link.write(m.bytes));
        break;
      case "event":
        this.dispatchEvent(new CustomEvent(["progress", "item", "xmodem"].includes(m.event.type) ? m.event.type : "search", { detail: m.event }));
        break;
      case "connection":
        this.connection = m.connection ?? null;
        this.dispatchEvent(new CustomEvent("connection", { detail: this.connection }));
        break;
      case "result": {
        const w = this.waiting.get(m.id);
        if (!w) return;
        this.waiting.delete(m.id);
        // The link's writes of the request are done before it resolves.
        this.io.then(() => (m.ok ? w.resolve({ ...m.outcome, data: m.data }) : w.reject(new TxFailure(m.error))));
        break;
      }
      default:
    }
  }

  /** Run a link operation after those before it; a failure is the link's error for the device. */
  linkDo(fn) {
    this.io = this.io.then(fn).catch((err) => {
      if (!this.closed) this.worker.postMessage({ cmd: "linkError", message: String(err?.message ?? err) });
    });
  }

  /**
   * Run `request` (saturnus-tx-web's `parse_request`: `connect`, `list`,
   * `get`, `put`, `mkdir`, `rename`, `purge`, `get-tree`, `put-tree`,
   * `memory`, `backup`, `restore`, `pict`, `xmodem-put`, `xmodem-get`,
   * `disconnect`; `data` a put's
   * or a restore's bytes, a tree put's items): resolves to the outcome
   * (`{type, ...}`, a file's bytes as `data`), rejects with a TxFailure.
   */
  async request(request, data = undefined) {
    await this.ready;
    if (this.closed) throw new TxFailure({ kind: "closed", message: "the link is closed" });
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      this.worker.postMessage({ cmd: "start", id, request, data });
    });
  }

  /** Drop the running request (its promise rejects with kind `aborted`). */
  abort() { this.worker.postMessage({ cmd: "abort" }); }

  /** Stop the Worker and close the link. */
  async close() {
    if (this.closed) return;
    this.closed = true;
    this.link.removeEventListener("data", this.onData);
    this.link.removeEventListener("lost", this.onLost);
    this.failAll("closed", "closed");
    this.worker.terminate?.();
    await this.io.catch(() => {});
    await this.link.close().catch(() => {});
  }
}
