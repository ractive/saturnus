// A short note under a control: why a button that is off does nothing,
// for touch, which shows no tooltip. One at a time, `role="status"` so a
// screen reader reads it; it goes on the next click, tap or key, or after
// `NOTE_MS`. Placed as a menu is (`placeMenu`).

import { placeMenu } from "./menu.js";

/** How long a note stays, in ms. */
export const NOTE_MS = 4000;

let shown = null;

/** Close the note, if one is shown. */
export function closeNote() {
  shown?.();
}

/** Show `text` under `anchor` (an element), its right edge on the anchor's. */
export function showNote(anchor, text) {
  closeNote();
  const note = document.createElement("p");
  note.className = "note";
  note.setAttribute("role", "status");
  note.textContent = text;
  // Inside a fullscreen element, or it would not show.
  const host = document.fullscreenElement?.contains(anchor) ? document.fullscreenElement : document.body;
  host.append(note);
  const { x, y } = placeMenu(anchor.getBoundingClientRect(), note.getBoundingClientRect(), { width: innerWidth, height: innerHeight });
  note.style.left = `${x}px`;
  note.style.top = `${y}px`;
  // Not the press that opened it: listening starts after it.
  const timer = setTimeout(close, NOTE_MS);
  const later = setTimeout(() => {
    document.addEventListener("pointerdown", close, true);
    document.addEventListener("keydown", close, true);
  }, 0);
  function close() {
    if (shown !== close) return;
    shown = null;
    clearTimeout(timer);
    clearTimeout(later);
    document.removeEventListener("pointerdown", close, true);
    document.removeEventListener("keydown", close, true);
    note.remove();
  }
  shown = close;
  return note;
}
