// Stepping between the model dialogs: ‹ and › open the neighbour by an
// invoker command (show-modal, no script needed); this closes the dialog
// they sit in first, so the dialogs do not pile up, and maps ← and → to
// them. Stores nothing. Without it the buttons still work, the dialogs
// then stack (Esc goes back one).

document.addEventListener("command", (e) => {
  if (e.command !== "show-modal") return;
  const from = e.source?.closest("dialog.model-dlg");
  if (from && from !== e.target) from.close();
}, true);

document.addEventListener("keydown", (e) => {
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
  if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
  const open = [...document.querySelectorAll("dialog.model-dlg[open]")].pop();
  if (!open) return;
  e.preventDefault();
  open.querySelector(e.key === "ArrowLeft" ? ".dlg-prev" : ".dlg-next")?.click();
});
