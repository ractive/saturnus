// The ON-key combinations (iteration 65): ON held down while another
// key (or two) is pressed. The ON key's menu on the skin (a long press or
// a right-click, sat-calculator.js `onMenu`) and the command palette
// (app.js) offer them per model, from the table alone (the chords differ
// by model); only the 48GX so far, the other models follow from a sweep
// of each. A combination is
// played as held keys (`keyDown` and `keyUp`, web/protocol.md): ON first,
// the others while it is down, released once the machine has had them
// all down. The rest is pure: web/test/onkeys.test.mjs.

import { confirmAction } from "./shared/components/confirm.js";

/**
 * Per model: `{id, title, keys, description, ask?}`. `keys` are held in
 * order (ON first); `ask` is the question asked before (`confirmAction`).
 */
export const ON_COMBOS = {
  // wiki: hardware/hp48gx-keyboard ("ON-key combinations": FAQ 4.6, and
  // what ROM R does on saturnus)
  "48gx": [
    { id: "restart", title: "Restart", keys: ["on", "c"], description: "ON+C: clears the stack and keeps the variables." },
    { id: "self-test", title: "Self-test", keys: ["on", "d"], description: "ON+D: the calculator's interactive self-test." },
    { id: "print-display", title: "Print display", keys: ["on", "1"], description: "ON+1: sends the display to the printer." },
    {
      id: "reset-memory", title: "Reset memory…", keys: ["on", "a", "f"], description: "ON+A+F: the calculator asks \"Try To Recover Memory?\"; NO clears it. Asks first.",
      ask: { title: "Reset the calculator's memory?", body: "The calculator asks \"Try To Recover Memory?\" next: NO (F) clears its variables and stack, YES (A) tries to keep them. Saved states are not touched.", action: "Reset memory" },
    },
  ],
};

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
 * `keysDown`: each key down in order, held until the machine has them all
 * down, then the others released before the first (ON). Resolves true,
 * or false when the machine has not taken them within `timeout` ms:
 * then its own keys are released, in reverse order (keys the user holds
 * stay down).
 */
export function playCombo(backend, store, keys, { timeout = 5000, setTimer = setTimeout, clearTimer = clearTimeout } = {}) {
  const [first, ...rest] = keys;
  return new Promise((resolve) => {
    let phase = "down";
    let stop = () => {};
    const timer = setTimer(() => {
      stop();
      for (const k of [...keys].reverse()) backend.keyUp(k);
      resolve(false);
    }, timeout);
    const check = (down) => {
      const now = new Set(down ?? []);
      if (phase === "down" && keys.every((k) => now.has(k))) {
        phase = "up";
        for (const k of [...rest].reverse()) backend.keyUp(k);
      }
      if (phase === "up" && rest.every((k) => !now.has(k))) {
        phase = "done";
        backend.keyUp(first);
        stop();
        clearTimer(timer);
        resolve(true);
      }
    };
    stop = store.watch(["keysDown"], (s) => check(s.keysDown));
    for (const k of keys) backend.keyDown(k);
  });
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
    ok = await playCombo(backend, store, combo.keys);
  } finally {
    playing = false;
  }
  if (!ok) store.set({ message: `The calculator did not take ${combo.keys.map((k) => k.toUpperCase()).join("+")}.`, messageError: true });
  return ok;
}
