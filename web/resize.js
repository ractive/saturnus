// A handle that resizes something by its width: drag it, move it with
// the arrow keys (16 px a step) while it has the focus, double-click it
// for the default width. The page's edges (app.js) and the memory view's
// tree (sat-explorer.js) use it, so they all behave the same.

/**
 * `width()` is the width now; `set(px, save)` sets one (clamped by the
 * caller; `save` false while a drag is under way, kept once it ends),
 * `set(null)` the default again. `grows` is 1 when dragging right
 * widens it, -1 when dragging left does.
 */
export function dragResize(handle, { width, set, grows = 1 }) {
  let start = null;
  handle.addEventListener("pointerdown", (ev) => {
    if (ev.button !== 0) return;
    ev.preventDefault();
    handle.setPointerCapture(ev.pointerId);
    start = { x: ev.clientX, w: width() };
    handle.classList.add("dragging");
    document.body.classList.add("resizing");
  });
  handle.addEventListener("pointermove", (ev) => {
    if (start) set(start.w + grows * (ev.clientX - start.x), false);
  });
  const end = () => {
    if (!start) return;
    start = null;
    handle.classList.remove("dragging");
    document.body.classList.remove("resizing");
    set(width());
  };
  handle.addEventListener("pointerup", end);
  handle.addEventListener("pointercancel", end);
  handle.addEventListener("dblclick", () => set(null));
  handle.addEventListener("keydown", (ev) => {
    const step = { ArrowLeft: -16, ArrowRight: 16 }[ev.key];
    if (step === undefined || ev.altKey || ev.ctrlKey || ev.metaKey) return;
    ev.preventDefault();
    set(width() + grows * step);
  });
}
