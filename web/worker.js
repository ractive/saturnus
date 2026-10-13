// saturnus Web Worker: a thin driver of the protocol's state machine
// (saturnus_host::protocol, through the wasm bindings' `Host`), which owns
// the machine, paces it against the clock passed in and answers the page
// (web/protocol.md). This file feeds it the page's messages and one timer
// at the deadline it asks for, posts what it gives back, and keeps the ROM
// slots (romstore.js) and the auto-saved states (states.js) in IndexedDB,
// which need browser APIs.

import init, { Host, identify_rom, model_names, plan_roms, rom_download, state_format } from "./pkg/saturnus_web.js";
import { RomStore } from "./romstore.js";
import { CalcLock, IN_USE } from "./shared/calc-lock.js";
import { TOO_NEW, autoKey, claim, dbDelete, dbGet, holderOf, putIfHolder, tooNew } from "./shared/calc-states.js";
import { ROM_HOLDING_STATES } from "./states.js";

/** The commands of the ROM slots, served here over romstore.js. */
const ROM_COMMANDS = new Set(["romSlots", "bootModel", "chooseRom", "forgetRom", "romSettings"]);
/** The models with a Kermit server (`endServer`). */
const SERVER_MODELS = new Set(["48sx", "48gx", "49g"]);
/** A browser timer's longest delay. */
const MAX_DELAY_MS = 2 ** 31 - 1;

let host = null;
let timer = 0;
/** The page's reply ids by the tags the engine knows them by. */
const ids = new Map();
let nextTag = 1;

/** The model the engine runs, from its `status` events (null: none). */
let running = null;
/** The Worker's own commands to the engine: their tags, and what waits for the reply. */
const own = new Map();

/** Post what the engine has (events, replies), then arm its timer. */
function deliver() {
  for (const m of host.drain()) {
    if (m.type === "status") running = m.model || null;
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
      // Only while this Worker runs the model's calculator (iteration 41):
      // a lock taken from it, or let go, ends its writes.
      const token = tokens.get(model);
      if (!token || !share.holds(model)) return;
      if (!(await putIfHolder(model, { state, saved: Date.now(), cycles, romName }, token))) {
        console.warn(`saturnus: the ${model} state was not kept: another page runs that calculator now`);
        return;
      }
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

// ------------------------------------------------------------ one page at a time
// The auto-saved calculator is the one saturnus tx runs too (iteration 41,
// web/shared/calc-lock.js): this Worker holds a model's lock while it runs
// that model, hands it over when another page asks (saved, then stopped:
// the page shows "In use by saturnus tx"), and writes its state only while
// its holder record (`tokens`) is the one in the database.

/** model -> this Worker's holder token (web/shared/calc-states.js `claim`). */
const tokens = new Map();
/** Whether the next `bootModel` asks a page that holds the model for it (false: the page's boot at start). */
let bootAsk = true;

const share = new CalcLock({
  who: "emulator",
  onWanted: (model, by) => serial(() => handOver(model, by)),
  onLost: (model) => serial(() => lostIt(model)),
});

/** Run `fn` between the page's commands, never inside one. */
function serial(fn) {
  const p = queue.then(fn);
  queue = p.catch(() => {});
  return p;
}

/** Send the engine a command of the Worker's own; resolves to its reply. */
function engine(cmd, args = {}) {
  const tag = nextTag++;
  const p = new Promise((resolve) => own.set(tag, resolve));
  host.command(JSON.stringify({ v: 1, cmd, ...args }), undefined, tag);
  deliver();
  return p;
}

/** Stop the calculator (saved first, if this Worker may still write); retried while a send types. */
async function stopRunning() {
  for (let i = 0; running && i < 80; i++) {
    const r = await engine("unload");
    if (r.ok) break;
    await new Promise((res) => setTimeout(res, 250));
  }
  await keeping;
}

/** Another page asked for `model`: save it, stop it and let go (the lock goes when this resolves). */
async function handOver(model, by) {
  if (running !== model) {
    tokens.delete(model);
    return;
  }
  await stopRunning();
  tokens.delete(model);
  self.postMessage({ type: "away", model, by });
}

/** `model`'s lock was taken without asking (this tab was frozen): stop without writing. */
async function lostIt(model) {
  tokens.delete(model);
  if (running === model) await stopRunning();
  // The page that took it writes its record right after: read it then.
  await new Promise((r) => setTimeout(r, 500));
  self.postMessage({ type: "away", model, by: await holderOf(model) });
}

/** Record this Worker as `model`'s holder; without a writable database it runs unsaved (no token), as before. */
async function claimFor(model) {
  try {
    tokens.set(model, await claim(model, "emulator"));
  } catch (err) {
    console.warn(`saturnus: the ${model} state cannot be kept here: ${err?.message ?? err}`);
  }
}

/** Let go of every lock but `keep`'s, once the writes before are done. */
async function releaseOthers(keep = null) {
  await keeping;
  for (const m of share.models()) {
    if (m === keep || m === running) continue;
    tokens.delete(m);
    share.release(m);
  }
}

/**
 * Boot `model`, as it was left unless `fresh`; a state that does not load
 * boots cold and says why in the console. Its calculator's lock first:
 * one another page holds is asked for (`bootAsk`), or the boot fails with
 * `IN_USE` and the page hears `away`.
 */
async function bootKept(model, rom, name, fresh) {
  const ask = bootAsk;
  bootAsk = true;
  if (!(await share.acquire(model, { ask }))) {
    self.postMessage({ type: "away", model, by: await holderOf(model) });
    throw new Error(IN_USE);
  }
  try {
    if (!tokens.has(model)) await claimFor(model);
    // The machine it replaces goes into its slot first: a reboot of the same
    // model then reads the newest state, a fresh start deletes after it.
    host.saveNow();
    deliver();
    const kept = await keptState(model, fresh);
    if (kept && tooNew(kept, state_format())) throw new Error(TOO_NEW);
    const booted = host.boot(model, rom, name, kept ?? undefined);
    forgotten.delete(booted.model);
    if (booted.restoreError) console.warn(`saturnus: the kept ${booted.model} state did not load (${booted.restoreError}); cold boot`);
    running = booted.model;
    if (booted.restored) await endServer(booted.model);
    return booted;
  } finally {
    // The model left behind (or this one, when it did not boot) is let go.
    await releaseOthers();
  }
}

/**
 * A calculator saved in SERVER (saturnus tx's tab closed, or its link
 * lost, while it served) is ended as CANCEL would: one I packet asks
 * whether the server runs, a `G F` ends it. Only where it has a server
 * and its port is on (web/protocol.md, `endServer`).
 */
async function endServer(model) {
  if (!SERVER_MODELS.has(model)) return;
  try {
    const r = await engine("endServer");
    if (!r.ok) console.warn(`saturnus: the ${model}'s server was not ended: ${r.error}`);
  } catch (err) {
    console.warn(`saturnus: the ${model}'s server was not ended: ${err?.message ?? err}`);
  }
}

/** After Remove ROMs: no auto-save of a ROM-holding model is written any more (the page deletes the slot). */
async function forgetKept(model) {
  for (const m of ROM_HOLDING_STATES) {
    if (model !== null && model !== m) continue;
    forgotten.add(m);
    // saturnus tx may run it: taken from it (saved there first) and its
    // holder record replaced, so it writes the ROM-holding state no more.
    if (await share.acquire(m, { ask: true })) await claimFor(m);
  }
  await keeping;
  await releaseOthers();
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
    case "bootModel":
      bootAsk = m.ask !== false;
      // Reset however the command ends (a missing ROM never reaches the boot).
      return roms().bootModel(String(m.model), m.fresh === true).finally(() => {
        bootAsk = true;
      });
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
    const { rom, state, data, bytes, ...rest } = m;
    let tag;
    if (m.id !== undefined) {
      tag = nextTag++;
      ids.set(tag, m.id);
    }
    host.command(JSON.stringify(rest), bytesOf(rom ?? state ?? data ?? bytes), tag);
    deliver();
    // Unloaded (Remove ROMs): its calculator is free for the other page.
    if (m.cmd === "unload" && !running) await releaseOthers();
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
