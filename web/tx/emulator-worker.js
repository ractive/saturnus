// The transfer page's emulated calculator: saturnus's wasm core (the
// emulator's package, ../pkg) driven as web/worker.js drives it, in the
// protocol of web/protocol.md, but keeping nothing: no ROM slots, no
// auto-saved states (the engine's auto-save is off), so the transfer page
// never writes the emulator's storage (kb/docs/saturnus-tx.md, "URL and
// deploy"). The page boots it with a ROM's bytes (`boot`), types SERVER
// (`run`) and talks over its serial port (`serialOpen`, `serialWrite`,
// the `serial` event).

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
    if (m.type === "autoSave") continue;
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
  queue = queue.then(() => handle(e.data ?? {})).catch(() => {});
};

async function handle(m) {
  try {
    await ready;
  } catch (err) {
    if (m.id !== undefined) self.postMessage({ type: "reply", id: m.id, ok: false, error: `the emulator failed to start: ${err?.message ?? err}` });
    return;
  }
  const { rom, bytes, ...rest } = m;
  let tag;
  if (m.id !== undefined) {
    tag = nextTag++;
    ids.set(tag, m.id);
  }
  const data = rom ?? bytes;
  host.command(JSON.stringify(rest), data === undefined ? undefined : new Uint8Array(data), tag);
  deliver();
}
