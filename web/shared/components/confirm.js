// The page's one question before something that cannot be undone: a
// modal <dialog> with a short title, one sentence of what happens, Cancel
// (the default) and a button naming the action. Escape and a click on the
// backdrop cancel; the focus goes back to what had it; while it is open
// the calculator takes no keys (a modal dialog keeps them,
// sat-calculator.js). On a narrow screen it is a sheet at the bottom
// (style.css). Used for Purge, Replace, Remove ROM(s) and Start fresh;
// fields (Rename, New directory) stay in place.

let serial = 0;

/**
 * Ask; resolves to true for the action, false for Cancel, Escape or a
 * click outside. `title` is the question ("Purge test?"), `body` the
 * sentence of what follows, `action` the button's word ("Purge");
 * `danger` paints it in the error colour. One at a time: asking again
 * answers the open one with Cancel.
 */
export function confirmAction({ title, body, action, danger = true, doc = document }) {
  doc.querySelector("dialog.confirm")?.dispatchEvent(new Event("cancel"));
  const opener = doc.activeElement;
  const id = `confirm-${++serial}`;
  return new Promise((resolve) => {
    const dialog = doc.createElement("dialog");
    dialog.className = "confirm";
    dialog.setAttribute("aria-labelledby", `${id}-title`);
    dialog.setAttribute("aria-describedby", `${id}-body`);
    const h = doc.createElement("h2");
    h.id = `${id}-title`;
    h.textContent = title;
    const p = doc.createElement("p");
    p.id = `${id}-body`;
    p.textContent = body;
    const row = doc.createElement("div");
    row.className = "confirm-actions";
    const button = (label, answer, cls) => {
      const b = doc.createElement("button");
      b.type = "button";
      b.textContent = label;
      if (cls) b.className = cls;
      b.addEventListener("click", () => done(answer));
      return b;
    };
    const cancel = button("Cancel", false);
    row.append(cancel, button(action, true, danger ? "confirm-danger" : "primary"));
    // The box inside: a click on the dialog itself is on its backdrop.
    const box = doc.createElement("div");
    box.className = "confirm-box";
    box.append(h, p, row);
    dialog.append(box);
    let answered = false;
    const done = (answer) => {
      if (answered) return;
      answered = true;
      if (dialog.open) dialog.close();
      dialog.remove();
      // Back to what had the focus, if it is still on the page.
      if (opener && opener !== doc.body && opener.isConnected && typeof opener.focus === "function") opener.focus();
      resolve(answer);
    };
    // Escape (the dialog's own cancel) and a click on the backdrop.
    dialog.addEventListener("cancel", (e) => {
      e.preventDefault();
      done(false);
    });
    dialog.addEventListener("click", (e) => {
      if (e.target === dialog) done(false);
    });
    doc.body.append(dialog);
    dialog.showModal();
    cancel.focus();
  });
}
