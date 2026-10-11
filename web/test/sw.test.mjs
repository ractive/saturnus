// The service worker (web/pwa/sw.js) against a fake worker global: a
// new build takes over only when the page asking is the only one open,
// it claims pages only on the first install, it deletes only its own
// scope's older caches (other deployments on the origin keep theirs), and
// it caches a build only when every file's SHA-256 matches FILES.
// The real thing in a browser: kb iteration 22's Outcome.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const SOURCE = readFileSync(new URL("../pwa/sw.js", import.meta.url), "utf8");

const sha256 = (text) => createHash("sha256").update(text).digest("hex");

/** sw.js run as `site.sh` ships it, under /saturnus/, with fake caches and clients; `served` maps a URL to the body the server sends (else a 404), `files` is FILES. */
function worker({ cacheNames = [], windows = [], served = {}, files = [["index.html", sha256("page")]] } = {}) {
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
  const stored = new Map();
  const caches = {
    keys: async () => [...names],
    delete: async (n) => { log.push(`delete ${n}`); return names.delete(n); },
    open: async (n) => {
      names.add(n);
      return { put: async (url, res) => { stored.set(url, await res.text()); } };
    },
  };
  const fetched = [];
  const fetch = async (req) => {
    fetched.push([req.url, req.cache]);
    return req.url in served ? new Response(served[req.url]) : new Response("not here", { status: 404 });
  };
  vm.runInNewContext(`const BUILD = "b2"; const FILES = ${JSON.stringify(files)};\n${SOURCE}`, { self, caches, URL, Request, Response, fetch, crypto: globalThis.crypto });
  /** Fire `type` with `data`; resolves once its waitUntil work is done. */
  const fire = async (type, extra = {}) => {
    let work = Promise.resolve();
    listeners[type]({ ...extra, waitUntil: (p) => { work = p; } });
    await work;
  };
  return { fire, log, names, stored, fetched };
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

test("install caches every file when each one's SHA-256 matches, fetched past the HTTP cache", async () => {
  const base = "https://example.org/saturnus/";
  const w = worker({
    files: [["index.html", sha256("page")], ["pkg/a.wasm", sha256("wasm")]],
    served: { [`${base}index.html`]: "page", [`${base}pkg/a.wasm`]: "wasm" },
  });
  await w.fire("install");
  assert.deepEqual([...w.names], ["saturnus:/saturnus/:b2"]);
  assert.deepEqual(Object.fromEntries(w.stored), { [`${base}index.html`]: "page", [`${base}pkg/a.wasm`]: "wasm" });
  assert.deepEqual(w.fetched.map(([, cache]) => cache), ["reload", "reload"]);
});

test("a file that is not this build's, or is missing, fails the install and caches nothing", async () => {
  const base = "https://example.org/saturnus/";
  // A deploy half way: the new worker, the old wasm.
  const mixed = worker({
    files: [["index.html", sha256("page")], ["pkg/a.wasm", sha256("new wasm")]],
    served: { [`${base}index.html`]: "page", [`${base}pkg/a.wasm`]: "old wasm" },
  });
  await assert.rejects(mixed.fire("install"), /pkg\/a\.wasm is not this build's/);
  assert.deepEqual([...mixed.names], [], "no cache made");
  assert.equal(mixed.stored.size, 0);

  const missing = worker({ files: [["index.html", sha256("page")], ["gone.js", sha256("x")]], served: { [`${base}index.html`]: "page" } });
  await assert.rejects(missing.fire("install"), /gone\.js: HTTP 404/);
  assert.equal(missing.stored.size, 0);
});
