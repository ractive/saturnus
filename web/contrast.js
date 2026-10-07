// The LCD contrast: how dark the page draws the pixels for the contrast
// register, and the ON + / ON - sequence that steps it (the palette's
// "Darker display" and "Lighter display", the side panel's buttons), for
// keyboards and devices that cannot hold ON and press + or -.

/** Darkness at the low end of the model's ON+/ON- range: faint, not gone. */
export const LIGHTEST = 0.15;
/** Darkness at the contrast the ROM sets at power-on. */
export const AT_DEFAULT = 0.9;

/**
 * Pixel darkness, LIGHTEST to 1, for `contrast` on a model whose ON+/ON-
 * range is `[lo, hi]` and whose ROM powers on at `def` (the frame's
 * `contrastDefault`; the middle of the range if the host sends none). A
 * real LCD stays dark over most of the range and fades near its low end:
 * below the default the darkness follows a square root (steepest at `lo`),
 * above it rises on to 1 at `hi`.
 */
export function contrastDarkness(contrast, [lo, hi], def) {
  const d = Math.min(Math.max(def ?? Math.round((lo + hi) / 2), lo + 1), hi - 1);
  if (contrast <= d) {
    const x = Math.max(0, (contrast - lo) / (d - lo));
    return LIGHTEST + (AT_DEFAULT - LIGHTEST) * Math.sqrt(x);
  }
  return AT_DEFAULT + (1 - AT_DEFAULT) * Math.min(1, (contrast - d) / (hi - d));
}

/**
 * How far the unlit pixels darken at darkness `d`: a trace below the
 * default, then up to a grey background at the top of the range (as the
 * real LCD does), so ON+ above the default still shows.
 */
export function offTint(d) {
  return 0.06 * d * d + 0.25 * Math.max(0, d - AT_DEFAULT) / (1 - AT_DEFAULT);
}

/** Wall time to wait for the machine to report a key, in ms. */
const KEY_WAIT_MS = 1000;

/** Resolve once `key` is down (or up) in `store`'s `keysDown`, or after `ms`. */
function keyIs(store, key, down, ms) {
  return new Promise((resolve) => {
    const done = () => {
      clearTimeout(timer);
      store.removeEventListener("change", look);
      resolve();
    };
    const look = () => {
      if (store.state.keysDown.includes(key) === down) done();
    };
    const timer = setTimeout(done, ms);
    store.addEventListener("change", look);
    look();
  });
}

/**
 * One step of the contrast, darker or lighter: press ON, tap + (or -),
 * release ON, through the key commands as a held ON key would. ON goes up
 * only after the machine let go of + (the queue releases a key once it was
 * down long enough), so the ROM sees the chord.
 */
export async function stepContrast(backend, store, darker) {
  const key = darker ? "plus" : "minus";
  backend.keyDown("on");
  backend.keyDown(key);
  await keyIs(store, key, true, KEY_WAIT_MS);
  backend.keyUp(key);
  await keyIs(store, key, false, KEY_WAIT_MS);
  backend.keyUp("on");
}
