// The service worker (web/pwa/sw.js) against a fake worker global: a
// new build takes over only when the page asking is the only one open,
// it claims pages only on the first install, and it deletes only its own
// scope's older caches (other deployments on the origin keep theirs).
// The real thing in a browser: kb iteration 22's Outcome.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const SOURCE = readFileSync(new URL("../pwa/sw.js", import.meta.url), "utf8");

/** sw.js run as `site.sh` ships it, under /saturnus/, with fake caches and clients. */
function worker({ cacheNames = [], windows = [] } = {}) {
  const listeners = {};
  const log = [];
  const names = new Set(cacheNames);
  const self = {
    location: { href: "https://example.org/saturnus/sw.js" },
    addEventListener: (type, fn) => { listeners[type] = fn; },
    skipWaiting: async () => { log.push("skipWaiting"); },
    clients: {
      matchAll: async (opts) => { assert.equal(opts?.type, "window"); return windows; },
      claim: async () => { log.push("claim"); },
    },
  };
  const caches = {
    keys: async () => [...names],
    delete: async (n) => { log.push(`delete ${n}`); return names.delete(n); },
  };
  vm.runInNewContext(`const BUILD = "b2"; const FILES = ["index.html"];\n${SOURCE}`, { self, caches, URL, Request: class {}, fetch: () => {} });
  /** Fire `type` with `data`; resolves once its waitUntil work is done. */
  const fire = async (type, extra = {}) => {
    let work = Promise.resolve();
    listeners[type]({ ...extra, waitUntil: (p) => { work = p; } });
    await work;
  };
  return { fire, log, names };
}

const page = (id) => {
  const got = [];
  return { id, got, postMessage: (m) => got.push(JSON.parse(JSON.stringify(m))) };
};

test("a page alone gets the new build; with others open it is told to wait", async () => {
  const a = page("a");
  const alone = worker({ windows: [a] });
  await alone.fire("message", { data: { type: "skip" }, source: a });
  assert.deepEqual(alone.log, ["skipWaiting"]);
  assert.deepEqual(a.got, []);

  const b = page("b");
  const c = page("c");
  const busy = worker({ windows: [b, c] });
  await busy.fire("message", { data: { type: "skip" }, source: c });
  assert.deepEqual(busy.log, [], "tab b keeps its build");
  assert.deepEqual(c.got, [{ type: "others" }]);
});

test("the first install claims the open pages; an update claims none", async () => {
  const first = worker({ cacheNames: ["saturnus:/saturnus/:b2"] });
  await first.fire("activate");
  assert.deepEqual(first.log, ["claim"]);

  const update = worker({ cacheNames: ["saturnus:/saturnus/:b1", "saturnus:/saturnus/:b2"] });
  await update.fire("activate");
  assert.deepEqual(update.log, ["delete saturnus:/saturnus/:b1"]);
});

test("only this scope's older caches go; other deployments on the origin keep theirs", async () => {
  const w = worker({ cacheNames: ["saturnus:/saturnus/:b1", "saturnus:/saturnus/:b2", "saturnus:/preview/:b9", "saturnus:/saturnus/x/:b1", "other"] });
  await w.fire("activate");
  assert.deepEqual([...w.names].sort(), ["other", "saturnus:/preview/:b9", "saturnus:/saturnus/:b2", "saturnus:/saturnus/x/:b1"]);
});
