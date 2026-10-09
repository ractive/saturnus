// A control that is off says why in its tooltip and gets its own tooltip
// back when it is on again. Pure (an element-like object will do), so
// `web/test/disable.test.mjs` runs it under Node.

/** Turn `button` on or off; off, its tooltip says `why`. */
export function setOff(button, off, why) {
  if (!("title" in button.dataset)) button.dataset.title = button.title;
  const title = off ? why : button.dataset.title;
  // Called on every frame: only a change touches the element.
  if (button.disabled !== off) button.disabled = off;
  if (button.title !== title) button.title = title;
}
