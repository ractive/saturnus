// The transfer page's emulated calculator: saturnus's wasm core (the
// emulator's package, ../pkg) driven as web/worker.js drives it, in the
// protocol of web/protocol.md. The page boots it with a ROM's bytes, types
// SERVER (`run`) and talks over its serial port (`serialOpen`,
// `serialWrite`, the `serial` event).
//
// Two kinds (kb/docs/saturnus-tx.md, "Devices"):
// - `bootShared`: the calculator of a ROM the emulator keeps is the one
//   the emulator runs (iteration 41). Its lock first (asking the emulator
//   for it, web/shared/calc-lock.js), then booted from its auto-saved
//   state and saved back there as the emulator does, only while this
//   Worker holds it. Asked for by another page, the Worker tells the page
//   (`wanted`), which leaves SERVER and sends `handOver`: the server
//   ended if it still runs (whatever way the page let go), saved,
//   stopped, let go. Taken without asking: `taken`.
// - `boot`: a ROM file chosen on the page boots cold and keeps nothing
//   (the engine's auto-save stays off).

import init, { Host, state_format } from "../pkg/saturnus_web.js";
import { CalcLock, IN_USE } from "../shared/calc-lock.js";
import { TOO_NEW, autoKey, claim, dbGet, holderOf, putIfHolder, tooNew } from "../shared/calc-states.js";

/** A browser timer's longest delay. */
const MAX_DELAY_MS = 2 ** 31 - 1;

let host = null;
let timer = 0;
/** The page's reply ids by the tags the engine knows them by. */
const ids = new Map();
let nextTag = 1;
/** The Worker's own commands to the engine: their tags, and what waits for the reply. */
const own = new Map();

/** The shared calculator this Worker runs: `{model, token}`, or null (none, or a ROM file's). */
let shared = null;
/** The writes of its state, one after the other. */
let keeping = Promise.resolve();
/** The page's answer to `wanted`: resolves the handover. */
let handedOver = null;

function deliver() {
  for (const m of host.drain()) {
    if (m.type === "autoSave") {
      keepState(m);
    } else if (m.type === "reply" && own.has(m.tag)) {
      own.get(m.tag)(m);
      own.delete(m.tag);
    } else if (m.type === "reply") {
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

/** Keep the shared calculator's state, while this Worker still holds it. */
function keepState({ model, romName, cycles, state }) {
  const mine = shared;
  keeping = keeping
    .then(async () => {
      if (!mine?.token || mine.model !== model || !lock.holds(model)) return;
      if (!(await putIfHolder(model, { state, saved: Date.now(), cycles, romName }, mine.token))) {
        console.warn(`saturnus tx: the ${model} state was not kept: another page runs that calculator now`);
      }
    })
    .catch((err) => console.warn(`saturnus tx: the ${model} state was not kept: ${err?.message ?? err}`));
}

const lock = new CalcLock({
  who: "tx",
  // The page leaves SERVER first, then sends `handOver`.
  onWanted: (model, by) => {
    if (shared?.model !== model) return undefined;
    self.postMessage({ type: "wanted", model, by });
    return new Promise((resolve) => {
      handedOver = resolve;
    });
  },
  onLost: (model) => {
    if (shared?.model !== model) return;
    shared = null;
    host.setAutoSave(false);
    // The page that took it writes its record right after: read it then.
    setTimeout(() => holderOf(model).then((by) => self.postMessage({ type: "taken", model, by })), 500);
  },
});

/** Send the engine a command of the Worker's own; resolves to its reply. */
function engine(cmd, args = {}) {
  const tag = nextTag++;
  const p = new Promise((resolve) => own.set(tag, resolve));
  host.command(JSON.stringify({ v: 1, cmd, ...args }), undefined, tag);
  deliver();
  return p;
}

/** Boot the shared calculator of `model` from `rom`: `{model, romName, restored?}`. */
async function bootShared(model, rom, romName, ask) {
  if (!(await lock.acquire(model, { ask }))) throw new Error(IN_USE);
  // Held from here: a `want` that comes while it boots is the page's to answer.
  shared = { model, token: null };
  try {
    let token = null;
    try {
      token = await claim(model, "tx");
    } catch (err) {
      console.warn(`saturnus tx: the ${model} state cannot be kept here: ${err?.message ?? err}`);
    }
    const rec = await dbGet(autoKey(model)).catch(() => null);
    const kept = rec?.state ? new Uint8Array(rec.state) : null;
    if (kept && tooNew(kept, state_format())) throw new Error(TOO_NEW);
    const booted = host.boot(model, rom, romName, kept ?? undefined);
    if (booted.restoreError) console.warn(`saturnus tx: the kept ${booted.model} state did not load (${booted.restoreError}); cold boot`);
    shared = { model: booted.model, token };
    host.setAutoSave(true);
    deliver();
    return booted;
  } catch (err) {
    shared = null;
    lock.release(model);
    throw err;
  }
}

/**
 * End the server if it still runs (the page sent FINISH when it could; a
 * link lost or a failed start did not), then save the shared calculator,
 * stop it and let go of it: the emulator never gets it in SERVER.
 */
async function handOver() {
  const model = shared?.model;
  if (model) {
    await engine("serialClose");
    const ended = await engine("endServer");
    if (!ended.ok) console.warn(`saturnus tx: the ${model}'s server was not ended: ${ended.error}`);
    for (let i = 0; i < 80; i++) {
      const r = await engine("unload");
      if (r.ok) break;
      await new Promise((res) => setTimeout(res, 250));
    }
    await keeping;
    shared = null;
    host.setAutoSave(false);
  }
  if (handedOver) {
    handedOver();
    handedOver = null;
  } else if (model) {
    lock.release(model);
  }
  return null;
}

const ready = init().then(() => {
  host = new Host(() => performance.now());
  host.setAutoSave(false);
});
let queue = ready.catch(() => {});

self.onmessage = (e) => {
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
    answer(m, false, `the emulator failed to start: ${err?.message ?? err}`);
    return;
  }
  if (m.cmd === "bootShared" || m.cmd === "handOver") {
    try {
      const r = m.cmd === "bootShared" ? await bootShared(String(m.model), new Uint8Array(m.rom), String(m.romName ?? ""), m.ask !== false) : await handOver();
      answer(m, true, r);
    } catch (err) {
      answer(m, false, String(err?.message ?? err));
    }
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
