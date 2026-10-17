// The calculator view (iteration 50c): its root element, which carries the
// layout's classes (style.css, `#calculator-view`), and whether the view is
// shown. Set once by mountCalculator (app.js). The document- and
// window-level handlers of the view (keys, paste, drops, the palette's and
// the tour's keys, resize) ask `shown()` first: a hidden view is not
// unmounted and its calculator keeps running, but it takes no input.

let root = null;
let isShown = () => true;

/** The view's root element and its `shown()` (mountCalculator). */
export function setView(el, shown = () => true) {
  root = el;
  isShown = shown;
}

/** The view's root element (`#calculator-view`), or null before it is mounted. */
export function viewRoot() {
  return root;
}

/** Whether the view is shown; true before it is mounted (a component alone, as in a test). */
export function shown() {
  return isShown();
}
