// A calculator in Kermit server mode on any link, as a Device
// (web/shared/device.js): connect, list, read (get), write (put),
// disconnect, over the protocol Worker (protocol.js). One operation at a
// time, as the link is half duplex; more wait in a queue (`queued`), and
// those not started yet can be cancelled. Listings are kept until the
// page changes that directory or asks again (`refresh`); a change made on
// the calculator's keyboard is not seen until then (the server is busy
// serving).
//
// Events: `change` (`{path}`; no path: everything), `busy` (`{label,
// queued}`, label null when idle), `progress` (`{label, done, total}`),
// `search` (the speed search: `{type: "listening" | "found", settings}`),
// `connection` (the calculator or null), `lost` (why).

import { HOME, pathKey } from "../shared/device.js";
import { TxFailure } from "./protocol.js";

export class KermitDevice extends EventTarget {
  /** `client` a ProtocolClient; `title` the place's name ("HP 48GX on USB serial port"). */
  constructor(client, { title = "Calculator", emulated = false } = {}) {
    super();
    this.client = client;
    this.title = title;
    this.emulated = emulated;
    this.listings = new Map();
    this.queue = [];
    this.running = null;
    this.lost = null;
    this.forward = (type) => (e) => this.dispatchEvent(new CustomEvent(type, { detail: e.detail }));
    client.addEventListener("search", this.forward("search"));
    client.addEventListener("connection", this.forward("connection"));
    client.addEventListener("lost", (e) => {
      this.lost = e.detail;
      this.dispatchEvent(new CustomEvent("lost", { detail: e.detail }));
    });
    client.addEventListener("progress", (e) => this.progress(e.detail));
  }

  /** The connected calculator (`{model, settings, version, userMode}`) or null. */
  get connection() { return this.client.connection; }

  /** How many operations wait behind the running one. */
  get queued() { return this.queue.length; }

  busyEvent() {
    this.dispatchEvent(new CustomEvent("busy", { detail: { label: this.running?.label ?? null, queued: this.queue.length } }));
  }

  /** Progress of the running operation: bytes so far of the expected total. */
  progress(e) {
    const op = this.running;
    if (!op) return;
    if (e.total !== undefined) {
      op.done = e.sent;
      op.total = e.total;
    } else if (e.received !== undefined) {
      op.done = (op.done ?? 0) + e.received;
    }
    this.dispatchEvent(new CustomEvent("progress", { detail: { label: op.label, done: op.done ?? 0, total: op.total ?? null } }));
  }

  /** Run `fn` when the operations before it are done; `label` says what it is ("Getting PRG…"). */
  run(label, fn, { total = null } = {}) {
    return new Promise((resolve, reject) => {
      this.queue.push({ label, fn, resolve, reject, total });
      this.busyEvent();
      this.pump();
    });
  }

  async pump() {
    if (this.running || !this.queue.length) return;
    const op = this.queue.shift();
    this.running = op;
    this.busyEvent();
    try {
      op.resolve(await op.fn());
    } catch (err) {
      op.reject(err);
    } finally {
      this.running = null;
      this.busyEvent();
      this.pump();
    }
  }

  /** Drop the operations not started yet (each rejects as `cancelled`). */
  cancelQueued() {
    const dropped = this.queue.splice(0);
    for (const op of dropped) op.reject(new TxFailure({ kind: "cancelled", message: "cancelled" }));
    this.busyEvent();
    return dropped.length;
  }

  /** Find the calculator and connect: `{find, baud, parity}`. */
  connect(options = {}) {
    return this.run("Looking for the calculator…", async () => {
      const r = await this.client.request({ type: "connect", find: options.find ?? true, baud: options.baud ?? 9600, parity: options.parity ?? "none" });
      return r.connection;
    });
  }

  /** The listing of `path`: kept, or read (once, however many ask). */
  list(path = HOME) {
    const key = pathKey(path);
    if (!this.listings.has(key)) {
      const p = this.run(`Reading ${path[path.length - 1]}…`, () => this.client.request({ type: "list", dir: path }));
      this.listings.set(key, p);
      p.catch(() => {
        if (this.listings.get(key) === p) this.listings.delete(key);
      });
    }
    return this.listings.get(key);
  }

  /** Forget the listing of `path` (none: all) and say so. */
  refresh(path = null) {
    if (path) this.listings.delete(pathKey(path));
    else this.listings.clear();
    this.dispatchEvent(new CustomEvent("change", { detail: { path: path ? [...path] : null } }));
  }

  /** Variable `name` of `path`: `{name, fileName, format, data}`; `text` asks for the calculator's text form. `size` (bytes) for the progress. */
  read(path, name, { text = false, size = null } = {}) {
    const label = `Getting ${name}${text ? " as text" : ""}…`;
    return this.run(label, async () => {
      const r = await this.client.request({ type: "get", dir: path, name, format: text ? "text" : "binary" });
      return { name: r.name, fileName: r.fileName, format: r.format, data: r.data };
    }, { total: size });
  }

  /**
   * Put `bytes` into variable `name` of `path`; `replace` a variable of
   * that name, `asString` store a file that is no object as a string.
   * Resolves to the name stored; the directory's listing is read again.
   */
  write(path, name, bytes, { replace = false, asString = false } = {}) {
    return this.run(`Putting ${name}…`, async () => {
      const r = await this.client.request({ type: "put", dir: path, name, replace, asString }, bytes);
      this.refresh(path);
      return r.name;
    }, { total: bytes.length });
  }

  /** Put flag -35 back and end server mode (FINISH). */
  disconnect() {
    this.cancelQueued();
    return this.run("Ending server mode…", () => this.client.request({ type: "disconnect" }));
  }

  /** Close the link and the Worker. */
  close() {
    this.cancelQueued();
    return this.client.close();
  }
}
