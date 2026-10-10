// A toast: what came of an action, centred at the top of the stage, on
// the calculator's case above its display, wherever the panel is. One at
// a time; a newer one replaces it. An outcome goes after `TOAST_MS`; a
// step still in progress ("Downloading …") stays until the next message;
// an error (`role="alert"`) stays until it is closed with its × or
// Escape. Its text goes in once it is in the page, as the note's does
// (web/components/note.js), or a screen reader may not announce it.
// `routeToasts` shows the store's `message`, and the memory view's
// `writeMessage` while the view is closed.

import { iconEl } from "./icons.js";

/** How long an outcome stays, in ms. */
export const TOAST_MS = 4000;

/** Narrow screens: the top bar, the stage without its toolbar (style.css). */
const PHONE = "(max-width: 759px)";

let shown = null;

/** Close the toast, if one is shown. */
export function closeToast() {
  shown?.close(false);
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
 * Put `el` in the page: inside a fullscreen element, or it would not
 * show, else the page; right after the region that has the focus (its
 * child of that host), so the next Tab reaches an error's × without
 * the focus being taken from the calculator. First when nothing has it.
 */
function insert(el) {
  const host = document.fullscreenElement ?? document.body;
  let region = document.activeElement;
  while (region && region.parentElement !== host) region = region.parentElement;
  if (region === el) return;
  if (region) region.after(el);
  else host.prepend(el);
}

/**
 * Show `text` as the toast, replacing the one shown. `error` keeps it
 * until it is closed with its × or Escape (a click on its text selects
 * it, for copying); the next Tab reaches the ×, which gives the focus
 * back as it closes. `source` names the store key it shows (for
 * `routeToasts`); `onClose(byUser)` hears that it went. Returns the
 * element.
 */
export function toast(text, { error = false, source = null, onClose = null } = {}) {
  closeToast();
  const el = document.createElement("div");
  el.className = `toast${error ? " error" : ""}`;
  el.setAttribute("role", error ? "alert" : "status");
  const p = document.createElement("p");
  el.append(p);
  let x = null;
  if (error) {
    x = document.createElement("button");
    x.type = "button";
    x.className = "icon toast-close";
    x.setAttribute("aria-label", "Close");
    x.title = "Close (Esc)";
    x.append(iconEl("close"));
    x.addEventListener("click", () => close(true));
    el.append(x);
  }
  // Moved into or out of a fullscreen element as that changes.
  const host = () => {
    if (el.parentNode !== (document.fullscreenElement ?? document.body)) insert(el);
    place(el);
  };
  insert(el);
  place(el);
  const frame = requestAnimationFrame(() => {
    p.textContent = text;
  });
  const before = document.activeElement;
  const timer = error || inProgress(text) ? null : setTimeout(() => close(false), TOAST_MS);
  // Placed again when the stage changes size (the panel hidden or shown, a resize).
  const onResize = () => place(el);
  const stage = document.getElementById("stage");
  const observer = typeof ResizeObserver === "function" && stage ? new ResizeObserver(onResize) : null;
  observer?.observe(stage);
  window.addEventListener("resize", onResize);
  document.addEventListener("fullscreenchange", host);
  // Escape closes an error: from its × (one Tab away), or when nothing
  // else took it (the memory view's Escape, a binding to ON go first).
  const onKey = (e) => {
    if (e.key !== "Escape" || (e.defaultPrevented && !el.contains(e.target))) return;
    e.preventDefault();
    close(true);
  };
  if (error) document.addEventListener("keydown", onKey);
  function close(byUser) {
    if (shown?.el !== el) return;
    shown = null;
    cancelAnimationFrame(frame);
    clearTimeout(timer);
    observer?.disconnect();
    window.removeEventListener("resize", onResize);
    document.removeEventListener("fullscreenchange", host);
    document.removeEventListener("keydown", onKey);
    const hadFocus = x && el.contains(document.activeElement);
    el.remove();
    if (hadFocus) {
      if (before?.isConnected && before !== document.body) before.focus({ preventScroll: true });
      else document.activeElement?.blur?.();
    }
    onClose?.(byUser);
  }
  shown = { el, text, error, source, close };
  return el;
}

/**
 * Say `text` through the store's `message` (an error with `error`):
 * emptied first, so the same text twice shows twice.
 */
export function say(store, text, error = false) {
  store.set({ message: "", messageError: false });
  store.set({ message: text, messageError: error });
}

/**
 * Show the store's messages as toasts: `message` (with `messageError`),
 * and the memory view's `writeMessage` while the view is closed (open,
 * its status row shows it). Emptied, a key ends its own step in
 * progress; an error stays for its × or Escape, and closing it empties
 * the key, so the same error can show again.
 */
export function routeToasts(store) {
  const endStep = (source) => {
    if (shown && shown.source === source && !shown.error && inProgress(shown.text)) closeToast();
  };
  store.watch(["message", "messageError"], (s) => {
    if (!s.message) {
      endStep("message");
      return;
    }
    const text = s.message;
    toast(text, {
      error: s.messageError,
      source: "message",
      onClose: (byUser) => {
        if (byUser && store.state.message === text) store.set({ message: "", messageError: false });
      },
    });
  });
  store.watch(["writeMessage"], (s) => {
    const m = s.writeMessage;
    if (s.layer) return;
    if (!m?.text) {
      endStep("write");
      return;
    }
    toast(m.text, {
      error: Boolean(m.error),
      source: "write",
      onClose: (byUser) => {
        if (byUser && store.state.writeMessage === m) store.set({ writeMessage: null });
      },
    });
  });
}
