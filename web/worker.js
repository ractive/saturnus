// saturnus Web Worker: a thin driver of the protocol's state machine
// (saturnus_host::protocol, through the wasm bindings' `Host`), which owns
// the machine, paces it against the clock passed in and answers the page
// (web/protocol.md). This file feeds it the page's messages and one timer
// at the deadline it asks for, posts what it gives back, and keeps the ROM
// slots (romstore.js) and the auto-saved states (states.js) in IndexedDB,
// which need browser APIs.

import init, { Host, identify_rom, model_names, plan_roms, rom_download } from "./pkg/saturnus_web.js";
import { RomStore } from "./romstore.js";
import { ROM_HOLDING_STATES, autoKey, dbDelete, dbGet, dbPut } from "./states.js";

/** The commands of the ROM slots, served here over romstore.js. */
const ROM_COMMANDS = new Set(["romSlots", "bootModel", "chooseRom", "forgetRom", "romSettings"]);
/** A browser timer's longest delay. */
const MAX_DELAY_MS = 2 ** 31 - 1;

let host = null;
let timer = 0;
/** The page's reply ids by the tags the engine knows them by. */
const ids = new Map();
let nextTag = 1;

/** Post what the engine has (events, replies), then arm its timer. */
function deliver() {
  for (const m of host.drain()) {
    if (m.type === "autoSave") {
      keepState(m);
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

/** A *bytes* field as a Uint8Array. */
function bytesOf(v) {
  if (v === undefined || v === null) return undefined;
  return v instanceof Uint8Array ? v : new Uint8Array(v);
}

// ------------------------------------------------------------ auto-save
// The calculator keeps its state across reloads (iteration 27): the
// engine hands out the state once it changed and settled
// (saturnus_host::protocol's auto-save); it goes into the model's auto
// slot, never the user's own, and comes back when the model boots.

/** The writes and deletes of the auto slots, one after the other, in order. */
let keeping = Promise.resolve();
/** Models whose ROM was forgotten: a 49G state holds its ROM, so none is kept until it boots again. */
const forgotten = new Set();

function keepState({ model, romName, cycles, state }) {
  keeping = keeping
    .then(async () => {
      if (forgotten.has(model)) return;
      await dbPut(autoKey(model), { state, saved: Date.now(), cycles, romName });
      self.postMessage({ type: "autoSaved", model, cycles });
    })
    .catch((err) => console.warn(`saturnus: the ${model} state was not kept: ${err?.message ?? err}`));
}

/** The state kept for `model`, or null; a `fresh` start deletes it instead. */
async function keptState(model, fresh) {
  // A write still under way finishes first.
  await keeping;
  try {
    if (fresh) {
      await dbDelete(autoKey(model));
      return null;
    }
    const rec = await dbGet(autoKey(model));
    return rec?.state ? new Uint8Array(rec.state) : null;
  } catch (err) {
    console.warn(`saturnus: the kept ${model} state cannot be read: ${err?.message ?? err}`);
    return null;
  }
}

/** Boot `model`, as it was left unless `fresh`; a state that does not load boots cold and says why in the console. */
async function bootKept(model, rom, name, fresh) {
  const kept = await keptState(model, fresh);
  const booted = host.boot(model, rom, name, kept ?? undefined);
  forgotten.delete(booted.model);
  if (booted.restoreError) console.warn(`saturnus: the kept ${booted.model} state did not load (${booted.restoreError}); cold boot`);
  return booted;
}

/** After Forget ROMs: no auto-save of a ROM-holding model is written any more (the page deletes the slot). */
async function forgetKept(model) {
  for (const m of ROM_HOLDING_STATES) {
    if (model === null || model === m) forgotten.add(m);
  }
  await keeping;
}

// ------------------------------------------------------------ ROM slots
// The ROM of each model, kept in this browser (romstore.js): chosen once,
// booted by model afterwards (web/protocol.md, "ROM slots").

let romStore = null;
function roms() {
  romStore ??= new RomStore({
    identify: (bytes) => identify_rom(bytes),
    plan: (input) => plan_roms(JSON.stringify(input)),
    download: (model) => rom_download(model),
    boot: (model, rom, name, fresh) => bootKept(model, rom, name, fresh),
    models: model_names(),
  });
  return romStore;
}

function romCommand(m) {
  switch (m.cmd) {
    case "romSlots": return roms().slots();
    case "bootModel": return roms().bootModel(String(m.model), m.fresh === true);
    case "chooseRom": return roms().chooseRom(String(m.model), m.files, m.offer);
    case "forgetRom": return forgetKept(m.model ?? null).then(() => roms().forget(m.model ?? null));
    default: return roms().settings(m.bootLast);
  }
}

const ready = init().then(() => {
  host = new Host(() => performance.now());
});
/** Commands run one after the other, in the order they came (a ROM command waits for the store). */
let queue = ready.catch(() => {});

self.onmessage = (e) => {
  // A failure inside `handle` must not stop the commands after it.
  queue = queue.then(() => handle(e.data ?? {})).catch(() => {});
};

/** Answer a command the Worker serves itself (or that never reached the engine). */
function reply(m, ok, value) {
  if (m.id !== undefined) {
    self.postMessage(ok ? { type: "reply", id: m.id, ok, result: value ?? null } : { type: "reply", id: m.id, ok, error: value });
  } else if (!ok) {
    self.postMessage({ type: "error", message: value });
  }
}

async function handle(m) {
  try {
    await ready;
  } catch (err) {
    reply(m, false, `the emulator failed to start: ${err?.message ?? err}`);
    return;
  }
  if (!ROM_COMMANDS.has(m.cmd)) {
    const { rom, state, data, ...rest } = m;
    let tag;
    if (m.id !== undefined) {
      tag = nextTag++;
      ids.set(tag, m.id);
    }
    host.command(JSON.stringify(rest), bytesOf(rom ?? state ?? data), tag);
    deliver();
    return;
  }
  let ok = true;
  let value;
  try {
    host.check(JSON.stringify({ v: m.v, cmd: m.cmd }));
    value = await romCommand(m);
  } catch (err) {
    ok = false;
    value = String(err?.message ?? err);
  }
  // The events the command caused (a boot's) go out before its reply.
  deliver();
  reply(m, ok, value);
}
