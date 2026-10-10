// The kill switch (web/landing/sw.js, shipped at /saturnus/sw.js) against
// a fake worker global: it takes over at install, deletes only the caches
// of the old worker at /saturnus/ (the emulator's `/saturnus/emulator/`
// caches and anything else stay), unregisters itself, and reloads only
// the windows it controls, each at its own URL. It has no fetch handler.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const SOURCE = readFileSync(new URL("../landing/sw.js", import.meta.url), "utf8");

/** The kill switch run at https://example.org/saturnus/sw.js with fake caches and clients. */
function worker({ cacheNames = [], windows = [] } = {}) {
  const listeners = {};
  const log = [];
  const names = new Set(cacheNames);
  const self = {
    location: { href: "https://example.org/saturnus/sw.js" },
    addEventListener: (type, fn) => { listeners[type] = fn; },
    skipWaiting: async () => { log.push("skipWaiting"); },
    registration: { unregister: async () => { log.push("unregister"); return true; } },
    clients: {
      // The default is the controlled windows only: the switch must not
      // ask for the uncontrolled ones.
      matchAll: async (opts) => {
        // (The object comes from the worker's realm: no deepEqual.)
        assert.equal(opts?.type, "window");
        assert.deepEqual(Object.keys(opts), ["type"]);
        return windows;
      },
      claim: async () => { log.push("claim"); },
    },
  };
  const caches = {
    keys: async () => [...names],
    delete: async (n) => { log.push(`delete ${n}`); return names.delete(n); },
  };
  vm.runInNewContext(SOURCE, { self, caches, URL });
  const fire = async (type) => {
    let work = Promise.resolve();
    listeners[type]({ waitUntil: (p) => { work = p; } });
    await work;
  };
  return { fire, log, names, listeners };
}

const page = (url, { refuses = false } = {}) => {
  const nav = [];
  return { url, nav, navigate: async (u) => { if (refuses) throw new Error("not allowed"); nav.push(u); } };
};

test("install takes over at once; there is no fetch handler", async () => {
  const w = worker();
  await w.fire("install");
  assert.deepEqual(w.log, ["skipWaiting"]);
  assert.equal(w.listeners.fetch, undefined);
});

test("activate deletes only the old worker's caches, unregisters, and reloads the controlled windows", async () => {
  const a = page("https://example.org/saturnus/");
  const b = page("https://example.org/saturnus/?x=1", { refuses: true });
  const w = worker({
    cacheNames: ["saturnus:/saturnus/:0123456789abcdef", "saturnus:/saturnus/:fedcba9876543210", "saturnus:/saturnus/emulator/:0123456789abcdef", "other-site:/saturnus/:1", "saturnus:/saturnus"],
    windows: [a, b],
  });
  await w.fire("activate");
  assert.deepEqual(w.log, ["delete saturnus:/saturnus/:0123456789abcdef", "delete saturnus:/saturnus/:fedcba9876543210", "unregister"]);
  assert.deepEqual([...w.names], ["saturnus:/saturnus/emulator/:0123456789abcdef", "other-site:/saturnus/:1", "saturnus:/saturnus"], "the emulator's and others' caches stay");
  assert.deepEqual(a.nav, ["https://example.org/saturnus/"], "each window at its own URL");
  // A window that refuses to navigate does not stop the rest.
  assert.deepEqual(b.nav, []);
});

test("the switch never claims pages, and works with no caches and no windows", async () => {
  const w = worker();
  await w.fire("activate");
  assert.deepEqual(w.log, ["unregister"]);
});

test("the source names its removal date", () => {
  assert.match(SOURCE, /Remove on 2027-01-15/);
});
