// The pages' way to the calculators' SharedWorker (web/shared-worker.js,
// web/hub.js, iteration 46): the emulator page and saturnus tx open it by
// the same URL, the emulator's folder's, so they share one live calculator
// per model. A page joins first: it says who it is and its build, and
// holds a Web Lock of its own for its whole life, which the hub asks for
// too and so learns when the page is gone (a SharedWorker hears no close
// of a port).

import { BUILD } from "./build.js";
import { siteHref } from "./site-links.js";

/**
 * What a page says when it and the SharedWorker are from two builds (the
 * worker of the other build lives while any tab of that build is open:
 * saturnus tx says so; the emulator page then runs on its own).
 */
export const MIXED_BUILDS = "Another saturnus tab is still on the previous version. Reload or close the other saturnus tabs, then try again.";

/**
 * What a page says when the calculators' worker stopped answering (it
 * crashed, or the calculator's engine failed): nothing more reaches it.
 */
export const CALC_STOPPED = "The calculator stopped; reload the page.";

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

/** What saturnus tx says where the browser cannot share a calculator (Samsung Internet). */
export const NO_SHARING = "This browser cannot share the emulator's calculator with saturnus tx. Use Chrome, Firefox or Safari for it, or choose a ROM file.";

/**
 * Whether this browser can share a calculator: SharedWorker, and Web
 * Locks, by which the hub learns that a page is gone (none on a page
 * served over plain HTTP).
 */
export function canShare(g = globalThis) {
  return typeof g.SharedWorker === "function" && Boolean(g.navigator?.locks);
}

/** The SharedWorker's URL from the page at `loc`: the emulator's folder (web/shared/site-links.js). */
export function hubUrl(loc = globalThis.location) {
  return new URL(`${siteHref("emulator", loc.pathname)}shared-worker.js`, loc.href);
}

/**
 * Open the SharedWorker and join as `who` ("emulator" or "tx"): resolves
 * to `{worker, port, release}` once joined (`release` lets go of the
 * page's lock); rejects with `MIXED_BUILDS` when the hub is of another
 * build. `onMessage` gets every message but the join's reply, from the
 * start.
 */
export async function joinHub(who, onMessage, { url = hubUrl(), locks = globalThis.navigator?.locks ?? null, build = BUILD } = {}) {
  const worker = new SharedWorker(url, { type: "module", name: "saturnus" });
  const port = worker.port;
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
  worker.onerror = (e) => joined.reject(new Error(e?.message || "the calculators' worker failed to start"));
  // The lock is held before the hub asks for it: granted to the hub only once this page is gone.
  const lock = `saturnus-page:${Date.now().toString(36)}:${Math.random().toString(36).slice(2)}`;
  let release = () => {};
  // Named to the hub only when this page holds it: a lock the browser
  // refused would be granted to the hub at once, which would take the
  // page for gone.
  let held = false;
  if (locks) {
    await new Promise((granted) => {
      locks.request(lock, () => {
        held = true;
        granted();
        return new Promise((r) => {
          release = r;
        });
      }).catch(() => granted());
    });
  }
  port.postMessage({ v: 1, id: "join", cmd: "join", who, build, ...(held ? { lock } : {}) });
  await answer;
  return { worker, port, release: () => release() };
}
