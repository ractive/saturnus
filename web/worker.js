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
/** Shortest time between two `memoryChanged` events, in ms. */
const MEMORY_EVENT_MS = 250;
/** Shortest time between two looks at the user memory, in ms. */
const MEMORY_LOOK_MS = 100;
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

/**
 * `watchMemory` and the `memoryChanged` event: whether a page watches, the
 * change counter (or read error) it knows, when it was read and the
 * machine's cycle count then (no cycles, no change), and whether memory
 * changed without cycles (a new machine, a loaded state).
 */
const memory = { on: false, last: null, at: -Infinity, told: -Infinity, cycles: null, force: false, timer: 0 };

const stats = { workMs: 0, ticks: 0, wakes: 0, memoryLooks: 0, memoryMs: 0 };

/**
 * A send typing into the command line (insert, run, replace), or null:
 * `{resolve, reject, freezes, started, timer}`. While it runs the run loop
 * is stopped and the send drives the machine.
 */
let typing = null;
let lastStatus = "";

function post(msg) {
  self.postMessage(msg);
}

function loopState() {
  return passTimer ? "frame" : wakeTimer ? "sleep" : "stopped";
}

/** The user memory's change counter, or why it cannot be read. */
function memoryState() {
  if (!emu) return null;
  try {
    return emu.memory_changes();
  } catch (err) {
    return `error: ${err}`;
  }
}

/**
 * Tell a watching page that the user memory changed, if it did: looked at
 * only while the calculator is not computing (the ROM's structures are
 * whole when it waits for a key), only after it ran, at most every
 * MEMORY_LOOK_MS, and not within MEMORY_EVENT_MS of the last event; a
 * look that comes too early is made by a timer, so a sleeping page still
 * hears of the last change.
 */
function pollMemory() {
  if (!memory.on || !emu || memory.timer) return;
  if (running && (emu.idle_ms() < 0 || emu.keys_busy())) return;
  const cycles = emu.cycles();
  if (cycles === memory.cycles && !memory.force) return;
  const wait = Math.max(memory.at + MEMORY_LOOK_MS, memory.told + MEMORY_EVENT_MS) - performance.now();
  if (wait > 0) {
    memory.timer = setTimeout(() => {
      memory.timer = 0;
      pollMemory();
    }, wait);
    return;
  }
  memory.at = performance.now();
  memory.cycles = cycles;
  memory.force = false;
  const state = memoryState();
  stats.memoryLooks++;
  stats.memoryMs += performance.now() - memory.at;
  if (state !== memory.last) {
    memory.last = state;
    memory.told = memory.at;
    post({ type: "memoryChanged" });
  }
}

/** Post the status if it changed, and any new frame, keys and errors. */
function flush() {
  if (emu) {
    for (const message of emu.take_errors()) post({ type: "error", message });
    const keys = emu.take_keys();
    if (keys) post(JSON.parse(keys));
    // A long send holds the last frame until it is done.
    const frame = typing?.freezes ? null : emu.take_frame();
    if (frame) post(JSON.parse(frame));
  }
  postStatus();
}

/** Post the status if it changed. */
function postStatus() {
  const busy = Boolean(typing?.freezes);
  const status = { type: "status", model, romName, running, halted, speed, loop: loopState(), busy };
  const text = JSON.stringify(status);
  if (text !== lastStatus) {
    lastStatus = text;
    post(status);
  }
  pollMemory();
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
  // A send drives the machine itself; the loop resumes when it ends.
  if (typing) return;
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
  if (!hidden && running && !passTimer && !wakeTimer && !typing) scheduleNext();
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
  if (typing) throw new Error("typing is in progress");
  return emu;
}

// ---- Typing (protocol.md, "Typing") ----

/** Emulated ms per step of a send. */
const TYPING_STEP_MS = 20;
/** Wall time one tick of a send may take before it yields to messages. */
const TYPING_TICK_MS = 40;
/** Wall time a send may take in all (as the native hosts' key scripts). */
const TYPING_LIMIT_MS = 30000;

/**
 * Type `text` with `verb` ("insert", "run", "replace"): stop the run loop,
 * step the send in ticks that yield to other messages, then resume
 * pacing from where it left the machine. Resolves with the send's result.
 */
function startTyping(verb, text) {
  const e = requireEmu();
  if (typeof text !== "string") throw new Error('missing string field "text"');
  if (halted !== null) throw new Error(`the CPU is halted: ${halted}`);
  // A sleeping machine first catches up the time that passed.
  if (running && wakeTimer) wake();
  const freezes = e.start_typing(verb, text);
  stopLoop();
  return new Promise((resolve, reject) => {
    typing = { resolve, reject, freezes, started: performance.now(), timer: 0 };
    flush();
    typing.timer = setTimeout(typingTick, 0);
  });
}

function typingTick() {
  const t = typing;
  if (!t) return;
  const start = performance.now();
  let done = false;
  let error = null;
  try {
    while (!done && performance.now() - start < TYPING_TICK_MS) {
      done = emu.typing_step(TYPING_STEP_MS);
    }
    if (!done && performance.now() - t.started > TYPING_LIMIT_MS) {
      error = `typing ran out of wall-clock time (${TYPING_LIMIT_MS / 1000} s)`;
    }
  } catch (err) {
    error = String(err?.message ?? err);
  } finally {
    stats.workMs += performance.now() - start;
  }
  if (!done && error === null) {
    flush();
    t.timer = setTimeout(typingTick, 0);
    return;
  }
  endTyping(error);
}

/** Finish the send: its result, or `error` (the send is stopped). */
function endTyping(error) {
  const t = typing;
  if (!t) return;
  clearTimeout(t.timer);
  let result = null;
  if (error === null) {
    try {
      result = emu.typing_result();
    } catch (err) {
      error = String(err?.message ?? err);
    }
  }
  if (error !== null) emu.stop_typing();
  typing = null;
  // The status says the screen is live again before its frame.
  if (t.freezes) postStatus();
  if (error !== null && error.includes("CPU halted")) halt(error);
  else setRunning(running);
  flush();
  if (error === null) t.resolve(result);
  else t.reject(new Error(error));
}

const handlers = {
  hello() {
    // The current state goes out again with this reply (protocol.md); a
    // Worker lives as long as its page, so this matters only to a second
    // view on the same Worker.
    lastStatus = "";
    memory.on = false;
    emu?.invalidate();
    return { protocol: PROTOCOL, host: "worker", models: model_names() };
  },
  skin: (m) => skin(m.model),
  layout: (m) => layout(m.model),
  boot(m) {
    // A send drives the machine; a new one is not swapped in under it.
    if (typing) throw new Error("typing is in progress: stop it (releaseAll) before booting");
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
    memory.force = true;
    halted = null;
    setRunning(true);
    return { model, romName };
  },
  // Key commands refuse an unknown key or a missing machine (an error
  // reply, or an `error` event without an id), as the native hosts do.
  keyDown(m) {
    const e = requireEmu();
    const key = String(m.key);
    if (!e.press(key)) throw new Error(`no key ${JSON.stringify(key)} on the ${e.model()}`);
    afterKeys();
  },
  keyUp(m) {
    const e = requireEmu();
    const key = String(m.key);
    if (!e.has_key(key)) throw new Error(`no key ${JSON.stringify(key)} on the ${e.model()}`);
    e.release(key);
    e.pump();
  },
  keyUpAll() {
    // The window lost the focus during a send: the send holds no queued
    // key, and its own key comes up when its press ends.
    if (!emu || typing) return;
    emu.release_held();
    emu.pump();
  },
  typeLetter(m) {
    if (!emu) return false;
    if (typing) throw new Error("typing is in progress");
    const ok = emu.type_letter(String(m.letter));
    afterKeys();
    return ok;
  },
  typeKeys(m) {
    const e = requireEmu();
    if (!Array.isArray(m.keys) || !m.keys.every((k) => typeof k === "string")) {
      throw new Error('"keys" must hold key names (strings)');
    }
    const bad = m.keys.find((k) => !e.has_key(k));
    if (bad !== undefined) throw new Error(`no key ${JSON.stringify(bad)} on the ${e.model()}`);
    e.type_keys(m.keys.join(" "));
    afterKeys();
  },
  releaseAll() {
    // Also stops a send in progress (its reply is an error).
    if (typing) endTyping("cancelled");
    emu?.release_keys();
  },
  commandLine() {
    if (!emu) throw new Error("no ROM loaded");
    return emu.command_line();
  },
  insert: (m) => startTyping("insert", m.text),
  typeText: (m) => startTyping("insert", m.text),
  run: (m) => startTyping("run", m.text),
  replace: (m) => startTyping("replace", m.text),
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
    memory.force = true;
    e.invalidate();
    setRunning(running);
    return {};
  },
  visibility: (m) => setHidden(m.hidden),
  // The read-only view of the user memory (48SX, 48GX, 49G), straight
  // from RAM; the other models refuse with the reason.
  watchMemory(m) {
    memory.on = Boolean(m.on);
    // The page reads after this reply: events are for what changes from
    // here on.
    memory.last = memoryState();
    memory.at = -Infinity;
    memory.told = -Infinity;
    memory.force = false;
    if (!emu) return { supported: null, reason: null };
    memory.cycles = emu.cycles();
    const reason = emu.memory_refusal() ?? null;
    return { supported: reason === null, reason };
  },
  memoryTree: () => requireEmu().memory_tree(),
  stack: () => requireEmu().stack(),
  flags: () => requireEmu().flags(),
  objectAt(m) {
    const address = Number(m.address);
    if (!Number.isInteger(address) || address < 0 || address > 0xfffff) {
      throw new Error("address is outside the address space");
    }
    return requireEmu().object_at(address);
  },
  stats: () => ({
    cycles: emu ? emu.cycles() : 0,
    emulatedMs: emu ? emu.emulated_ms() : 0,
    workMs: stats.workMs,
    ticks: stats.ticks,
    wakes: stats.wakes,
    memoryLooks: stats.memoryLooks,
    memoryMs: stats.memoryMs,
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
    // Typing replies when the send is done; other messages are served
    // meanwhile.
    value = await handler(m);
  } catch (err) {
    ok = false;
    value = String(err?.message ?? err);
  }
  // The events the command caused go out before its reply: a caller that
  // has the reply has the state (web/protocol.md).
  flush();
  reply(ok, value);
};
