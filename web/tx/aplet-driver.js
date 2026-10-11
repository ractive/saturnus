// The aplet server's driver: one `TxApletServer` (saturnus-tx-web, the
// folder served to a 38G, 39G or 40G), fed the page's messages, a clock
// and one timer at the deadline it asks for, and what it gives back
// posted to the page. The dedicated Worker (aplet-worker.js) runs it with
// `performance.now()` and `setTimeout`; the tests with a fake clock.
//
// Messages from the page: `{cmd: "input", bytes}`, `{cmd: "provide",
// data?}` (the answer to a `needFile`), `{cmd: "files", files}`,
// `{cmd: "sendWire", data}`, `{cmd: "cancel"}`. To the page: `{type:
// "write", bytes}` and the server's events (`model`, `needFile`, ...,
// `received` with `data`), and `{type: "refused", message}` for a call it
// refused.

export class ApletDriver {
  /** `TxApletServer` the wasm class; `files` the folder's `[{name, head}]`; the rest as DeviceDriver's. */
  constructor({ TxApletServer, now, setTimer, clearTimer, post, files = [] }) {
    this.now = now;
    this.setTimer = setTimer;
    this.clearTimer = clearTimer;
    this.post = post;
    this.server = new TxApletServer(now(), files);
    this.timer = null;
  }

  /** One message from the page. */
  handle(m) {
    const now = this.now();
    try {
      switch (m?.cmd) {
        case "input":
          this.server.input(now, m.bytes);
          break;
        case "provide":
          this.server.provide(now, m.data ?? undefined);
          break;
        case "files":
          this.server.setFiles(m.files ?? []);
          break;
        case "sendWire":
          this.server.sendWire(now, m.data);
          break;
        case "cancel":
          this.server.cancel(now);
          break;
        default:
          return;
      }
    } catch (err) {
      this.post({ type: "refused", cmd: m.cmd, message: String(err?.message ?? err) });
    }
    this.deliver();
  }

  /** The timer fired. */
  fire() {
    this.timer = null;
    this.server.timeout(this.now());
    this.deliver();
  }

  /** Post what the server has, then arm its timer. */
  deliver() {
    for (const out of this.server.drain(this.now())) this.post(out);
    if (this.timer !== null) this.clearTimer(this.timer);
    this.timer = null;
    const at = this.server.deadline();
    if (at !== undefined) this.timer = this.setTimer(() => this.fire(), Math.max(at - this.now(), 0));
  }
}
