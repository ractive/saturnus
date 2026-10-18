// The transfer page's private emulated calculator: saturnus's wasm core
// (the wasm package, ../pkg) driven as web/hub.js drives it, in the
// protocol of web/protocol.md, for a ROM file chosen on the page (and in
// the desktop app). It boots cold, keeps nothing (the engine's auto-save
// stays off) and is nobody else's: the calculator of a ROM the emulator
// keeps is the emulator's own, shared through its SharedWorker
// (web/shared/calc-hub.js, iteration 46). The page types SERVER (`run`)
// and talks over its serial port (`serialOpen`, `serialWrite`, the
// `serial` event).

import init, { Host } from "../pkg/saturnus_web.js";

/** A browser timer's longest delay. */
const MAX_DELAY_MS = 2 ** 31 - 1;

let host = null;
let timer = 0;
/** The page's reply ids by the tags the engine knows them by. */
const ids = new Map();
let nextTag = 1;

function deliver() {
  for (const m of host.drain()) {
    if (m.type === "reply") {
      const id = ids.get(m.tag);
      ids.delete(m.tag);
      self.postMessage(m.ok ? { type: "reply", id, ok: true, result: m.result } : { type: "reply", id, ok: false, error: m.error });
    } else {
      self.postMessage(m);
    }
  }
  clearTimeout(timer);
  const at = host.deadline();
  timer = at === undefined ? 0 : setTimeout(fire, Math.min(Math.max(at - performance.now(), 0), MAX_DELAY_MS));
}

function fire() {
  timer = 0;
  host.timer();
  deliver();
}

const ready = init().then(() => {
  host = new Host(() => performance.now());
  host.setAutoSave(false);
});
let queue = ready.catch(() => {});

self.onmessage = (e) => {
  // The page's heartbeat (web/shared/calc-hub.js): answered at once, outside the queue.
  if (e.data?.cmd === "ping") {
    answer(e.data, true, null);
    return;
  }
  queue = queue.then(() => handle(e.data ?? {})).catch(() => {});
};

function answer(m, ok, value) {
  if (m.id === undefined) return;
  self.postMessage(ok ? { type: "reply", id: m.id, ok: true, result: value ?? null } : { type: "reply", id: m.id, ok: false, error: value });
}

async function handle(m) {
  try {
    await ready;
  } catch (err) {
    answer(m, false, `the emulated calculator failed to start: ${err?.message ?? err}`);
    return;
  }
  const { rom, bytes, ...rest } = m;
  let tag;
  if (m.id !== undefined) {
    tag = nextTag++;
    ids.set(tag, m.id);
  }
  try {
    const data = rom ?? bytes;
    host.command(JSON.stringify(rest), data === undefined ? undefined : new Uint8Array(data), tag);
    deliver();
  } catch (err) {
    // The core failed (a wasm panic leaves it unusable): this command's
    // answer, then `fatal`, which the page takes as the calculator gone.
    const message = `the emulated calculator failed: ${err?.message ?? err}`;
    if (tag !== undefined && ids.has(tag)) {
      ids.delete(tag);
      self.postMessage({ type: "reply", id: m.id, ok: false, error: message });
    }
    self.postMessage({ type: "fatal", message });
  }
}
