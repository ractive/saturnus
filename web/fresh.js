// "Start fresh" (iteration 27): the calculator keeps its state across
// reloads, so starting over is asked for, in the page's own card (the
// update notice's), never with window.confirm. Resolves to the answer.

/**
 * Ask whether to cold-boot `title` (the model's title) with an empty
 * memory; resolves to true for "Start fresh", false for Cancel or Escape.
 * One question at a time: asking again answers the open one with Cancel.
 */
export function confirmFresh(title, doc = document) {
  doc.querySelector(".fresh-notice")?.dispatchEvent(new Event("cancel"));
  return new Promise((resolve) => {
    const box = doc.createElement("div");
    box.className = "fresh-notice";
    box.setAttribute("role", "alertdialog");
    box.setAttribute("aria-labelledby", "fresh-text");
    const text = doc.createElement("p");
    text.id = "fresh-text";
    text.textContent = `Start the ${title} fresh? Its stack, variables and settings go, as on a new calculator. Your saved state stays.`;
    const row = doc.createElement("div");
    row.className = "notice-actions";
    const done = (yes) => {
      box.remove();
      doc.removeEventListener("keydown", onKey, true);
      resolve(yes);
    };
    const onKey = (e) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      done(false);
    };
    const button = (label, yes, primary = false) => {
      const b = doc.createElement("button");
      b.type = "button";
      b.textContent = label;
      if (primary) b.className = "primary";
      b.addEventListener("click", () => done(yes));
      return b;
    };
    const cancel = button("Cancel", false);
    row.append(cancel, button("Start fresh", true, true));
    box.append(text, row);
    box.addEventListener("cancel", () => done(false));
    doc.addEventListener("keydown", onKey, true);
    doc.body.append(box);
    cancel.focus();
  });
}
