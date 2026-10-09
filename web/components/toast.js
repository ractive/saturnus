// A toast: what came of an action, centred at the top of the stage, on
// the calculator's case above its display, wherever the panel is. One at
// a time; a newer one replaces it. An outcome goes after `TOAST_MS`; a
// step still in progress ("Downloading …") stays until the next message;
// an error (`role="alert"`) stays until it is clicked or closed with its
// ×. Its text goes in once it is in the page, as the
// note's does (web/components/note.js), or a screen reader may not
// announce it. `routeToasts` shows the store's `message`, and the memory
// view's `writeMessage` while the view is closed.

import { iconEl } from "./icons.js";

/** How long an outcome stays, in ms. */
export const TOAST_MS = 4000;

/** Narrow screens: the top bar, the stage without its toolbar (style.css). */
const PHONE = "(max-width: 759px)";

let shown = null;

/** Close the toast, if one is shown. */
export function closeToast() {
  shown?.close();
}

/** Whether `text` is a step still in progress: it ends in "…". */
const inProgress = (text) => /…$/.test(text);

/**
 * Put `el` over the stage, centred near its top: there it lies on the
 * case above the display. The calculator fills the stage's height on a
 * phone and on most desks, so at the bottom it would cover the last row
 * of keys. Clear of the toolbar's corners and, in fullscreen, of the
 * close and search buttons.
 */
function place(el) {
  const stage = document.getElementById("stage");
  const r = stage?.getBoundingClientRect() ?? { left: 0, top: 0, width: innerWidth };
  const width = r.width || innerWidth;
  const corners = matchMedia(PHONE).matches && !document.fullscreenElement && !stage?.classList.contains("fs") ? 16 : 2 * 64;
  el.style.left = `${(r.left ?? 0) + width / 2}px`;
  el.style.maxWidth = `${Math.max(200, Math.min(480, width - corners))}px`;
  el.style.top = `${Math.max(r.top, 0) + 8}px`;
}

/**
 * Show `text` as the toast, replacing the one shown; `error` keeps it
 * until it is clicked or closed. Returns the element.
 */
export function toast(text, { error = false } = {}) {
  closeToast();
  const el = document.createElement("div");
  el.className = `toast${error ? " error" : ""}`;
  el.setAttribute("role", error ? "alert" : "status");
  const p = document.createElement("p");
  el.append(p);
  if (error) {
    const x = document.createElement("button");
    x.type = "button";
    x.className = "icon toast-close";
    x.setAttribute("aria-label", "Close");
    x.title = "Close";
    x.append(iconEl("close"));
    el.append(x);
    el.addEventListener("click", () => close());
  }
  // Inside a fullscreen element, or it would not show.
  (document.fullscreenElement ?? document.body).append(el);
  place(el);
  const frame = requestAnimationFrame(() => {
    p.textContent = text;
  });
  const timer = error || inProgress(text) ? null : setTimeout(() => close(), TOAST_MS);
  const onResize = () => place(el);
  window.addEventListener("resize", onResize);
  function close() {
    if (shown?.el !== el) return;
    shown = null;
    cancelAnimationFrame(frame);
    clearTimeout(timer);
    window.removeEventListener("resize", onResize);
    el.remove();
  }
  shown = { el, text, error, close };
  return el;
}

/**
 * Show the store's messages as toasts: `message` (with `messageError`),
 * and the memory view's `writeMessage` while the view is closed (open,
 * its status row shows it). An emptied `message` ends a step in
 * progress; an error stays for its click.
 */
export function routeToasts(store) {
  const endStep = () => {
    if (shown && !shown.error && inProgress(shown.text)) closeToast();
  };
  store.watch(["message", "messageError"], (s) => {
    if (s.message) toast(s.message, { error: s.messageError });
    else endStep();
  });
  store.watch(["writeMessage"], (s) => {
    const m = s.writeMessage;
    if (s.layer) return;
    if (m?.text) toast(m.text, { error: Boolean(m.error) });
    else endStep();
  });
}
