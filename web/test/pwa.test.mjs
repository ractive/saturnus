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
import { StorageChoice, keepScreenOnWhileComputing, serviceWorkerAllowed } from "../pwa.js";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const nav = { serviceWorker: {} };

test("the service worker only over HTTPS or on this computer, never in the desktop app", () => {
  const at = (href) => new URL(href);
  assert.equal(serviceWorkerAllowed("worker", at("https://ractive.ch/saturnus/"), nav), true);
  assert.equal(serviceWorkerAllowed("worker", at("http://localhost:8000/"), nav), true);
  assert.equal(serviceWorkerAllowed("worker", at("http://127.0.0.1:8000/"), nav), true);
  assert.equal(serviceWorkerAllowed("worker", at("http://example.org/"), nav), false);
  assert.equal(serviceWorkerAllowed("worker", at("https://ractive.ch/"), {}), false, "no service workers in this browser");
  // The app's pages: tauri://localhost (macOS, Linux), http://tauri.localhost (Windows).
  assert.equal(serviceWorkerAllowed("tauri", at("tauri://localhost/"), nav), false);
  assert.equal(serviceWorkerAllowed("tauri", at("http://tauri.localhost/"), nav), false);
  assert.equal(serviceWorkerAllowed("tauri", at("https://localhost/"), nav), false);
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
