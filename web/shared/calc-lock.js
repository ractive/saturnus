// One page runs a calculator at a time (iteration 41). The emulator and
// saturnus tx share one calculator per model (its auto-saved state,
// web/shared/calc-states.js); the Worker that runs it holds the model's
// Web Lock (`navigator.locks`, one lock per model) for as long as it does.
//
// A Worker that wants a calculator another one holds asks for it over a
// BroadcastChannel (`want`). The holder answers at once (`answer`), lets
// its page wrap up (`onWanted`: saturnus tx leaves SERVER, the Worker
// saves the calculator and stops it), then lets go of the lock, and the
// asker gets it. If nobody answers within `askMs` (a frozen background
// tab), or the holder does not let go within `handoverMs`, the asker
// takes the lock with `steal`; the old holder hears of it (`onLost`) and
// stops, and its writes are refused by the holder record anyway.
//
// Plain JavaScript over injected `locks` and `channel` (the browser's in a
// Worker, fakes in web/test/calc-lock.test.mjs).

/** The BroadcastChannel both pages talk on. */
export const CHANNEL = "saturnus-calculators";
/** The Web Lock of `model`'s calculator. */
export const lockName = (model) => `saturnus-calculator:${model}`;
/** How long an asker waits for any answer before it takes the lock. */
export const ASK_MS = 3000;
/** How long it waits, once answered, for the holder to let go. */
export const HANDOVER_MS = 60_000;

/** A model's calculator is in use elsewhere and was not asked for: the Worker's error text, which the pages know. */
export const IN_USE = "in use elsewhere";

/** The words for who runs a calculator, by `who` ("emulator", "tx"), as the other page shows them. */
export function inUseText(by, here) {
  if (by === "tx") return here === "tx" ? "In use on the other side or in another saturnus tx tab." : "In use by saturnus tx.";
  if (by === "emulator") return here === "emulator" ? "In use in another emulator tab." : "In use by the emulator.";
  return "In use in another tab.";
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export class CalcLock {
  /**
   * `who` names this side ("emulator" or "tx"); `onWanted(model, by)` wraps
   * up before the lock goes to `by` (resolves when done); `onLost(model)`
   * says the lock was taken without asking.
   */
  constructor({
    who,
    locks = globalThis.navigator?.locks ?? null,
    channel = typeof BroadcastChannel === "function" ? new BroadcastChannel(CHANNEL) : null,
    onWanted = async () => {},
    onLost = () => {},
    askMs = ASK_MS,
    handoverMs = HANDOVER_MS,
  }) {
    this.who = who;
    this.locks = locks;
    this.channel = channel;
    this.onWanted = onWanted;
    this.onLost = onLost;
    this.askMs = askMs;
    this.handoverMs = handoverMs;
    /** model -> the hold (`{release}`). */
    this.held = new Map();
    /** Models being handed over now. */
    this.handing = new Set();
    /** Answers awaited, by request id. */
    this.answers = new Map();
    this.nextId = 1;
    this.prefix = Math.random().toString(36).slice(2);
    if (this.channel) this.channel.onmessage = (e) => this.heard(e.data);
    // Node's channel would keep a test's process alive (a browser's has no unref).
    this.channel?.unref?.();
  }

  /** Without Web Locks (an old browser) every calculator is this page's: as before iteration 41. */
  get solo() { return !this.locks; }

  holds(model) {
    return this.solo || this.held.has(model);
  }

  heard(m) {
    if (!m || typeof m !== "object") return;
    if (m.type === "answer") {
      this.answers.get(m.id)?.();
      return;
    }
    if (m.type !== "want" || !this.held.has(m.model)) return;
    this.channel.postMessage({ type: "answer", id: m.id, model: m.model, who: this.who });
    if (this.handing.has(m.model)) return;
    this.handing.add(m.model);
    Promise.resolve()
      .then(() => this.onWanted(m.model, m.who ?? null))
      .catch((err) => console.warn(`saturnus: handing over the ${m.model}: ${err?.message ?? err}`))
      .finally(() => {
        this.handing.delete(m.model);
        this.release(m.model);
      });
  }

  /**
   * Request the lock with `options`; resolves to the hold, or null when
   * `ifAvailable` found it taken. Rejects when the request was aborted.
   */
  hold(model, options) {
    return new Promise((resolve, reject) => {
      let granted = null;
      this.locks
        .request(lockName(model), options, (lock) => {
          if (!lock) {
            resolve(null);
            return undefined;
          }
          let release;
          const until = new Promise((r) => {
            release = r;
          });
          granted = { release };
          resolve(granted);
          return until;
        })
        .then(
          () => {},
          (err) => {
            if (!granted) reject(err);
            else this.lostHold(model, granted);
          },
        );
    });
  }

  /** The lock of a hold was stolen. */
  lostHold(model, hold) {
    if (this.held.get(model) !== hold) return;
    this.held.delete(model);
    this.onLost(model);
  }

  take(model, hold) {
    this.held.set(model, hold);
    return true;
  }

  /**
   * Get `model`'s lock. Taken elsewhere: without `ask` resolves to false;
   * with it, asks the holder and waits (taking the lock from a holder that
   * does not answer). Resolves to true once held.
   */
  async acquire(model, { ask = true } = {}) {
    if (this.holds(model)) return true;
    const free = await this.hold(model, { ifAvailable: true });
    if (free) return this.take(model, free);
    if (!ask) return false;
    const id = `${this.prefix}:${this.nextId++}`;
    let answered = false;
    this.answers.set(id, () => {
      answered = true;
    });
    const ctrl = new AbortController();
    const waiting = this.hold(model, { signal: ctrl.signal });
    waiting.catch(() => {});
    let got = null;
    waiting.then((h) => {
      got = h;
    }, () => {});
    try {
      this.channel?.postMessage({ type: "want", id, model, who: this.who });
      const step = 50;
      for (let t = 0; !got; t += step) {
        if (t >= (answered ? this.handoverMs : this.askMs)) break;
        await sleep(step);
      }
    } finally {
      this.answers.delete(id);
    }
    if (got) return this.take(model, got);
    ctrl.abort();
    const stolen = await this.hold(model, { steal: true });
    // Granted just before the abort: that hold lost to ours, and is let go.
    if (got) got.release();
    return this.take(model, stolen);
  }

  /** Let go of `model`'s lock. */
  release(model) {
    const h = this.held.get(model);
    if (!h) return;
    this.held.delete(model);
    h.release();
  }

  /** The models held. */
  models() {
    return [...this.held.keys()];
  }
}
