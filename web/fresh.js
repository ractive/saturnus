// The page's questions (iteration 27, "Start fresh"; Forget ROMs): asked
// in the page's own card (the update notice's), never with
// window.confirm. Each resolves to the answer.

/**
 * Ask `question` with Cancel and the action `yes` (its button's label);
 * resolves to true for the action, false for Cancel or Escape. One
 * question at a time: asking again answers the open one with Cancel.
 */
export function confirmCard(question, yes, doc = document) {
  doc.querySelector(".fresh-notice")?.dispatchEvent(new Event("cancel"));
  return new Promise((resolve) => {
    const box = doc.createElement("div");
    box.className = "fresh-notice";
    box.setAttribute("role", "alertdialog");
    box.setAttribute("aria-labelledby", "fresh-text");
    const text = doc.createElement("p");
    text.id = "fresh-text";
    text.textContent = question;
    const row = doc.createElement("div");
    row.className = "notice-actions";
    const done = (answer) => {
      box.remove();
      doc.removeEventListener("keydown", onKey, true);
      resolve(answer);
    };
    const onKey = (e) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      done(false);
    };
    const button = (label, answer, primary = false) => {
      const b = doc.createElement("button");
      b.type = "button";
      b.textContent = label;
      if (primary) b.className = "primary";
      b.addEventListener("click", () => done(answer));
      return b;
    };
    const cancel = button("Cancel", false);
    row.append(cancel, button(yes, true, true));
    box.append(text, row);
    box.addEventListener("cancel", () => done(false));
    doc.addEventListener("keydown", onKey, true);
    doc.body.append(box);
    cancel.focus();
  });
}

/**
 * Ask whether to cold-boot `title` (the model's title) with an empty
 * memory; resolves to true for "Start fresh".
 */
export function confirmFresh(title, doc = document) {
  return confirmCard(`Start the ${title} fresh? Its stack, variables and settings go, as on a new calculator. Your saved state stays.`, "Start fresh", doc);
}

/**
 * Ask before Forget ROMs: the browser deletes the ROMs and the saved
 * 49G state (it holds the 49G's flash, the ROM); the app only forgets
 * where the files are (`app`).
 */
export function confirmForget(app, doc = document) {
  const question = app
    ? "Forget where the ROMs are? The files and the saved states stay."
    : "Forget all kept ROMs? The saved 49G state goes too, as it contains the ROM. Other saved states stay.";
  return confirmCard(question, "Forget ROMs", doc);
}
