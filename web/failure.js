// What the page says when it cannot start at all. Pure, so
// `web/test/failure.test.mjs` runs it under Node.

/** The start failure `err` as a sentence with what to do, for the browser or (`app`) the desktop app. */
export function startFailure(err, app) {
  const why = String(err?.message ?? err).replace(/[.\s]+$/, "");
  const next = app ? "Restart the app." : "Reload the page; if it keeps failing, your browser may be too old for WebAssembly.";
  return `saturnus could not start: ${why}. ${next}`;
}
