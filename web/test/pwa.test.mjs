// The installed page's parts in the page (web/pwa.js): where the service
// worker may be registered, the screen kept on through a long computation
// only, and persistent storage asked for once a ROM is kept. The worker
// itself and its cache are checked against a built site in
// site.test.mjs, and in a browser by hand (kb iteration 22).
import { test } from "node:test";
import assert from "node:assert/strict";
import { Store } from "../store.js";
import { keepScreenOnWhileComputing, persistWhenKept, serviceWorkerAllowed } from "../pwa.js";

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

const slots = (...names) => ({ slots: [{ model: "48sx", fileName: names[0] ?? "" }, { model: "49g", fileName: names[1] ?? "" }] });

test("persistent storage is asked for once, when the first ROM is kept in the browser", async () => {
  const store = new Store();
  let asks = 0;
  const storage = { persisted: async () => false, persist: async () => { asks++; return true; } };
  persistWhenKept({ romSource: "file" }, store, storage);
  store.set({ roms: slots() });
  await sleep(0);
  assert.equal(asks, 0, "no ROM kept yet");
  assert.equal(store.state.storage, null);
  store.set({ roms: slots("sxrom") });
  await sleep(0);
  store.set({ roms: slots("sxrom", "rom.49g") });
  await sleep(0);
  assert.equal(asks, 1);
  assert.equal(store.state.storage, "persistent");
});

test("a refusal is shown as best effort; an earlier grant is not asked again", async () => {
  const refused = new Store();
  persistWhenKept({ romSource: "file" }, refused, { persisted: async () => false, persist: async () => false });
  refused.set({ roms: slots("sxrom") });
  await sleep(0);
  assert.equal(refused.state.storage, "best-effort");

  const granted = new Store();
  let asks = 0;
  persistWhenKept({ romSource: "file" }, granted, { persisted: async () => true, persist: async () => { asks++; return true; } });
  granted.set({ roms: slots("sxrom") });
  await sleep(0);
  assert.equal(granted.state.storage, "persistent");
  assert.equal(asks, 0);
});

test("the desktop app keeps no ROMs in the browser: nothing asked", async () => {
  const store = new Store();
  let asks = 0;
  persistWhenKept({ romSource: "dialog" }, store, { persisted: async () => false, persist: async () => { asks++; return true; } });
  store.set({ roms: slots("sxrom") });
  await sleep(0);
  assert.equal(asks, 0);
  assert.equal(store.state.storage, null);
});
