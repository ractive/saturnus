// Setting the calculator's clock to this computer's (protocol.md,
// `setClock`): the controls' "Set the clock" button and the "Set the
// clock on every start" preference. The host types or presses what the
// calculator's owner would (→TIME and →DATE on the 48 and 49, STO▶ TIME
// and DATE in HOME on the 38G, 39G and 40G, the TIME application's SET
// menu on the 27S, 17BII and 17B); the 42S has no clock.

/** The models with a clock: all but the 42S. */
export const CLOCK_MODELS = new Set(["48sx", "48gx", "49g", "38g", "39g", "40g", "27s", "17bii", "17b"]);

/** `setClock`'s fields for the local time `date` (a Date). */
export function clockFields(date) {
  return {
    year: date.getFullYear(),
    month: date.getMonth() + 1,
    day: date.getDate(),
    hour: date.getHours(),
    minute: date.getMinutes(),
    second: date.getSeconds(),
  };
}

/**
 * Why the clock cannot be set now (the store's state), or null: no
 * calculator, a model without a clock, a send or a change running, a
 * command line open. The host checks the rest (off, busy, the 17B's
 * language prompt) and says why in its own words.
 */
export function clockRefusal(s) {
  if (!s.booted || s.booted !== s.model) return "Start the calculator first.";
  if (!CLOCK_MODELS.has(s.booted)) return `The ${s.booted.toUpperCase()} has no clock.`;
  if (s.writing || s.busy) return "The calculator is busy. Try again when it has finished.";
  if (s.cmdlineOpen) return "Finish what you're typing first.";
  return null;
}

/** What the toast says once the clock is set to `date`. */
export function clockMessage(date, locale = undefined) {
  const when = date.toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" });
  return `Clock set to ${when}.`;
}

/** Set the clock to `now` (a Date): resolves when the host has taken it. */
export function setClock(backend, now = new Date()) {
  return backend.setClock(clockFields(now));
}

/** How long the calculator must have slept before a start sets its clock, in ms. */
export const START_QUIET_MS = 1500;
/** How long a start waits for that at most, in ms. */
export const START_CAP_MS = 30_000;

/**
 * With the preference on, set the clock after every start (a boot, a
 * state loaded: the store's `starts` counts them) once the calculator is
 * idle: asleep (`loop` "sleep") for `START_QUIET_MS`, nothing typing or
 * writing. A refusal while it still starts up (busy) is tried again
 * until `START_CAP_MS`; any other refusal ends it quietly.
 */
export function watchClockOnStart(backend, store, prefs, { quietMs = START_QUIET_MS, capMs = START_CAP_MS, now = () => new Date() } = {}) {
  let run = 0;
  const start = () => {
    const mine = ++run;
    const s = store.state;
    if (prefs.get("clockOnStart") !== "on" || !CLOCK_MODELS.has(s.booted)) return;
    const booted = s.booted;
    const began = Date.now();
    let quietSince = null;
    const tick = async () => {
      if (mine !== run || store.state.booted !== booted || Date.now() - began > capMs) return;
      const st = store.state;
      const idle = st.loop === "sleep" && !st.busy && !st.writing && st.frame;
      if (!idle) quietSince = null;
      else quietSince ??= Date.now();
      if (quietSince === null || Date.now() - quietSince < quietMs || clockRefusal(st)) {
        setTimeout(tick, 250);
        return;
      }
      try {
        await setClock(backend, now());
      } catch (err) {
        // Still starting (the boot's question): again in a moment.
        if (/busy/i.test(String(err?.message ?? err))) {
          quietSince = null;
          setTimeout(tick, 1000);
        }
      }
    };
    setTimeout(tick, 250);
  };
  store.watch(["booted"], (s) => {
    if (s.booted) start();
  });
  store.watch(["starts"], start);
}
