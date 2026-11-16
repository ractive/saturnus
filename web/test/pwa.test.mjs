// The installed page's parts in the page (web/pwa.js): where the service
// worker may be registered, the screen kept on through a long computation
// only, and persistent storage asked for in context (never on load). The
// notice and the ROMs panel's button are checked in a browser by
// overflow.test.mjs. The worker
// itself and its cache are checked against a built site in
// site.test.mjs, and in a browser by hand (kb iteration 22).
import { test } from "node:test";
import assert from "node:assert/strict";
import { Store } from "../store.js";
import { LOOK_EVERY_MS, StorageChoice, keepScreenOnWhileComputing, lookOnReturn, ownController, serviceWorkerAllowed, watchUpdates } from "../pwa.js";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const nav = { serviceWorker: {} };

test("the service worker only over HTTPS or on this computer, never in the desktop app", () => {
  const at = (href) => new URL(href);
  assert.equal(serviceWorkerAllowed("worker", at("https://ractive.ch/saturnus/app/"), nav), true);
  assert.equal(serviceWorkerAllowed("worker", at("http://localhost:8000/"), nav), true);
  assert.equal(serviceWorkerAllowed("worker", at("http://127.0.0.1:8000/"), nav), true);
  assert.equal(serviceWorkerAllowed("worker", at("http://example.org/"), nav), false);
  assert.equal(serviceWorkerAllowed("worker", at("https://ractive.ch/"), {}), false, "no service workers in this browser");
  // The app's pages: tauri://localhost (macOS, Linux), http://tauri.localhost (Windows).
  assert.equal(serviceWorkerAllowed("tauri", at("tauri://localhost/"), nav), false);
  assert.equal(serviceWorkerAllowed("tauri", at("http://tauri.localhost/"), nav), false);
  assert.equal(serviceWorkerAllowed("tauri", at("https://localhost/"), nav), false);
});

test("only the page's own sw.js counts as its controller: not a worker of a wider scope", () => {
  const at = new URL("https://ractive.ch/saturnus/app/");
  const own = { scriptURL: "https://ractive.ch/saturnus/app/sw.js" };
  assert.equal(ownController(own, at), own);
  assert.equal(ownController({ scriptURL: "https://ractive.ch/saturnus/sw.js" }, at), null, "a worker scoped to /saturnus/");
  assert.equal(ownController(null, at), null);
  // With a query on the page, and at the old address, the rule is the same.
  assert.equal(ownController(own, new URL("https://ractive.ch/saturnus/app/?x=1")), own);
  assert.equal(ownController({ scriptURL: "https://ractive.ch/saturnus/sw.js" }, new URL("https://ractive.ch/saturnus/")).scriptURL, "https://ractive.ch/saturnus/sw.js");
});

/** A wake lock that records its requests and releases. */
function fakeWakeLock() {
  const log = [];
  return {
    log,
    request: async (type) => {
      log.push(`request ${type}`);
      const lock = new EventTarget();
      lock.release = async () => {
        log.push("release");
        lock.dispatchEvent(new Event("release"));
      };
      return lock;
    },
  };
}

test("the screen stays on only once a computation has run for the delay", async () => {
  const store = new Store();
  const doc = Object.assign(new EventTarget(), { hidden: false });
  const wakeLock = fakeWakeLock();
  const h = keepScreenOnWhileComputing(store, { wakeLock, doc, delay: 30 });
  // Waiting for a key: never.
  store.set({ loop: "sleep" });
  await sleep(50);
  assert.deepEqual(wakeLock.log, []);
  // A short computation: never.
  store.set({ loop: "frame" });
  await sleep(10);
  store.set({ loop: "sleep" });
  await sleep(50);
  assert.deepEqual(wakeLock.log, []);
  // A long one: after the delay, released when it ends.
  store.set({ loop: "frame" });
  await sleep(50);
  assert.equal(h.held(), true);
  store.set({ loop: "sleep" });
  await sleep(5);
  assert.equal(h.held(), false);
  assert.deepEqual(wakeLock.log, ["request screen", "release"]);
});

test("the screen lock goes with the page and comes back with it", async () => {
  const store = new Store();
  const doc = Object.assign(new EventTarget(), { hidden: false });
  const wakeLock = fakeWakeLock();
  const h = keepScreenOnWhileComputing(store, { wakeLock, doc, delay: 20 });
  store.set({ loop: "frame" });
  await sleep(40);
  assert.equal(h.held(), true);
  doc.hidden = true;
  doc.dispatchEvent(new Event("visibilitychange"));
  assert.equal(h.held(), false);
  doc.hidden = false;
  doc.dispatchEvent(new Event("visibilitychange"));
  await sleep(40);
  assert.equal(h.held(), true);
  assert.deepEqual(wakeLock.log, ["request screen", "release", "request screen"]);
});

test("no Wake Lock API: nothing to hold", () => {
  assert.equal(keepScreenOnWhileComputing(new Store(), { wakeLock: undefined, doc: new EventTarget() }), null);
});

/** A `navigator.storage` that records its `persist()` calls. */
function fakeStorage({ persisted = false, grants = true } = {}) {
  const log = [];
  return {
    log,
    persisted: async () => persisted,
    persist: () => {
      log.push("persist");
      return Promise.resolve(grants);
    },
  };
}

/** The page's preferences in a Map (app.js keeps them in localStorage). */
function fakePrefs() {
  const m = new Map();
  return { get: (k) => m.get(k) ?? null, set: (k, v) => m.set(k, v), remove: (k) => m.delete(k), map: m };
}

test("persistent storage is never asked for on load, only read", async () => {
  const store = new Store();
  const storage = fakeStorage();
  const choice = new StorageChoice({ romSource: "file" }, store, fakePrefs(), storage);
  await choice.read();
  store.set({ roms: { slots: [{ model: "48sx", fileName: "sxrom" }] } });
  await sleep(0);
  assert.deepEqual(storage.log, []);
  assert.equal(store.state.storage, "best-effort");
  assert.equal(store.state.storageOffer, null, "no notice without a ROM kept by the user");
});

test("after a ROM is kept the notice asks; an earlier grant is not asked about", async () => {
  const store = new Store();
  const storage = fakeStorage();
  await new StorageChoice({ romSource: "file" }, store, fakePrefs(), storage).offer();
  assert.equal(store.state.storageOffer, "ask");
  assert.deepEqual(storage.log, [], "asking is ours, not the browser's");

  const granted = new Store();
  await new StorageChoice({ romSource: "file" }, granted, fakePrefs(), fakeStorage({ persisted: true })).offer();
  assert.equal(granted.state.storageOffer, null);
  assert.equal(granted.state.storage, "persistent");
});

test("Keep it calls persist() at once, inside the click, and says what the browser answered", async () => {
  const store = new Store();
  const storage = fakeStorage();
  const choice = new StorageChoice({ romSource: "file" }, store, fakePrefs(), storage);
  await choice.offer();
  const answered = choice.keep();
  assert.deepEqual(storage.log, ["persist"], "called synchronously, while the gesture lasts");
  await answered;
  assert.equal(store.state.storage, "persistent");
  assert.equal(store.state.storageOffer, "kept");

  const refused = new Store();
  const no = new StorageChoice({ romSource: "file" }, refused, fakePrefs(), fakeStorage({ grants: false }));
  await no.offer();
  await no.keep();
  assert.equal(refused.state.storage, "best-effort");
  assert.equal(refused.state.storageOffer, "refused");
});

test("a refusal is remembered: the next ROM does not ask again, Keep permanently still does", async () => {
  const prefs = fakePrefs();
  const store = new Store();
  const storage = fakeStorage({ grants: false });
  const choice = new StorageChoice({ romSource: "file" }, store, prefs, storage);
  await choice.offer();
  await choice.keep();
  assert.equal(prefs.map.get("storageAsk"), "refused");
  choice.dismiss();
  await choice.offer();
  assert.equal(store.state.storageOffer, null, "Chrome would refuse again on every ROM");
  await choice.keep();
  assert.deepEqual(storage.log, ["persist", "persist"], "Keep permanently retries");
});

test("forgetting the ROMs takes the notice away", async () => {
  const store = new Store();
  const choice = new StorageChoice({ romSource: "file" }, store, fakePrefs(), fakeStorage());
  store.set({ roms: { slots: [{ model: "48sx", fileName: "sxrom" }] } });
  await choice.offer();
  assert.equal(store.state.storageOffer, "ask");
  store.set({ roms: { slots: [{ model: "48sx", fileName: "" }] } });
  assert.equal(store.state.storageOffer, null);
});

test("Not now is remembered and the notice is not offered again", async () => {
  const prefs = fakePrefs();
  const store = new Store();
  const storage = fakeStorage();
  const choice = new StorageChoice({ romSource: "file" }, store, prefs, storage);
  await choice.offer();
  choice.notNow();
  assert.equal(store.state.storageOffer, null);
  assert.equal(prefs.map.get("storageAsk"), "not-now");
  // The next ROM, or the next visit (the same preferences).
  await new StorageChoice({ romSource: "file" }, store, prefs, storage).offer();
  assert.equal(store.state.storageOffer, null);
  assert.equal(store.state.storage, "best-effort", "the panel still offers it");
  assert.deepEqual(storage.log, []);
});

test("the desktop app and browsers without persist(): nothing shown, nothing asked", async () => {
  for (const [backend, storage] of [[{ romSource: "dialog" }, fakeStorage()], [{ romSource: "file" }, { persisted: async () => false }], [{ romSource: "file" }, undefined]]) {
    const store = new Store();
    const choice = new StorageChoice(backend, store, fakePrefs(), storage);
    await choice.read();
    await choice.offer();
    await choice.keep();
    assert.equal(store.state.storage, null);
    assert.equal(store.state.storageOffer, null);
    assert.deepEqual(storage?.log ?? [], []);
  }
});


/** A service worker in `state`; `to(state)` moves it on, as the browser does. */
function fakeWorker(state) {
  const t = new EventTarget();
  t.state = state;
  t.to = (next) => {
    t.state = next;
    t.dispatchEvent(new Event("statechange"));
  };
  return t;
}

/** A registration; `found(worker)` starts an update, as the browser does. */
function fakeRegistration({ waiting = null, installing = null } = {}) {
  const r = new EventTarget();
  r.waiting = waiting;
  r.installing = installing;
  r.updates = 0;
  r.update = async () => { r.updates += 1; };
  r.found = (w) => {
    r.installing = w;
    r.dispatchEvent(new Event("updatefound"));
  };
  return r;
}

test("a new build is offered once installed: waiting, still installing at registration, or found later", () => {
  const waiting = fakeWorker("installed");
  const early = fakeWorker("installing");
  const reg = fakeRegistration({ waiting, installing: early });
  const offered = [];
  watchUpdates(reg, (w) => offered.push(w));
  assert.deepEqual(offered, [waiting]);
  // The race: its updatefound went by before the page listened.
  early.to("installed");
  assert.deepEqual(offered, [waiting, early]);
  // A late updatefound for the same worker does not watch it twice.
  const twice = fakeWorker("installing");
  const reg2 = fakeRegistration({ installing: twice });
  const offered2 = [];
  watchUpdates(reg2, (w) => offered2.push(w));
  reg2.found(twice);
  twice.to("installed");
  assert.deepEqual(offered2, [twice]);
  const later = fakeWorker("installing");
  reg.found(later);
  later.to("activating");
  assert.equal(offered.length, 2, "only an installed worker is offered");
  const next = fakeWorker("installing");
  reg.found(next);
  next.to("installed");
  assert.deepEqual(offered, [waiting, early, next]);
});

test("a new build is looked for on return (shown or focused), at most once per LOOK_EVERY_MS, never while hidden", async () => {
  const reg = fakeRegistration();
  const doc = new EventTarget();
  doc.hidden = false;
  const win = new EventTarget();
  let t = 0;
  lookOnReturn(reg, doc, win, () => t);
  const back = async (target, type) => {
    target.dispatchEvent(new Event(type));
    await sleep(0);
  };
  await back(win, "focus");
  assert.equal(reg.updates, 0, "the load itself just looked");
  t = LOOK_EVERY_MS;
  await back(win, "focus");
  assert.equal(reg.updates, 1);
  await back(doc, "visibilitychange");
  assert.equal(reg.updates, 1, "too soon after the last look");
  t = 2 * LOOK_EVERY_MS;
  doc.hidden = true;
  await back(doc, "visibilitychange");
  assert.equal(reg.updates, 1, "hidden");
  doc.hidden = false;
  await back(doc, "visibilitychange");
  assert.equal(reg.updates, 2);
  reg.update = async () => { throw new Error("offline"); };
  t = 3 * LOOK_EVERY_MS;
  await back(win, "focus");
});
