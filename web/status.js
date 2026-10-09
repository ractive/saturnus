// The status line's message (the store's `message`, `messageError`):
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
 * Clear the store's message as above. `on(type, fn)` listens to the
 * page's actions (a capturing `pointerdown` and `keydown`); `now()` is
 * the clock; the timers are injected for the tests.
 */
export function watchMessages(store, {
  ms = MESSAGE_MS,
  grace = ERROR_GRACE_MS,
  now = () => performance.now(),
  setTimer = (f, t) => setTimeout(f, t),
  clearTimer = (id) => clearTimeout(id),
  on = (type, fn) => document.addEventListener(type, fn, true),
} = {}) {
  let timer = null;
  let shownAt = 0;
  const clear = () => store.set({ message: "", messageError: false });
  store.watch(["message", "messageError"], (s) => {
    if (timer !== null) clearTimer(timer);
    timer = null;
    shownAt = now();
    if (!clearsItself(s.message, s.messageError)) return;
    const text = s.message;
    timer = setTimer(() => {
      timer = null;
      if (store.state.message === text) clear();
    }, ms);
  });
  const action = () => {
    const s = store.state;
    if (s.message && s.messageError && now() - shownAt >= grace) clear();
  };
  on("pointerdown", action);
  on("keydown", action);
}
