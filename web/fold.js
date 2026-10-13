// The clamshell's two arrangements (a skin with `fold`: the 28S, which
// opens like a book). The skin is one drawing of both halves side by side,
// the letter half on the left, the display half on the right and the
// hinge between them; on a stage taller than wide the halves are stacked,
// the display half on top, the hinge's barrel across, the letter half's
// keyboard below (its reference card left off). Each half moves as a
// whole. Pure (no DOM): web/test/fold.test.mjs.

/** Room between a stacked half and the barrel, in skin units. */
export const STACK_GAP = 14;

/** Whether a stage of `room` ({w, h}, CSS pixels) stacks the halves: taller than wide. */
export function stacks(room) {
  return room.h > room.w;
}

/** Which half of `fold` the point (x, y) is in: "letters" or "display". */
export function halfAt(fold, x, y) {
  const [lx, , lw] = fold.letters;
  return x < lx + lw ? "letters" : "display";
}

/** Whether `[x, y, w, h]` of the letter half lies wholly above its stacked view (the reference card). */
export function aboveStack(fold, [x, y, w, h]) {
  return halfAt(fold, x, y) === "letters" && y + h <= fold.stackedLetters[1];
}

/**
 * The drawing of skin `s` side by side (`stacked` false) or stacked:
 * `{stacked, width, height, letters, display, lcd, glass, logo, barrel,
 * bodies}`. `letters` and `display` are the [dx, dy] each half moves by;
 * `lcd`, `glass` (null without one) and `logo` are where they land;
 * `barrel` is the hinge's [x, y, w, h] (upright side by side, across when
 * stacked); `bodies` each half's case as shown, in its own coordinates
 * (stacked, cut to the part shown: `stackedLetters`, `stackedDisplay`).
 * A skin without `fold` is drawn as it is.
 */
export function foldGeometry(s, stacked) {
  const f = s.fold;
  if (!f || !stacked) {
    return {
      stacked: false,
      width: s.width,
      height: s.height,
      letters: [0, 0],
      display: [0, 0],
      lcd: s.lcd,
      glass: s.glass ?? null,
      logo: s.logo,
      barrel: f?.hinge ?? null,
      bodies: f ? { letters: f.letters, display: f.display } : null,
    };
  }
  const [dx, dy, dw, dh] = f.stackedDisplay;
  const [lx, ly, lw, lh] = f.stackedLetters;
  const thick = f.hinge[2];
  const width = Math.max(dw, lw);
  const display = [(width - dw) / 2 - dx, -dy];
  const barrelY = dh + STACK_GAP;
  const lettersTop = barrelY + thick + STACK_GAP;
  const letters = [(width - lw) / 2 - lx, lettersTop - ly];
  const length = Math.round(width * 0.82);
  const moved = ([x, y, w, h], [ox, oy]) => [x + ox, y + oy, w, h];
  return {
    stacked: true,
    width,
    height: lettersTop + lh,
    letters,
    display,
    lcd: moved(s.lcd, display),
    glass: s.glass ? moved(s.glass, display) : null,
    logo: moved(s.logo, display),
    barrel: [(width - length) / 2, barrelY, length, thick],
    bodies: { letters: f.stackedLetters, display: f.stackedDisplay },
  };
}

/**
 * The scale, CSS pixels per skin unit, that fits geometry `g` into `room`.
 * Side by side the whole drawing fits; stacked it fills the width and the
 * page scrolls to the letter half, but never so large that the display
 * half alone overflows the stage's height.
 */
export function foldScale(s, g, room) {
  if (!g.stacked) return Math.min(room.w / g.width, room.h / g.height);
  return Math.min(room.w / g.width, room.h / s.fold.stackedDisplay[3]);
}
