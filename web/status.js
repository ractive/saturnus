// The status line's message (the store's `message`, `messageError`), and
// the memory view's (`writeMessage`):
// an outcome ("State saved.") goes after `MESSAGE_MS`; a step still in
// progress ("Downloading …") stays until the next message; an error
// stays until the user next clicks, taps or presses a key, so it is not
// missed. Pure but for the timers and listeners it is given:
// web/test/status.test.mjs.

/** How long an outcome stays, in ms. */
export const MESSAGE_MS = 6000;
/** An error is not cleared by the very action that caused it: this long first, in ms. */
export const ERROR_GRACE_MS = 1000;

/** Whether `message` clears by itself: an outcome, not an error or a step in progress. */
export function clearsItself(message, error) {
  return Boolean(message) && !error && !/…$/.test(message);
}

/**
 * Clear a message in the store as above: `keys` are its store keys,
 * `read(state)` gives `[text, error]` and `clear()` empties it. `on(type,
 * fn)` listens to the page's actions (a capturing `pointerdown` and
 * `keydown`); `now()` is the clock; the timers are injected for the tests.
 */
export function watchCleared(store, keys, read, clear, {
  ms = MESSAGE_MS,
  grace = ERROR_GRACE_MS,
  now = () => performance.now(),
  setTimer = (f, t) => setTimeout(f, t),
  clearTimer = (id) => clearTimeout(id),
  on = (type, fn) => document.addEventListener(type, fn, true),
} = {}) {
  let timer = null;
  let shownAt = 0;
  store.watch(keys, (s) => {
    if (timer !== null) clearTimer(timer);
    timer = null;
    shownAt = now();
    const [text, error] = read(s);
    if (!clearsItself(text, error)) return;
    timer = setTimer(() => {
      timer = null;
      if (read(store.state)[0] === text) clear();
    }, ms);
  });
  const action = () => {
    const [text, error] = read(store.state);
    if (text && error && now() - shownAt >= grace) clear();
  };
  on("pointerdown", action);
  on("keydown", action);
}

/** The status line's message (`message`, `messageError`). */
export function watchMessages(store, options = {}) {
  watchCleared(store, ["message", "messageError"], (s) => [s.message, s.messageError],
    () => store.set({ message: "", messageError: false }), options);
}

/**
 * The memory view's message (`writeMessage`, `{text, error}`), shown in
 * its status row: the same rules, so a write's outcome gives way to the
 * tab's hint again.
 */
export function watchWriteMessages(store, options = {}) {
  watchCleared(store, ["writeMessage"], (s) => [s.writeMessage?.text ?? "", Boolean(s.writeMessage?.error)],
    () => store.set({ writeMessage: null }), options);
}
