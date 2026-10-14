// The calculators of this site, one live machine per model, shared by
// every page that shows one (iteration 46): the emulator page and saturnus
// tx talk to the same running calculator at once. Run by a SharedWorker
// (shared-worker.js), one connection per page, and in a plain Worker
// (worker.js) where there is no SharedWorker, with the page as its only
// connection.
//
// Each engine is the protocol's state machine (saturnus_host::protocol,
// through the wasm bindings' `Host`), driven as before: commands in, one
// timer at the deadline it asks for, its output routed. Every page has an
// engine of its own (its "home": no ROM, or a ROM given once with `boot`,
// kept nowhere); the calculator of a kept ROM is one engine per model,
// which every page that boots or attaches that model shows. The hub keeps
// what is per page (web/protocol.md, "Shared calculators"): the engine it
// shows, its speed choice, whether it is hidden, whether it watches the
// memory, whether it holds the serial line. And per engine: its pages,
// the holder of its serial line, the last status, keys and frame (sent to
// a page that comes later), and the auto-saved state of a kept one, which
// the hub alone writes (IndexedDB, `auto:<model>`), as the Worker did.
// With one page the hub passes the page's commands through unchanged, so
// a plain Worker answers as before (web/test/protocol-script.test.mjs).

import init, { Host, identify_rom, model_names, plan_roms, rom_download, state_format } from "./pkg/saturnus_web.js";
import { RomStore } from "./romstore.js";
import { BUILD } from "./shared/build.js";
import { CALC_STOPPED, MIXED_BUILDS } from "./shared/calc-hub.js";
import "./shared/models.js";
import { TOO_NEW, autoKey, dbDelete, dbGet, dbPut, tooNew } from "./shared/calc-states.js";
import { ROM_HOLDING_STATES } from "./states.js";

/** The commands of the ROM slots, served over romstore.js. */
const ROM_COMMANDS = new Set(["romSlots", "bootModel", "chooseRom", "forgetRom", "romSettings"]);
/** The models with a Kermit server (`endServer`; web/shared/models.js). */
const SERVER_MODELS = new Set(globalThis.SATURNUS_SERIAL);
/** How many times a stop asks the engine to unload (250 ms apart) while a send types. */
const STOP_TRIES = 80;
/** A browser timer's longest delay. */
const MAX_DELAY_MS = 2 ** 31 - 1;
/** How long after the last byte on the serial line saturnus tx's speed still applies. */
export const SERIAL_IDLE_MS = 3000;
/** The longest wait for a reply to the hub's own command. */
const OWN_REPLY_MS = 30_000;
/** The engine's refusal of a write while the serial port is held. */
const PORT_HELD = "the serial port is in use";
/** What a page hears when saturnus tx holds the serial line. */
export const LINE_TAKEN = "saturnus tx is connected to this calculator. Disconnect it there first.";
/** What a page hears when another emulator page holds it. */
export const LINE_ELSEWHERE = "Another page is using this calculator's serial port.";
/** What a page hears when another tab keeps `model`'s state (its auto-save lock is taken). */
export const autoSaveOff = (model) => `The ${globalThis.SATURNUS_MODEL_TITLES[model] ?? model} also runs in another saturnus tab, which keeps its state: this tab does not save it.`;
/** Commands of another page that end or change what tx's Kermit server sees: refused while tx holds the line. */
const REFUSED_WHILE_TX = new Set(["reset", "loadState"]);
/** Commands of another page that type on the calculator: tx hears `keysElsewhere`, its server may have ended. */
const TYPING = new Set(["typeKeys", "typeLetter", "insert", "run", "replace", "typeText", "setClock"]);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** A *bytes* field as a Uint8Array. */
function bytesOf(v) {
  if (v === undefined || v === null) return undefined;
  return v instanceof Uint8Array ? v : new Uint8Array(v);
}

/** Whether two ROMs are the same bytes. */
function sameBytes(a, b) {
  if (a === b) return true;
  if (!a || !b || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

/** The auto-saved states in IndexedDB (web/shared/calc-states.js). */
const idbStates = {
  get: async (model) => {
    const rec = await dbGet(autoKey(model));
    return rec?.state ? new Uint8Array(rec.state) : null;
  },
  put: (model, rec) => dbPut(autoKey(model), rec),
  delete: (model) => dbDelete(autoKey(model)),
};

/** One page's connection: a MessagePort, or the Worker's own scope. */
class Conn {
  constructor(port, id) {
    this.port = port;
    this.id = id;
    /** "emulator" (the default, also a plain Worker's page) or "tx". */
    this.who = "emulator";
    /** Its own engine (made once the wasm is ready), and the engine it shows (its home, or a kept model's). */
    this.home = null;
    this.machine = null;
    /** Its speed choice (`setSpeed`), or null. */
    this.speed = null;
    this.hidden = false;
    /** It asked for `memoryChanged`. */
    this.watch = false;
    /** Gone: left, or its page closed; refused: another build. */
    this.gone = false;
    this.refused = null;
  }

  post(m, { always = false } = {}) {
    if (this.gone && !always) return;
    try {
      this.port.postMessage(m);
    } catch {
      // A port whose page is gone.
    }
  }
}

/** One engine: a kept model's machine, or a page's home. */
class Engine {
  constructor(key, { kept = false } = {}) {
    this.key = key;
    this.model = null;
    this.romName = "";
    /** The ROM it runs (a running machine is shown only to a page with the same ROM). */
    this.rom = null;
    this.kept = kept;
    this.host = new Host(() => performance.now());
    this.host.setAutoSave(kept);
    this.timer = 0;
    this.conns = new Set();
    /** The page whose command runs now (an `error` event is its). */
    this.current = null;
    /** The page holding the serial line. */
    this.holder = null;
    /** Reply ids by the engine's tags: `{conn, id}`. */
    this.tags = new Map();
    /** The hub's own commands, by tag. */
    this.own = new Map();
    this.last = { status: null, keys: null, frame: null };
    /** The emulator page's speed; the speed, visibility and memory watch the engine has (its defaults first). */
    this.emuSpeed = null;
    this.sentSpeed = "1";
    this.sentHidden = false;
    this.watching = false;
    /** saturnus tx moved bytes on the serial line lately (`SERIAL_IDLE_MS`). */
    this.lineBusy = false;
    this.lineTimer = 0;
    this.stopped = false;
    /** A stop waits for a send to end (`retryStop`), outside the hub's queue. */
    this.stopping = false;
    /** Lets go of the model's auto-save lock (`saturnus-auto:<model>`); `autoSaveOff` the message when another tab holds it. */
    this.releaseAuto = null;
    this.autoSaveOff = null;
  }
}

export class Hub {
  /**
   * `locks`: the Web Locks of the scope (a page's lifetime lock tells
   * that it closed), or null; `states`: where the auto-saved states go;
   * `build`: this build's id (web/shared/build.js).
   */
  constructor({ locks = null, states = idbStates, build = BUILD, lineIdleMs = SERIAL_IDLE_MS, stopTries = STOP_TRIES } = {}) {
    this.stopTries = stopTries;
    this.locks = locks;
    this.states = states;
    this.build = build;
    this.lineIdleMs = lineIdleMs;
    /** The kept models' machines, by model. */
    this.machines = new Map();
    this.conns = new Set();
    this.nextConn = 1;
    this.nextTag = 1;
    /** The auto-saved states' writes and deletes, one after the other. */
    this.keeping = Promise.resolve();
    /** Models whose ROM was removed: a 49G state holds its ROM, so none is written until it boots again. */
    this.forgotten = new Set();
    this.romStore = null;
    /** The page whose ROM command runs (the boot it leads to is its). */
    this.romConn = null;
    this.ready = init();
    /** Commands run one after the other, in the order they came, whichever page sent them. */
    this.queue = this.ready.catch(() => {});
  }

  // ---------------------------------------------------------- connections

  /** A new page on `port`. */
  connect(port) {
    const conn = new Conn(port, this.nextConn++);
    this.conns.add(conn);
    return conn;
  }

  /**
   * A message from `conn`'s page: queued after every command before it.
   * A `ping` (the page's heartbeat, web/shared/calc-hub.js) is answered at
   * once, outside the queue: a long command does not delay it.
   */
  receive(conn, m) {
    if (m?.cmd === "ping") {
      this.reply(conn, m, !conn.gone && !conn.refused, conn.gone ? CALC_STOPPED : conn.refused ?? null);
      return Promise.resolve();
    }
    return this.serial(() => this.handle(conn, m ?? {}));
  }

  /** Run `fn` between the pages' commands, never inside one. */
  serial(fn) {
    const p = this.queue.then(fn);
    this.queue = p.catch(() => {});
    return p;
  }

  /** `join`: who the page is, its lifetime lock, its build. */
  join(conn, m) {
    if (m.build !== undefined && m.build !== this.build) {
      conn.refused = MIXED_BUILDS;
      throw new Error(MIXED_BUILDS);
    }
    conn.who = m.who === "tx" ? "tx" : "emulator";
    if (typeof m.lock === "string" && this.locks) {
      // Granted when the page is gone (it holds the lock for its life).
      this.locks.request(m.lock, () => this.serial(() => this.leave(conn))).catch(() => {});
    }
    return { build: this.build };
  }

  /**
   * The page left (or closed): off its machine, which stops when no page
   * is left; its home goes. `done` is told before the page is let go (the
   * reply to its `leave`).
   */
  async leave(conn, done = () => {}) {
    if (conn.gone) return;
    await this.detach(conn);
    done();
    conn.gone = true;
    this.conns.delete(conn);
    if (conn.home) await this.stop(conn.home);
  }

  /**
   * `conn` stops showing a kept machine and is back on its home: its
   * serial line let go (a server it started ended), the machine stopped
   * when it was the last page.
   */
  async detach(conn) {
    const e = conn.machine;
    if (!e || e === conn.home) return;
    this.toHome(conn);
    e.conns.delete(conn);
    if (e.holder === conn) {
      e.holder = null;
      await this.engine(e, "serialClose");
      if (SERVER_MODELS.has(e.model)) {
        const r = await this.engine(e, "endServer");
        if (!r.ok) console.warn(`saturnus: the ${e.model}'s server was not ended: ${r.error}`);
      }
    }
    if (!e.conns.size) {
      await this.stop(e);
      return;
    }
    this.settle(e);
  }

  /** `conn` shows its home again. */
  toHome(conn) {
    conn.machine = conn.home;
    conn.home?.conns.add(conn);
  }

  /** `conn` shows `e` from now on (off its home): the status, keys and frame it missed first. */
  attach(conn, e) {
    // A stop that waits for a send is called off: the page shows it again, and it keeps its state again.
    if (e.stopping) {
      e.stopping = false;
      if (e.model) this.forgotten.delete(e.model);
    }
    if (conn.machine === e) return;
    if (e.autoSaveOff) conn.post({ type: "autoSaveOff", model: e.model, message: e.autoSaveOff });
    if (conn.home && conn.machine === conn.home) {
      conn.home.conns.delete(conn);
      // Its own ROM, if any, stops: nobody shows it now.
      this.command(conn.home, null, { cmd: "unload" });
      conn.home.last.status = null;
    }
    conn.machine = e;
    e.conns.add(conn);
    if (conn.who === "emulator" && conn.speed) e.emuSpeed = conn.speed;
    for (const k of ["status", "keys", "frame"]) {
      if (e.last[k]) conn.post(e.last[k]);
    }
    this.settle(e);
  }

  /** The machine's speed, visibility and memory watch, from its pages. */
  settle(e) {
    if (e.stopped) return;
    this.applySpeed(e);
    const hidden = this.hiddenOf(e);
    if (hidden !== e.sentHidden) {
      e.sentHidden = hidden;
      this.command(e, null, { cmd: "visibility", hidden });
    }
    const watch = [...e.conns].some((c) => c.watch);
    if (watch !== e.watching) {
      e.watching = watch;
      this.command(e, null, { cmd: "watchMemory", on: watch });
    }
  }

  /** Hidden only when every page that shows it is (saturnus tx never says so). */
  hiddenOf(e) {
    return e.conns.size > 0 && [...e.conns].every((c) => c.hidden);
  }

  /**
   * The speed: the emulator page's; saturnus tx's (Max, 1x for a fast
   * transfer) while it holds the serial line and moves bytes or works
   * the calculator, or when no emulator page shows it.
   */
  speedOf(e) {
    const conns = [...e.conns];
    const emulator = conns.some((c) => c.who === "emulator");
    const base = emulator ? (e.emuSpeed ?? "1") : (conns.map((c) => c.speed).filter(Boolean).at(-1) ?? "1");
    const h = e.holder;
    // A slower speed than Max (1x for the fast transfer) holds while tx has the line.
    if (h && h.who === "tx" && h.speed && (e.lineBusy || !emulator || h.speed !== "max")) return h.speed;
    return base;
  }

  applySpeed(e) {
    const speed = this.speedOf(e);
    if (speed === e.sentSpeed) return;
    e.sentSpeed = speed;
    this.command(e, null, { cmd: "setSpeed", speed });
  }

  /** saturnus tx moved bytes on its line, or worked the calculator: its speed for a while. */
  lineMoved(e) {
    if (e.holder?.who !== "tx") return;
    clearTimeout(e.lineTimer);
    e.lineTimer = setTimeout(() => {
      e.lineBusy = false;
      this.applySpeed(e);
    }, this.lineIdleMs);
    e.lineBusy = true;
    // Also when it was busy already: the holder may be new.
    this.applySpeed(e);
  }

  // ---------------------------------------------------------- engines

  /** Post what `e`'s engine has (events, replies) to its pages, then arm its timer. */
  deliver(e) {
    if (e.stopped) return;
    for (const m of e.host.drain()) this.route(e, m);
    clearTimeout(e.timer);
    const at = e.host.deadline();
    e.timer = at === undefined ? 0 : setTimeout(() => this.fire(e), Math.min(Math.max(at - performance.now(), 0), MAX_DELAY_MS));
  }

  fire(e) {
    e.timer = 0;
    if (e.stopped) return;
    try {
      e.host.timer();
      this.deliver(e);
    } catch (err) {
      this.broken(e, err);
    }
  }

  route(e, m) {
    if (m.type === "reply") {
      if (e.own.has(m.tag)) {
        e.own.get(m.tag)(m);
        e.own.delete(m.tag);
        return;
      }
      const to = e.tags.get(m.tag);
      e.tags.delete(m.tag);
      if (!to) return;
      let error = m.error;
      // A write refused while saturnus tx holds the line: said so.
      if (!m.ok && e.holder && e.holder !== to.conn && String(error).startsWith(PORT_HELD)) error = e.holder.who === "tx" ? LINE_TAKEN : LINE_ELSEWHERE;
      to.conn.post(m.ok ? { type: "reply", id: to.id, ok: true, result: m.result } : { type: "reply", id: to.id, ok: false, error });
      return;
    }
    if (m.type === "autoSave") {
      this.keepState(e, m);
      return;
    }
    if (m.type === "status" || m.type === "keys" || m.type === "frame") e.last[m.type] = m;
    if (m.type === "serial") {
      this.lineMoved(e);
      e.holder?.post(m);
      return;
    }
    if (m.type === "memoryChanged") {
      for (const c of e.conns) if (c.watch) c.post(m);
      return;
    }
    if (m.type === "error" && e.current) {
      if (e.conns.has(e.current)) e.current.post(m);
      return;
    }
    for (const c of e.conns) c.post(m);
  }

  /** Give `e`'s engine `m` from `conn` (null: the hub's own, no reply). */
  command(e, conn, m) {
    const { rom, state, data, bytes, ...rest } = m;
    let tag;
    if (conn && m.id !== undefined) {
      tag = this.nextTag++;
      e.tags.set(tag, { conn, id: m.id });
    }
    e.current = conn;
    try {
      e.host.command(JSON.stringify({ v: 1, ...rest }), bytesOf(rom ?? state ?? data ?? bytes), tag);
      this.deliver(e);
    } catch (err) {
      // The core failed (a wasm panic leaves it unusable): every page on it hears so, and it goes.
      this.broken(e, err);
    } finally {
      e.current = null;
    }
  }

  /** The hub's own command to `e`'s engine; resolves to its reply (an error reply after `OWN_REPLY_MS`). */
  engine(e, cmd, args = {}) {
    if (e.stopped) return Promise.resolve({ ok: false, error: "stopped" });
    const tag = this.nextTag++;
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        e.own.delete(tag);
        resolve({ ok: false, error: "no answer" });
      }, OWN_REPLY_MS);
      e.own.set(tag, (r) => {
        clearTimeout(timer);
        resolve(r);
      });
      try {
        e.host.command(JSON.stringify({ v: 1, cmd, ...args }), undefined, tag);
        this.deliver(e);
      } catch (err) {
        e.own.delete(tag);
        clearTimeout(timer);
        resolve({ ok: false, error: "stopped" });
        this.broken(e, err);
      }
    });
  }

  /**
   * Save `e` (a kept one that owes a change), stop it and drop it. Its
   * pages go home. While a send types the engine refuses: the stop is
   * retried every 250 ms outside the hub's queue (`retryStop`), so the
   * other calculators' commands do not wait for it.
   */
  async stop(e) {
    if (e.stopped || e.stopping) return;
    if (e.kept) {
      const r = await this.engine(e, "unload");
      if (!r.ok && r.error !== "stopped") {
        e.stopping = true;
        this.retryStop(e);
        return;
      }
      await this.keeping;
    }
    this.drop(e);
  }

  /** `stop`'s retries, each one a turn in the queue; a page showing `e` again (`attach`) calls it off. */
  async retryStop(e) {
    for (let i = 1; i < this.stopTries; i++) {
      await sleep(250);
      const done = await this.serial(async () => {
        if (e.stopped || !e.stopping) return true;
        const r = await this.engine(e, "unload");
        if (!r.ok && r.error !== "stopped") return false;
        await this.keeping;
        this.drop(e);
        return true;
      });
      if (done) return;
    }
    // Given up: dropped unsaved, unless a page showed it again; its pages show no calculator.
    await this.serial(() => {
      if (e.stopped || !e.stopping) return;
      for (const c of e.conns) c.post({ type: "status", model: null, romName: "", running: false, halted: null, speed: e.sentSpeed, loop: "stopped", busy: false });
      this.drop(e);
    });
  }

  /** `e` is done: its timers, its waiting commands, its lock and its core go; its pages go home. */
  drop(e) {
    e.stopped = true;
    e.stopping = false;
    clearTimeout(e.timer);
    clearTimeout(e.lineTimer);
    for (const [, resolve] of e.own) resolve({ ok: false, error: "stopped" });
    e.own.clear();
    e.releaseAuto?.();
    e.releaseAuto = null;
    e.autoWait?.abort();
    e.autoWait = null;
    if (this.machines.get(e.key) === e) this.machines.delete(e.key);
    for (const c of e.conns) {
      if (c.machine === e && c.home !== e) this.toHome(c);
    }
    e.conns.clear();
    try {
      e.host.free?.();
    } catch {
      // Freed already.
    }
  }

  /**
   * `e`'s core failed (a wasm panic): it is stopped without a save and
   * dropped. Every page on it hears `CALC_STOPPED`: its waiting replies
   * as errors, saturnus tx a `gone`, the emulator page a status without a
   * model and an `error`. A page whose home it was gets a new home at its
   * next command.
   */
  broken(e, err) {
    if (e.stopped) return;
    console.warn(`saturnus: the ${e.model ?? "page's"} calculator failed and was stopped: ${err?.message ?? err}`);
    const pages = new Set(e.conns);
    for (const c of this.conns) if (c.home === e || c.machine === e) pages.add(c);
    for (const [, to] of e.tags) to.conn.post({ type: "reply", id: to.id, ok: false, error: CALC_STOPPED });
    e.tags.clear();
    e.holder = null;
    e.conns.clear();
    this.drop(e);
    for (const c of pages) {
      if (c.home === e) c.home = null;
      if (c.machine === e) this.toHome(c);
      if (c.who === "tx") {
        c.post({ type: "gone", message: CALC_STOPPED });
      } else {
        c.post({ type: "status", model: null, romName: "", running: false, halted: null, speed: "1", loop: "stopped", busy: false });
        c.post({ type: "error", message: CALC_STOPPED });
      }
    }
  }

  /** Tell `e`'s saturnus tx pages but `except` that it is gone, and send them home. */
  dropTx(e, message, except = null) {
    for (const c of [...e.conns]) {
      if (c === except || c.who !== "tx") continue;
      c.post({ type: "gone", message });
      e.conns.delete(c);
      if (e.holder === c) e.holder = null;
      this.toHome(c);
    }
  }

  // ---------------------------------------------------------- auto-save
  // The calculator keeps its state across reloads (iteration 27): the
  // engine hands out the state once it changed and settled; it goes into
  // the model's auto slot and comes back when the model boots. The hub is
  // its only writer.

  keepState(e, { model, romName, cycles, state }) {
    this.keeping = this.keeping
      .then(async () => {
        if (!e.kept || e.autoSaveOff || this.forgotten.has(model)) return;
        await this.states.put(model, { state, saved: Date.now(), cycles, romName });
        for (const c of e.conns) c.post({ type: "autoSaved", model, cycles });
      })
      .catch((err) => console.warn(`saturnus: the ${model} state was not kept: ${err?.message ?? err}`));
  }

  /** The state kept for `model`, or null; a `fresh` start deletes it instead. */
  async keptState(model, fresh) {
    await this.keeping;
    try {
      if (fresh) {
        await this.states.delete(model);
        return null;
      }
      return await this.states.get(model);
    } catch (err) {
      console.warn(`saturnus: the kept ${model} state cannot be read: ${err?.message ?? err}`);
      return null;
    }
  }

  /**
   * `conn` shows `model`'s calculator: the one running, or booted from
   * `rom` with the state kept for it (cold if `fresh`, which also starts
   * a running one again, for every page). A state saved by a newer build
   * is not booted. Resolves to `boot`'s result; `attached` when it ran
   * already.
   */
  async bootKept(conn, model, rom, name, fresh) {
    let e = this.machines.get(model);
    const running = Boolean(e && !fresh && sameBytes(e.rom, rom));
    // Refused now (a send types): nothing is dropped or deleted, and the page stays where it was.
    if (e && !running) e.host.check(JSON.stringify({ v: 1, cmd: "boot" }));
    if (conn.machine !== e) await this.detach(conn);
    if (running) {
      this.attach(conn, e);
      return { model: e.model, romName: e.romName, restored: true, attached: true };
    }
    const created = !e;
    if (e) {
      this.dropTx(e, fresh ? "The emulator started this calculator afresh." : "The emulator started this calculator with another ROM.", conn);
      if (!fresh) {
        // Another ROM of the model: the running machine is kept first, and restored on the new ROM.
        e.host.saveNow();
        this.deliver(e);
      }
    } else {
      e = new Engine(model, { kept: true });
      this.machines.set(model, e);
    }
    try {
      const kept = await this.keptState(model, fresh);
      if (kept && tooNew(kept, state_format())) throw new Error(TOO_NEW);
      // Its pages hear the boot's status and frame.
      this.attach(conn, e);
      e.holder = null;
      const booted = e.host.boot(model, rom, name, kept ?? undefined);
      e.model = booted.model;
      e.romName = booted.romName ?? name;
      e.rom = rom;
      this.forgotten.delete(booted.model);
      if (booted.model !== model && !this.machines.has(booted.model)) {
        this.machines.delete(model);
        e.key = booted.model;
        this.machines.set(booted.model, e);
      }
      if (booted.restoreError) console.warn(`saturnus: the kept ${booted.model} state did not load (${booted.restoreError}); cold boot`);
      if (created) await this.lockAutoSave(e, booted.model);
      this.deliver(e);
      // A calculator saved in SERVER (a page closed while it served) is
      // ended as CANCEL would (web/protocol.md, `endServer`).
      if (booted.restored && SERVER_MODELS.has(booted.model)) {
        const r = await this.engine(e, "endServer");
        if (!r.ok) console.warn(`saturnus: the ${booted.model}'s server was not ended: ${r.error}`);
      }
      return booted;
    } catch (err) {
      if (created) {
        this.toHome(conn);
        e.conns.clear();
        await this.stop(e);
      }
      throw err;
    }
  }

  /**
   * The model's auto-save lock (`saturnus-auto:<model>`, a Web Lock), held
   * while `e` runs, so one engine of a model writes its auto slot: another
   * tab's own Worker (no SharedWorker, another build's) has it, so this
   * one's state is not saved and its pages hear `autoSaveOff`. Without Web
   * Locks the engine saves as before.
   */
  async lockAutoSave(e, model) {
    if (!this.locks) return;
    const got = await new Promise((resolve) => {
      this.locks.request(`saturnus-auto:${model}`, { ifAvailable: true }, (lock) => {
        if (!lock) {
          resolve(false);
          return null;
        }
        resolve(true);
        return new Promise((r) => {
          e.releaseAuto = r;
        });
      }).catch(() => resolve(false));
    });
    if (got) return;
    if (e.stopped) return;
    e.host.setAutoSave(false);
    e.autoSaveOff = autoSaveOff(model);
    for (const c of e.conns) c.post({ type: "autoSaveOff", model, message: e.autoSaveOff });
    // Waited for in the background: once the other tab lets go, this one keeps the state.
    e.autoWait = typeof AbortController === "function" ? new AbortController() : null;
    this.locks.request(`saturnus-auto:${model}`, e.autoWait ? { signal: e.autoWait.signal } : {}, () => {
      if (e.stopped) return null;
      e.autoWait = null;
      e.autoSaveOff = null;
      e.host.setAutoSave(true);
      for (const c of e.conns) c.post({ type: "autoSaveOn", model });
      return new Promise((r) => {
        e.releaseAuto = r;
      });
    }).catch(() => {});
  }

  /** After Remove ROMs: `model`'s calculator (every one's, if null) stops; no 49G state is written until it boots again. */
  async forgetKept(conn, model) {
    for (const m of ROM_HOLDING_STATES) {
      if (model === null || model === m) this.forgotten.add(m);
    }
    for (const e of [...this.machines.values()]) {
      if (model !== null && e.model !== model) continue;
      this.dropTx(e, "The emulator's ROM of this calculator was removed.", conn);
      await this.stop(e);
    }
    await this.keeping;
  }

  // ---------------------------------------------------------- ROM slots

  roms() {
    this.romStore ??= new RomStore({
      identify: (bytes) => identify_rom(bytes),
      plan: (input) => plan_roms(JSON.stringify(input)),
      download: (model) => rom_download(model),
      boot: (model, rom, name, fresh) => this.bootKept(this.romConn, model, rom, name, fresh),
      models: model_names(),
    });
    return this.romStore;
  }

  async romCommand(conn, m) {
    this.romConn = conn;
    try {
      switch (m.cmd) {
        case "romSlots": return await this.roms().slots();
        case "bootModel": return await this.roms().bootModel(String(m.model), m.fresh === true);
        case "chooseRom": return await this.roms().chooseRom(String(m.model), m.files, m.offer);
        case "forgetRom":
          await this.forgetKept(conn, m.model ?? null);
          return await this.roms().forget(m.model ?? null);
        default: return await this.roms().settings(m.bootLast);
      }
    } finally {
      this.romConn = null;
    }
  }

  // ---------------------------------------------------------- commands

  /** Answer a command the hub serves itself (also to a page it took for gone, which may still wait). */
  reply(conn, m, ok, value) {
    const always = { always: true };
    if (m.id !== undefined) {
      conn.post(ok ? { type: "reply", id: m.id, ok, result: value ?? null } : { type: "reply", id: m.id, ok, error: value }, always);
    } else if (!ok) {
      conn.post({ type: "error", message: value }, always);
    }
  }

  /** Run `fn` and answer with its result or its error. */
  async answer(conn, m, fn) {
    let ok = true;
    let value;
    try {
      value = await fn();
    } catch (err) {
      ok = false;
      value = String(err?.message ?? err);
    }
    this.reply(conn, m, ok, value);
  }

  async handle(conn, m) {
    if (conn.gone) {
      // A page that left (or a port the hub took for closed) is told, never left waiting.
      this.reply(conn, m, false, CALC_STOPPED);
      return;
    }
    try {
      await this.ready;
    } catch (err) {
      this.reply(conn, m, false, `the emulator failed to start: ${err?.message ?? err}`);
      return;
    }
    if (conn.refused) {
      this.reply(conn, m, false, conn.refused);
      return;
    }
    if (m.cmd === "join") {
      await this.answer(conn, m, () => this.join(conn, m));
      // Refused: the page goes no further, and the hub keeps nothing of it.
      if (conn.refused) this.conns.delete(conn);
      return;
    }
    if (!conn.home) {
      conn.home = new Engine(`home:${conn.id}`);
      // A first command, or its home failed: a page still on a kept machine stays there.
      if (!conn.machine) this.toHome(conn);
    }
    const e = conn.machine;
    const home = conn.home;
    // Alone on its engine (a plain Worker's page, a page's own ROM), the page's commands pass unchanged.
    const alone = e.conns.size === 1;
    const txHolds = Boolean(e.holder && e.holder !== conn && e.holder.who === "tx");
    // A reset or a state loaded under saturnus tx's server: refused until it disconnects.
    if (txHolds && REFUSED_WHILE_TX.has(m.cmd)) {
      this.reply(conn, m, false, LINE_TAKEN);
      return undefined;
    }
    // ATTN (ON) pressed, or typing, on a calculator saturnus tx serves may end its server: tx checks it before its next request.
    const attn = (m.cmd === "keyDown" && m.key === "on") || TYPING.has(m.cmd);
    if (attn && txHolds) e.holder.post({ type: "keysElsewhere" });
    switch (m.cmd) {
      case "leave": return this.leave(conn, () => this.reply(conn, m, true, null));
      case "attach":
        // saturnus tx: the calculator of a kept ROM (`rom`, `romName`), running or booted.
        return this.answer(conn, m, () => this.bootKept(conn, String(m.model), bytesOf(m.rom), String(m.romName ?? ""), false));
      case "hello":
        conn.watch = false;
        if (alone) {
          // The engine stops its watch and lets the port go too.
          e.watching = false;
          e.holder = null;
          break;
        }
        return this.answer(conn, m, () => this.hello(conn, m));
      case "boot":
        // A ROM given once: the page's own calculator, kept nowhere.
        if (e !== home) await this.detach(conn);
        home.holder = null;
        this.command(home, conn, m);
        return undefined;
      case "setSpeed":
        conn.speed = String(m.speed);
        if (conn.who === "emulator") e.emuSpeed = conn.speed;
        if (e.holder === conn) this.lineMoved(e);
        e.sentSpeed = this.speedOf(e);
        this.command(e, conn, { ...m, speed: e.sentSpeed });
        return undefined;
      case "visibility":
        conn.hidden = Boolean(m.hidden);
        e.sentHidden = this.hiddenOf(e);
        this.command(e, conn, { ...m, hidden: e.sentHidden });
        return undefined;
      case "watchMemory":
        conn.watch = Boolean(m.on);
        e.watching = [...e.conns].some((c) => c.watch);
        // The page's own answer (`{supported, reason}`), the engine watching while any page does.
        this.command(e, conn, { ...m, on: e.watching });
        return undefined;
      case "serialOpen":
        if (e.holder && e.holder !== conn) {
          this.reply(conn, m, false, e.holder.who === "tx" ? LINE_TAKEN : LINE_ELSEWHERE);
          return undefined;
        }
        e.holder = conn;
        break;
      case "serialWrite":
        if (e.holder && e.holder !== conn) {
          this.reply(conn, m, false, "the serial port is not open (serialOpen first)");
          return undefined;
        }
        break;
      case "serialClose":
        if (e.holder && e.holder !== conn) {
          this.reply(conn, m, true, null);
          return undefined;
        }
        e.holder = null;
        break;
      case "unload":
        if (e === home) break;
        if (!alone) {
          // Other pages still show it: this one only lets go, and shows its home (no ROM).
          await this.detach(conn);
          conn.post({ type: "status", model: null, romName: "", running: false, halted: null, speed: home.sentSpeed, loop: "stopped", busy: false });
          this.reply(conn, m, true, null);
          return undefined;
        }
        {
          // The last page: saved and stopped (its status without a model went out); refused while a send types, in the engine's words.
          const r = await this.engine(e, "unload");
          if (!r.ok && r.error !== "stopped") {
            this.reply(conn, m, false, r.error);
            return undefined;
          }
          await this.keeping;
          this.drop(e);
          this.reply(conn, m, true, null);
          return undefined;
        }
      default:
        if (ROM_COMMANDS.has(m.cmd)) {
          let ok = true;
          let value;
          try {
            // The state machine checks it first (the version, the refusals during a send).
            e.host.check(JSON.stringify({ v: m.v, cmd: m.cmd }));
            value = await this.romCommand(conn, m);
          } catch (err) {
            ok = false;
            value = String(err?.message ?? err);
          }
          // The events the command caused (a boot's) go out before its reply.
          this.deliver(conn.machine);
          this.reply(conn, m, ok, value);
          return undefined;
        }
    }
    // saturnus tx working the calculator (SERVER typed, a probe): its speed for a while.
    if (e.holder === conn) this.lineMoved(e);
    this.command(e, conn, m);
    return undefined;
  }

  /**
   * `hello` from a page that shares its machine: answered by its home
   * engine (the machine's other pages keep their watch and serial line);
   * the machine's status, keys and frame go to it again before the reply.
   */
  async hello(conn, m) {
    const e = conn.machine;
    this.settle(e);
    const r = await this.engine(conn.home, "hello", { v: m.v });
    if (!r.ok) throw new Error(r.error);
    for (const k of ["status", "keys", "frame"]) {
      if (e.last[k]) conn.post(e.last[k]);
    }
    return r.result;
  }
}
