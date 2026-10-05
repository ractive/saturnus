// saturnus Web Worker: owns the wasm machine, paces it against the wall
// clock and pushes events to the page (web/protocol.md). While the
// calculator computes it runs on a ~60 Hz timer; while it sleeps in SHUTDN
// with nothing queued it stops and sets a timer for the next timer event.

import init, { Emulator, layout, model_for, model_names, skin } from "./pkg/saturnus_web.js";

const PROTOCOL = 1;
/** Run pass period while the calculator computes, in ms (an animation frame). */
const FRAME_MS = 1000 / 60;
/** Longest stretch of wall time one pass may make up for. */
const MAX_FRAME_MS = 100;
/** At "Max" speed, the wall time a pass may spend emulating. */
const MAX_BUDGET_MS = 11;
/** At "Max" speed, the emulated time one pass may run at most. */
const MAX_EMULATED_PER_FRAME_MS = 1000;
/** Emulated time per wall time while sleeping at "Max" (the wake timer). */
const MAX_RATE = 60;
/**
 * Most emulated time a wake catches up, in ms: 12 hours. Idle time is
 * cheap (the core jumps over SHUTDN; only the ROM's timer ticks run: an
 * idle 48SX hour took 2 ms, 12 hours 9 ms), but owed time the ROM spends
 * awake (an alarm, a running program) is computed for real at the pass
 * budget; the bound keeps a computer that slept for days from paying that
 * off for minutes afterwards.
 */
const MAX_BEHIND_MS = 12 * 3600 * 1000;
/** Wall time one wake may spend catching up while the page is visible. */
const WAKE_BUDGET_MS = 22;
/** The same while hidden, where no frame is waiting for it. */
const HIDDEN_WAKE_BUDGET_MS = 200;
const SPEEDS = ["1", "2", "4", "max"];

let emu = null;
let model = null;
let romName = "";
/** The user's Run/Pause switch. */
let running = false;
let halted = null;
let speed = "1";
/** The page is hidden: no passes while computing (as no animation frames). */
let hidden = false;

// The run loop: a pass timer while the calculator computes, a wake timer
// while it sleeps in SHUTDN with nothing queued.
let passTimer = 0;
let lastPass = null;
let wakeTimer = 0;
/** Wall clock up to which emulated time is accounted for while asleep. */
let sleptAt = 0;
/**
 * Emulated ms owed to wall time: what a wake could not catch up within its
 * budget. Passes and further wakes run it off before their own share.
 */
let behindMs = 0;

const stats = { workMs: 0, ticks: 0, wakes: 0 };
let lastStatus = "";

function post(msg) {
  self.postMessage(msg);
}

function loopState() {
  return passTimer ? "frame" : wakeTimer ? "sleep" : "stopped";
}

/** Post the status if it changed, and any new frame, keys and errors. */
function flush() {
  if (emu) {
    for (const message of emu.take_errors()) post({ type: "error", message });
    const keys = emu.take_keys();
    if (keys) post(JSON.parse(keys));
    const frame = emu.take_frame();
    if (frame) post(JSON.parse(frame));
  }
  const status = { type: "status", model, romName, running, halted, speed, loop: loopState() };
  const text = JSON.stringify(status);
  if (text !== lastStatus) {
    lastStatus = text;
    post(status);
  }
}

/** Emulated ms per wall ms while asleep, for the wake timer. */
function rate() {
  return speed === "max" ? MAX_RATE : Number(speed);
}

/**
 * Run `ms` of emulated time in slices, feeding the key queue unless `keys`
 * is false; a sleeping CPU with no keys to time skips to its next timer
 * event in one step. Returns the emulated ms the budget left unrun.
 */
function runSlices(ms, budgetMs, keys = true) {
  const start = performance.now();
  let left = ms;
  try {
    while (left > 0) {
      left -= emu.run_slice(left, keys);
      if (budgetMs !== undefined && performance.now() - start > budgetMs) break;
    }
  } finally {
    stats.workMs += performance.now() - start;
  }
  return Math.max(left, 0);
}

function halt(err) {
  halted = String(err);
  setRunning(false);
}

function pass() {
  passTimer = 0;
  const t = performance.now();
  const wall = lastPass === null ? 0 : Math.min(Math.max(t - lastPass, 0), MAX_FRAME_MS);
  lastPass = t;
  if (!emu || !running) return;
  stats.ticks++;
  try {
    const max = speed === "max";
    const own = max ? MAX_EMULATED_PER_FRAME_MS : wall * Number(speed);
    const left = runSlices(behindMs + own, max ? MAX_BUDGET_MS : MAX_BUDGET_MS * 2);
    // Time owed from a sleep runs first and is kept until paid; the
    // pass's own share that did not fit is dropped, as ever.
    behindMs = Math.max(left - own, 0);
  } catch (err) {
    halt(err);
    flush();
    return;
  }
  scheduleNext();
  flush();
}

/**
 * Keep running passes while the calculator computes or keys are queued;
 * once it sleeps in SHUTDN with nothing to do, stop and set a timer for
 * its next timer event instead, so an idle page costs nothing. Time still
 * owed brings the timer forward to now.
 */
function scheduleNext() {
  if (!emu || !running) return;
  const idle = emu.idle_ms();
  if (idle < 0 || emu.keys_busy()) {
    if (!passTimer && !hidden) {
      const due = lastPass === null ? 0 : Math.max(0, lastPass + FRAME_MS - performance.now());
      passTimer = setTimeout(pass, due);
    }
    return;
  }
  clearTimeout(wakeTimer);
  // The last pass or wake accounted for wall time up to `lastPass`.
  sleptAt = lastPass ?? performance.now();
  lastPass = null;
  const delay = behindMs > 0 ? 0 : Math.min(idle / rate(), 2 ** 31 - 1) + 1;
  wakeTimer = setTimeout(wake, delay);
}

/**
 * Leave the sleep: run all the emulated time that passed meanwhile (cheap
 * while the CPU sleeps), however late the timer fired. What does not fit
 * the wall-time budget, as when the ROM wakes up and computes, stays owed
 * for the next pass or wake.
 */
function wake() {
  if (!emu || !running || !wakeTimer) return;
  clearTimeout(wakeTimer);
  wakeTimer = 0;
  stats.wakes++;
  const now = performance.now();
  behindMs = Math.min(behindMs + (now - sleptAt) * rate(), MAX_BEHIND_MS);
  try {
    // A key that woke the machine goes down after the time that passed.
    behindMs = runSlices(behindMs, hidden ? HIDDEN_WAKE_BUDGET_MS : WAKE_BUDGET_MS, false);
  } catch (err) {
    halt(err);
  }
  lastPass = now;
  // The ROM may have woken for a timer event and redrawn (the clock), then
  // gone back to sleep: show it, and sleep on without a pass.
  scheduleNext();
  flush();
}

function stopLoop() {
  clearTimeout(passTimer);
  passTimer = 0;
  clearTimeout(wakeTimer);
  wakeTimer = 0;
  lastPass = null;
  behindMs = 0;
}

function setRunning(on) {
  running = on && emu !== null && halted === null;
  stopLoop();
  if (running && !hidden) passTimer = setTimeout(pass, 0);
  else if (running) scheduleNext();
}

function setSpeed(value) {
  // A sleeping calculator first catches up at the old rate ...
  if (running && wakeTimer) wake();
  speed = SPEEDS.includes(value) ? value : "1";
  // ... and sleeps on the new schedule.
  if (running && wakeTimer) scheduleNext();
}

function setHidden(h) {
  hidden = Boolean(h);
  if (!hidden && running && !passTimer && !wakeTimer) scheduleNext();
}

/**
 * A command queued keys: wake a sleeping machine (its time first, then
 * the keys; the wake starts passes for the queue), then feed the queue.
 */
function afterKeys() {
  wake();
  emu.pump();
}

function requireEmu() {
  if (!emu) throw new Error("no ROM loaded");
  return emu;
}

const handlers = {
  hello: () => ({ protocol: PROTOCOL, host: "worker", models: model_names() }),
  skin: (m) => skin(m.model),
  layout: (m) => layout(m.model),
  boot(m) {
    const rom = m.rom instanceof Uint8Array ? m.rom : new Uint8Array(m.rom);
    const chosen = model_for(rom, m.model);
    stopLoop();
    let next;
    try {
      next = new Emulator(chosen, rom);
    } catch (err) {
      // The machine that ran before keeps running.
      setRunning(running);
      throw err;
    }
    if (emu) emu.free();
    emu = next;
    model = chosen;
    romName = String(m.romName ?? "");
    halted = null;
    setRunning(true);
    return { model, romName };
  },
  keyDown(m) {
    if (!emu) return;
    emu.press(String(m.key));
    afterKeys();
  },
  keyUp(m) {
    if (!emu) return;
    emu.release(String(m.key));
    emu.pump();
  },
  keyUpAll() {
    if (!emu) return;
    emu.release_held();
    emu.pump();
  },
  typeLetter(m) {
    if (!emu) return false;
    const ok = emu.type_letter(String(m.letter));
    afterKeys();
    return ok;
  },
  typeKeys(m) {
    if (!emu) return;
    emu.type_keys((m.keys ?? []).join(" "));
    afterKeys();
  },
  releaseAll() {
    emu?.release_keys();
  },
  setSpeed: (m) => setSpeed(String(m.speed)),
  pause: (m) => setRunning(!m.paused),
  reset() {
    const e = requireEmu();
    e.release_keys();
    e.reset();
    halted = null;
    setRunning(true);
  },
  saveState() {
    const e = requireEmu();
    const state = e.save_state();
    return { state, cycles: e.cycles() };
  },
  loadState(m) {
    const e = requireEmu();
    e.release_keys();
    e.load_state(m.state instanceof Uint8Array ? m.state : new Uint8Array(m.state));
    halted = null;
    e.invalidate();
    setRunning(running);
    return {};
  },
  visibility: (m) => setHidden(m.hidden),
  stats: () => ({
    cycles: emu ? emu.cycles() : 0,
    emulatedMs: emu ? emu.emulated_ms() : 0,
    workMs: stats.workMs,
    ticks: stats.ticks,
    wakes: stats.wakes,
    loop: loopState(),
    // Emulated time owed to the wall clock: unpaid, plus the sleep so far.
    owedMs: behindMs + (wakeTimer ? (performance.now() - sleptAt) * rate() : 0),
    nowMs: performance.now(),
  }),
};

const ready = init();

self.onmessage = async (e) => {
  const m = e.data ?? {};
  const reply = (ok, value) => {
    if (m.id === undefined) {
      if (!ok) post({ type: "error", message: value });
      return;
    }
    post(ok ? { type: "reply", id: m.id, ok, result: value ?? null } : { type: "reply", id: m.id, ok, error: value });
  };
  let ok = true;
  let value;
  try {
    await ready;
    if (m.v !== PROTOCOL) throw new Error(`protocol version ${m.v} not supported (this host speaks ${PROTOCOL})`);
    const handler = Object.hasOwn(handlers, m.cmd) ? handlers[m.cmd] : null;
    if (!handler) throw new Error(`unknown command ${JSON.stringify(m.cmd)}`);
    value = handler(m);
  } catch (err) {
    ok = false;
    value = String(err?.message ?? err);
  }
  // The events the command caused go out before its reply: a caller that
  // has the reply has the state (web/protocol.md).
  flush();
  reply(ok, value);
};
