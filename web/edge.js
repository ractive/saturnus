// Edge to edge (the fullscreen view): how much of the calculator's face to
// draw and how large. The keyboard and the display window (the core) take
// the screen's width; the rim, the bezel's sides and the lettering above the
// window are what gets cropped when the screen is too narrow or too short
// for the whole face.

/** Room for the overlay buttons along the top, in CSS pixels: 44 and 4 on each side. */
export const BUTTON_BAND = 52;

/**
 * The layout of `face` and `core` (`[x, y, w, h]` in skin units, the core
 * inside the face) in a room of `w` x `h` CSS pixels. `snap(f)` may shrink
 * the scale a little (whole device pixels per LCD pixel).
 *
 * The core fills the room's width, or its height where the room is wide.
 * Where the buttons' band would not fit beside the core, the core starts
 * below it. The face around the core shows as far as the room goes: what
 * does not fit is cropped evenly at the sides and from the top, so the
 * keys keep the bottom of the screen. A face that fits whole is centred,
 * and no higher than the band; beside the buttons it is cropped clear of
 * them.
 *
 * Returns `f` (CSS pixels per skin unit), `view` (the face's part drawn,
 * in skin units) and `x`, `y` (where the view's top left lands in the
 * room, in CSS pixels).
 */
export function edgeLayout(face, core, { w, h }, snap = (f) => f) {
  const [fx, fy, fw, fh] = face;
  const [cx, cy, cw, ch] = core;
  let f = Math.min(w / cw, h / ch);
  // The buttons sit beside the core where it leaves them room (a wide
  // screen), else above it.
  let band = 0;
  if ((w - cw * f) / 2 < BUTTON_BAND) {
    band = BUTTON_BAND;
    f = Math.min(w / cw, (h - band) / ch);
  }
  f = snap(f);
  // Across: the whole face, or as much as fits around the core's middle
  // (beside the buttons, where they are not above).
  const vw = Math.min(fw, (band ? w : w - 2 * BUTTON_BAND) / f);
  const vx = clamp(cx + cw / 2 - vw / 2, fx, fx + fw - vw);
  // Down: the whole face, or the core and as much of the face below and
  // then above it as fits.
  const vh = Math.min(fh, (h - band) / f);
  const vy = Math.max(fy, Math.min(cy, fy + fh - vh));
  const x = (w - vw * f) / 2;
  const y = Math.max(band, (h - vh * f) / 2);
  return { f, view: [vx, vy, vw, vh], x, y };
}

function clamp(v, lo, hi) {
  return Math.min(Math.max(v, lo), hi);
}
