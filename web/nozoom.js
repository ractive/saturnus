/**
 * No zooming in fullscreen: the calculator stays at the screen's scale, so
 * a pinch or a double tap beside the keys cannot enlarge and crop it.
 * Outside fullscreen the page zooms as usual (it must, for those who need
 * it larger).
 *
 * - The viewport meta gains `maximum-scale=1, user-scalable=no` while
 *   fullscreen: Chrome and Firefox on Android stop pinch and double-tap
 *   zoom and take a zoom made before back to scale 1; the meta's own
 *   content comes back on leaving. (iOS Safari ignores both for the
 *   user's zoom.)
 * - The stage's `touch-action: none` in fullscreen (style.css) keeps
 *   gestures that start on it, the keys or beside them, from zooming in
 *   the browsers that honour it.
 * - iOS Safari, which zooms regardless: its `gesturestart` and
 *   `gesturechange` cancelled, and a `touchmove` of two or more fingers.
 *   A touch's start is left alone, so a key's press, a drag across the
 *   keys and two keys held at once work as before.
 */

const LOCK = "maximum-scale=1, user-scalable=no";

/** The meta's content without a scale limit of its own (as index.html has it). */
let original = null;

const cancel = (e) => {
  if (e.cancelable) e.preventDefault();
};
const cancelPinch = (e) => {
  if (e.touches.length > 1 && e.cancelable) e.preventDefault();
};

/** Locks (`on`) or frees the page's zoom. Idempotent. */
export function lockZoom(on, doc = document) {
  const meta = doc.querySelector('meta[name="viewport"]');
  if (meta) {
    if (on && original === null) {
      original = meta.getAttribute("content") ?? "";
      meta.setAttribute("content", original ? `${original}, ${LOCK}` : LOCK);
    } else if (!on && original !== null) {
      meta.setAttribute("content", original);
      original = null;
    }
  }
  const method = on ? "addEventListener" : "removeEventListener";
  for (const type of ["gesturestart", "gesturechange"]) doc[method](type, cancel, { passive: false });
  doc[method]("touchmove", cancelPinch, { passive: false });
}
