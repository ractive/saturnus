// The protocol's driver: one `TxDevice` (saturnus-tx-web, the wasm
// bindings of saturnus-tx), fed the messages of the page, a clock and one
// timer at the deadline the device asks for, and what it gives back
// posted to the page. The dedicated Worker (protocol-worker.js) runs it
// with `performance.now()` and `setTimeout`; the tests run it with a fake
// clock (web/test/tx-driver.test.mjs).
//
// Messages from the page: `{cmd: "start", id, request, data?}`,
// `{cmd: "input", bytes}`, `{cmd: "linkError", message}`,
// `{cmd: "abort"}`. To the page: `{type: "open", baud, parity,
// dataBits}`, `{type: "write", bytes}`, `{type: "event", event}`,
// `{type: "result", id, ok, outcome, data?, error?}`, each after
// `{type: "connection", connection}`, the calculator connected then.

export class DeviceDriver {
  /**
   * `TxDevice` the wasm class; `now()` the clock in ms; `setTimer(fn,
   * ms)` / `clearTimer(handle)` one timer; `post(message)` to the page;
   * `seed` the device's seed (random by default); `listenMs` the speed
   * search's listening time per speed (the device's default when absent).
   */
  constructor({ TxDevice, now, setTimer, clearTimer, post, seed = null, listenMs = undefined }) {
    this.now = now;
    this.setTimer = setTimer;
    this.clearTimer = clearTimer;
    this.post = post;
    const s = seed ?? Math.floor(Math.random() * 2 ** 52);
    this.device = new TxDevice(now(), s, listenMs);
    this.timer = null;
    /** The id of the running request's page message. */
    this.running = null;
  }

  /** One message from the page. */
  handle(m) {
    const now = this.now();
    switch (m?.cmd) {
      case "start":
        try {
          this.device.start(now, JSON.stringify(m.request), m.data ?? undefined);
          this.running = m.id;
        } catch (err) {
          this.post({ type: "result", id: m.id, ok: false, error: { kind: "refused", message: String(err?.message ?? err) } });
          return;
        }
        break;
      case "input":
        this.device.input(now, m.bytes);
        break;
      case "linkError":
        this.device.linkError(now, String(m.message ?? "the link failed"));
        break;
      case "abort":
        if (this.running !== null) {
          this.device.abort(now);
          // The connection first, as with every result: a fast transfer stopped after the server ended has none.
          this.post({ type: "connection", connection: this.device.connection() });
          this.post({ type: "result", id: this.running, ok: false, error: { kind: "aborted", message: "stopped" } });
          this.running = null;
        }
        break;
      default:
        return;
    }
    this.deliver();
  }

  /** The timer fired. */
  fire() {
    this.timer = null;
    this.device.timeout(this.now());
    this.deliver();
  }

  /** Post what the device has, then arm its timer. */
  deliver() {
    for (const out of this.device.drain()) {
      if (out.type === "result") {
        const id = this.running;
        this.running = null;
        // The connection first: a page that has the result has it too.
        this.post({ type: "connection", connection: this.device.connection() });
        this.post({ ...out, id });
      } else if (out.type === "open" || out.type === "write") {
        this.post(out);
      } else {
        this.post({ type: "event", event: out });
      }
    }
    if (this.timer !== null) this.clearTimer(this.timer);
    this.timer = null;
    const at = this.device.deadline();
    if (at !== undefined) this.timer = this.setTimer(() => this.fire(), Math.max(at - this.now(), 0));
  }
}
