// Stepping between the model dialogs: ‹ and › open the neighbour by an
// invoker command (show-modal, no script needed). This swaps the two in
// one frame instead, the next opened and the last closed without the
// fade, so the backdrop does not flicker, and maps ← and → to them.
// Stores nothing. Without it the buttons still work; the dialogs then
// stack (Esc goes back one).

document.addEventListener("command", (e) => {
  if (e.command !== "show-modal" || !e.target.matches("dialog.model-dlg")) return;
  const from = e.source?.closest("dialog.model-dlg");
  if (!from || from === e.target) {
    // Opened from the page: with its fade.
    e.target.classList.remove("instant");
    return;
  }
  e.preventDefault();
  from.classList.add("instant");
  e.target.classList.add("instant");
  e.target.showModal();
  from.close();
}, true);

document.addEventListener("keydown", (e) => {
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
  if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
  const open = [...document.querySelectorAll("dialog.model-dlg[open]")].pop();
  if (!open) return;
  e.preventDefault();
  open.querySelector(e.key === "ArrowLeft" ? ".dlg-prev" : ".dlg-next")?.click();
});
