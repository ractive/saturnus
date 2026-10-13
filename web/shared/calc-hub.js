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
export const MIXED_BUILDS = "This page's files are from different versions. Reload the page.";

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
  if (locks) {
    await new Promise((granted) => {
      locks.request(lock, () => {
        granted();
        return new Promise((r) => {
          release = r;
        });
      }).catch(() => granted());
    });
  }
  port.postMessage({ v: 1, id: "join", cmd: "join", who, build, ...(locks ? { lock } : {}) });
  await answer;
  return { worker, port, release: () => release() };
}
