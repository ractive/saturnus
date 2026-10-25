// The ON-key combinations (iteration 65): ON held down while another
// key (or two) is pressed; on the 42S its EXIT key, on the 27S, 17B and
// 17BII their CLR key, which sit where ON does. The ON key's menu on the
// skin (a long press or a right-click, sat-calculator.js `onMenu`) and
// the command palette (app.js) offer them per model, from the table
// alone (the chords differ by model). A combination is played as held
// keys (`keyDown` and `keyUp`, web/protocol.md): all of them sent in one
// go, so they go down in the same instant (the Pioneers' memory clear
// takes only that, wiki: hardware/hp42s-keyboard), held until the
// machine has had them all down (and `hold` ms more where a model wants
// a long hold), then released, ON last. The rest is pure:
// web/test/onkeys.test.mjs.

import { confirmAction } from "./shared/components/confirm.js";

/** The question before a memory reset whose ROM then asks "Try To Recover Memory?" (the 48 series, the 49G). */
const RECOVER = (no) => ({
  title: "Reset the calculator's memory?",
  body: `The calculator asks "Try To Recover Memory?" next: NO (${no[0]}) clears its variables and stack, YES (${no[1]}) tries to keep them. Saved states are not touched.`,
  action: "Reset memory",
});

/** The question before a memory clear that asks nothing more (the 28S, the aplet and Pioneer models). */
const CLEAR = {
  title: "Clear the calculator's memory?",
  body: "Everything in the calculator's memory is erased at once, as on a new calculator. Saved states are not touched.",
  action: "Clear memory",
};

/**
 * A menu key's combinations on the models whose six menu keys are a to f
 * (numbered from the left); `hold` the ms Restart is held (the 39G's
 * continuous tests take it only held).
 */
const menuKeys = (on, restart, reset, extra = [], hold = 0) => [
  { id: "restart", title: "Restart", keys: ["on", "c"], chord: `${on}+menu key 3`, description: restart, ...(hold ? { hold } : {}) },
  ...extra,
  { id: "reset-memory", title: "Clear memory…", keys: ["on", "a", "f"], chord: `${on}+menu keys 1 and 6`, description: `${reset} Asks first.`, ask: CLEAR },
];

/**
 * Per model: `{id, title, keys, chord?, description, ask?, hold?}`.
 * `keys` are held in order (ON first); `chord` is how the menu and the
 * palette write it (default the keys' names in capitals, ON+C); `ask` is
 * the question asked before (`confirmAction`); `hold` the ms all keys
 * stay down once the machine has them.
 */
export const ON_COMBOS = {
  // wiki: hardware/hp48gx-keyboard ("ON-key combinations": FAQ 4.6, and
  // what ROM R does on saturnus)
  "48gx": [
    { id: "restart", title: "Restart", keys: ["on", "c"], description: "ON+C: clears the stack and keeps the variables." },
    { id: "self-test", title: "Self-test", keys: ["on", "d"], description: "ON+D: the calculator's interactive self-test." },
    { id: "print-display", title: "Print display", keys: ["on", "1"], description: "ON+1: sends the display to the printer." },
    { id: "reset-memory", title: "Reset memory…", keys: ["on", "a", "f"], description: "ON+A+F: the calculator asks \"Try To Recover Memory?\"; NO clears it. Asks first.", ask: RECOVER("FA") },
  ],
  // wiki: hardware/hp48sx-keyboard ("ON-key combinations": Owner's
  // Manual p. A-11 to A-13, 32-4, and ROM J on saturnus)
  "48sx": [
    { id: "restart", title: "Restart", keys: ["on", "c"], description: "ON+C: clears the stack and keeps the variables." },
    { id: "self-test", title: "Self-test", keys: ["on", "d"], description: "ON+D: the calculator's interactive tests." },
    // "Prints the display; the key's left-shift label is PRINT" (p. 32-4); ON+1 does nothing on the SX.
    { id: "print-display", title: "Print display", keys: ["on", "mth"], description: "ON+MTH: sends the display to the printer." },
    // "when A and F were held together with ON for a second or more; a half-second hold did nothing".
    { id: "reset-memory", title: "Reset memory…", keys: ["on", "a", "f"], hold: 1200, description: "ON+A+F, held a second: the calculator asks \"Try To Recover Memory?\"; NO clears it. Asks first.", ask: RECOVER("FA") },
  ],
  // wiki: hardware/hp49g-keyboard ("ON-key combinations": User's Manual
  // p. D-4, D-5, FAQ 5.8, and ROM 2.10 on saturnus); F1 to F6 take the
  // 48's A to F.
  "49g": [
    { id: "restart", title: "Restart", keys: ["on", "c"], chord: "ON+F3", description: "ON+F3: clears the stack and keeps the variables." },
    // The Tests menu lists "9 FROM Format", a format of the flash (observed).
    {
      id: "self-test", title: "Self-test…", keys: ["on", "d"], chord: "ON+F4", description: "ON+F4: the \"Tests\" menu; a digit picks a test, Q reboots; 9 formats the flash. Asks first.",
      ask: { title: "Open the 49G's tests menu?", body: "A digit picks a test and Q reboots. Test 9 (FROM Format) formats the flash memory, erasing what is stored in it. Saved states are not touched.", action: "Open tests" },
    },
    { id: "reset-memory", title: "Reset memory…", keys: ["on", "a", "f"], chord: "ON+F1+F6", description: "ON+F1+F6: the calculator asks \"Try To Recover Memory?\"; NO clears it. Asks first.", ask: RECOVER(["F6", "F1"]) },
  ],
  // wiki: hardware/hp28s-keyboard ("ON-key combinations": Owner's Manual
  // p. 20, 216, 217, and ROM 2BB on saturnus); the menu keys named by
  // their cursor labels INS DEL ▲ ▼ ◀ ▶.
  "28s": [
    { id: "restart", title: "Restart", keys: ["on", "c"], chord: "ON+▲", description: "ON+▲ (third menu key): System Halt, clears the stack." },
    // "Leaving the tests": ON alone does nothing in it, System Halt leaves it.
    { id: "self-test", title: "Self-test", keys: ["on", "e"], chord: "ON+◀", description: "ON+◀ (fifth menu key): the self-test, repeating. Restart (ON+▲) leaves it, with the stack cleared." },
    { id: "print-display", title: "Print display", keys: ["on", "letterl"], chord: "ON+L", description: "ON+L: sends the display to the printer." },
    { id: "reset-memory", title: "Clear memory…", keys: ["on", "a", "f"], chord: "ON+INS+▶", description: "ON+INS+▶ (menu keys 1 and 6): Memory Reset, \"Memory Lost\". Asks first.", ask: CLEAR },
  ],
  // wiki: hardware/hp38g-39g-keyboard ("ON-key combinations": 38G guide
  // p. 9-8, and A1.67 on saturnus; "Leaving the tests": ON alone does
  // nothing in them, ON + menu key 3 leaves both, memory kept)
  "38g": menuKeys("ON", "ON+menu key 3: reset, back to HOME with the history cleared.", "ON+menu keys 1 and 6: erases all memory (\"Memory Clear\").", [
    { id: "self-test", title: "Self-test", keys: ["on", "d"], chord: "ON+menu key 4", description: "ON+menu key 4: test mode; menu keys 1 to 6 run single tests. Restart (ON+menu key 3) leaves it." },
    { id: "continuous-test", title: "Continuous test", keys: ["on", "e"], chord: "ON+menu key 5", description: "ON+menu key 5: the tests, repeating. Restart (ON+menu key 3) leaves it." },
  ]),
  // wiki: hardware/hp42s-keyboard ("EXIT-key combinations": Owner's
  // Manual p. 261, 266-268, and the 42S ROM on saturnus)
  "42s": [
    { id: "restart", title: "Restart", keys: ["on", "sqrt"], chord: "EXIT+√x", description: "EXIT+√x: \"Machine Reset\", memory kept; also stops the self-test." },
    { id: "self-test", title: "Self-test", keys: ["on", "ln"], chord: "EXIT+LN", description: "EXIT+LN: the self-test, repeating; Restart (EXIT+√x) stops it." },
    // "only when EXIT and Σ+ went down in the same instant", as every combination here does.
    { id: "reset-memory", title: "Clear memory…", keys: ["on", "sigmaplus", "xeq"], chord: "EXIT+Σ++XEQ", description: "EXIT+Σ++XEQ: clears all memory and flags (\"Memory Clear\"). Asks first.", ask: CLEAR },
  ],
  // wiki: hardware/hp19bii-keyboard ("ON-key combinations": Owner's
  // Manual p. 204, 276, and the 19BII ROM on saturnus)
  "19bii": [
    // Held 2 s: at some steps of the self-test a shorter hold (1 s) was not taken (observed, 19BII ROM).
    { id: "restart", title: "Restart", keys: ["on", "c"], chord: "ON+menu key 3", description: "ON+menu key 3, held 2 s: \"MACHINE RESET\"; also leaves the self-test.", hold: 2000 },
    // "Leaving the tests": another key stops it with a FAIL message, Restart leaves it.
    { id: "self-test", title: "Self-test", keys: ["on", "e"], chord: "ON+menu key 5", description: "ON+menu key 5: the self-test, repeating. Restart (ON+menu key 3 held about 2 s) leaves it, memory kept." },
    { id: "print-display", title: "Print display", keys: ["on", "prt"], chord: "ON+PRNT", description: "ON+PRNT: prints the display, menu labels included." },
    { id: "reset-memory", title: "Clear memory…", keys: ["on", "a", "f"], chord: "ON+menu keys 1 and 6", description: "ON+menu keys 1 and 6: erases continuous memory (\"MEMORY LOST\"). Asks first.", ask: CLEAR },
  ],
};
// wiki: hardware/hp38g-39g-keyboard (39G/40G guide R-4, R-5, and
// rom.39g on saturnus; the 40G gave the same): the 38G's two, and the
// "Tests" menu behind menu key 4, which ON leaves. "Leaving the tests":
// the continuous test behind menu key 5 ignores ON and is left only with
// ON + menu key 3 held (2 s left it every time, 0.4 s not always), so
// Restart holds it that long.
ON_COMBOS["39g"] = menuKeys("ON", "ON+menu key 3, held 2 s: reset, back to HOME with the history cleared; also leaves the tests.", "ON+menu keys 1 and 6: erases all memory (\"Memory Clear\").", [
  { id: "self-test", title: "Self-test", keys: ["on", "d"], chord: "ON+menu key 4", description: "ON+menu key 4: the \"Tests\" menu; a digit picks a test, ON reboots to HOME." },
  { id: "continuous-test", title: "Continuous test", keys: ["on", "e"], chord: "ON+menu key 5", description: "ON+menu key 5: the tests, repeating; ON does not stop them. Restart (ON+menu key 3 held about 2 s) leaves them." },
], 2000);
ON_COMBOS["40g"] = ON_COMBOS["39g"];
// wiki: hardware/hp27s-17b-17bii-keyboard ("CLR-key combinations": 27S
// p. 195, 196, 17BII and 17B p. 218, and their ROMs on saturnus): CLR
// sits where ON does; the memory clear only with all three keys going
// down together, as every combination here does. "Leaving the tests":
// CLR + third menu key leaves the self-test; another key stops it with a
// FAIL message that only means a key was pressed.
for (const m of ["27s", "17b", "17bii"]) {
  ON_COMBOS[m] = menuKeys("CLR", "CLR+menu key 3: \"MACHINE RESET\", memory kept.", "CLR+menu keys 1 and 6: erases continuous memory (\"MEMORY LOST\").", [
    { id: "self-test", title: "Self-test", keys: ["on", "e"], chord: "CLR+menu key 5", description: "CLR+menu key 5: the self-test, repeating. Restart (CLR+menu key 3) leaves it, memory kept; another key stops it with a FAIL message." },
  ]);
}

/** How a combination is written: its `chord`, or its keys in capitals joined by +. */
export function chordOf(combo) {
  return combo.chord ?? combo.keys.map((k) => k.toUpperCase()).join("+");
}

/** The ON combinations of `model` (none for a model without a table). */
export function onCombos(model) {
  return ON_COMBOS[model] ?? [];
}

/** How long a press on ON is held before its menu opens in place of the press, in ms. */
export const ON_LONG_PRESS = 500;

/**
 * What a `pointerdown` on the ON key of a model with combinations does:
 * "menu" for a right-click (a mouse's secondary button with no modifier),
 * "press" for a Ctrl or Option/Alt click (its shifted function, at once;
 * a Mac's Ctrl+click is a secondary click with Ctrl, still a press),
 * "wait" otherwise: the press waits for the release (a tap: ON) or for
 * `ON_LONG_PRESS` (the menu, and no press). Sending ON at once would
 * cancel an open command line before the menu could open.
 */
export function onPointerDown(e, modifier) {
  if (e.pointerType === "mouse" && modifier) return "press";
  if (e.pointerType === "mouse" && e.button === 2) return "menu";
  if (e.pointerType === "mouse" && e.button !== 0) return "press";
  return "wait";
}

/**
 * Play `keys` as one combination on `backend`, watching `store`'s
 * `keysDown`: every key down at once (in order, in one go), held until
 * the machine has them all down and `hold` ms more (`pause(hold)`, by
 * default wall time; `runCombo` counts emulated time), then the others
 * released before the first (ON). Resolves true, or false when the
 * machine has not taken them within `timeout` ms (waiting for the keys
 * to go down, and again for them to come up): then its own keys are
 * released, in reverse order (keys the user holds stay down).
 */
export function playCombo(backend, store, keys, { hold = 0, pause = null, timeout = 5000, setTimer = setTimeout, clearTimer = clearTimeout } = {}) {
  const [first, ...rest] = keys;
  const wait = pause ?? ((ms) => new Promise((r) => setTimer(r, ms)));
  return new Promise((resolve) => {
    let phase = "down";
    let stop = () => {};
    let timer = null;
    const arm = () => {
      timer = setTimer(() => {
        phase = "failed";
        stop();
        for (const k of [...keys].reverse()) backend.keyUp(k);
        resolve(false);
      }, timeout);
    };
    const release = () => {
      phase = "up";
      arm();
      for (const k of [...rest].reverse()) backend.keyUp(k);
    };
    const check = (down) => {
      const now = new Set(down ?? []);
      if (phase === "down" && keys.every((k) => now.has(k))) {
        clearTimer(timer);
        if (hold > 0) {
          // Held `hold` ms more (the 48SX's memory reset wants a second).
          phase = "hold";
          wait(hold).then(() => {
            if (phase !== "hold") return;
            release();
            check(store.state.keysDown);
          });
          return;
        }
        release();
      }
      if (phase === "up" && rest.every((k) => !now.has(k))) {
        phase = "done";
        backend.keyUp(first);
        stop();
        clearTimer(timer);
        resolve(true);
      }
    };
    arm();
    stop = store.watch(["keysDown"], (s) => check(s.keysDown));
    for (const k of keys) backend.keyDown(k);
  });
}

/**
 * Wait until the calculator has run `ms` of its own time (`stats`:
 * emulated time, and what it owes the clock while asleep), so a hold
 * lasts as long for the ROM at any speed and when the page lags; at
 * most ten times `ms` of wall time.
 */
export async function emulatedPause(backend, ms, { sleep = (t) => new Promise((r) => setTimeout(r, t)), now = () => performance.now() } = {}) {
  const at = async () => {
    const s = await backend.stats();
    return s.emulatedMs + (s.owedMs ?? 0);
  };
  const t0 = await at();
  const end = now() + 10 * ms;
  while ((await at()) - t0 < ms && now() < end) await sleep(50);
}

/**
 * Run combination `combo` (an `onCombos` entry): asked first where it
 * asks; refused with a message while the calculator is paused or
 * another combination is still being pressed. Resolves
 * whether it was played.
 */
let playing = false;

export async function runCombo(combo, backend, store, ask = confirmAction) {
  if (!store.state.booted) return false;
  if (playing) {
    store.set({ message: "An ON combination is still being pressed.", messageError: true });
    return false;
  }
  if (!store.state.running) {
    store.set({ message: "The calculator is paused. Run it first.", messageError: true });
    return false;
  }
  if (combo.ask && !(await ask(combo.ask))) return false;
  if (playing) return false;
  playing = true;
  let ok = false;
  try {
    ok = await playCombo(backend, store, combo.keys, { hold: combo.hold ?? 0, pause: (ms) => emulatedPause(backend, ms) });
  } finally {
    playing = false;
  }
  if (!ok) store.set({ message: `The calculator did not take ${chordOf(combo)}.`, messageError: true });
  return ok;
}
