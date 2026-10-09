// The memory view's actions on what it shows, as data: one primary
// action for the preview's button, the rest for its "⋯" menu (or, when
// only one is left, beside the primary instead of a menu of one). The
// same set wherever the subject was chosen: a row of the list, a node
// of the tree, the crumbs. The context menu is the primary and the rest.
// Pure (no DOM), so the set per subject is tested in node
// (test/actions.test.mjs); sat-explorer.js gives each id its deed.

/**
 * The actions for `subject`:
 * - `kind`: "object" (a variable that is not a directory), "dir" (a
 *   directory selected in the list), "shown" (the directory shown, not
 *   HOME), "home" (HOME shown), "level" (a stack level);
 * - `name`: its name (the menu's note for HOME);
 * - `current`: a directory that is the calculator's current one.
 *
 * `ctx`: `writes` (the model has writes), `off` (they are off now: one
 * runs, or the calculator is typing), `editor` (an editor is attached),
 * `read` ("reading", "failed", "text" or "textless": the object's state),
 * `copy` (its text can be copied), `hints` (key labels by id).
 *
 * Returns `{primary, menu, inline}`: `primary` an action or null, `menu`
 * actions with "-" between groups and `{note}` lines, `inline` the
 * actions drawn beside the primary when they are too few for a menu.
 * An action: `{id, text, icon, title, disabled, write, danger, hint}`;
 * `blocked`: off whatever the writes say (an Edit with nothing to edit).
 */
export function subjectActions(subject, ctx) {
  const { kind } = subject;
  const { writes, off, editor, read, copy, hints = {} } = ctx;
  const dir = kind === "dir" || kind === "shown" || kind === "home";
  const act = (id, text, icon, title, extra = {}) => ({ id, text, icon, title, disabled: false, write: false, danger: false, hint: hints[id] ?? "", ...extra });
  const write = (id, text, icon, title, extra = {}) => act(id, text, icon, title, { write: true, disabled: off, ...extra });

  const edit = () => {
    const why = {
      reading: "Still reading it from the calculator",
      failed: "Could not read this object",
      textless: "This object has no text form",
    }[read];
    return write("edit", "Edit", "edit", why ?? "Edit its text; saving stores it back on the calculator", { disabled: off || Boolean(why), blocked: Boolean(why) });
  };
  const cd = () => write("cd", "Make current", "pin", "Make it the calculator's current directory");
  const open = () => act("open", "Open", "folder-open", "Show its variables");
  // Copy to…, Move to…, Rename…: where it is and what it is called.
  const place = () => (writes ? [
    write("copyto", "Copy to…", "copy-to", "Copy it into another directory"),
    write("moveto", "Move to…", "move-to", "Move it into another directory"),
    write("rename", "Rename…", "rename", "Give it another name"),
  ] : []);

  let primary = null;
  const groups = [];
  if (kind === "object" || kind === "level") {
    if (editor) primary = edit();
    groups.push([
      copy ? act("copy", "Copy text", "copy", "Copy the object's text form") : null,
      writes && kind === "object" ? write("save", "Save as file…", "save", "Save it as a file on this computer (HP binary format)") : null,
    ]);
    if (kind === "object") groups.push(place());
    if (writes && kind === "object") groups.push([write("purge", "Purge…", "trash", "Delete it", { danger: true })]);
  } else if (dir) {
    const canCd = writes && !subject.current;
    if (kind === "dir") {
      primary = open();
      groups.push([canCd ? cd() : null]);
    } else {
      // The directory shown is open already: no Open.
      primary = canCd ? cd() : null;
    }
    if (writes) {
      groups.push([
        write("store", "Store file here…", "load", "Store a file from this computer in it"),
        write("mkdir", "New directory here…", "folder", "Create an empty directory in it"),
      ]);
      if (kind !== "home") groups.push(place());
      if (kind !== "home") groups.push([write("purge", "Purge…", "trash", "Delete the directory with everything in it", { danger: true })]);
    }
  }

  const menu = [];
  for (const g of groups.map((g) => g.filter(Boolean)).filter((g) => g.length)) {
    if (menu.length) menu.push("-");
    menu.push(...g);
  }
  const items = menu.filter((x) => x !== "-");
  if (items.length <= 1) return { primary, menu: [], inline: items };
  if (kind === "home" && writes) menu.push({ note: `${subject.name ?? "HOME"} cannot be renamed or purged.` });
  return { primary, menu, inline: [] };
}

/** The context menu's items: the primary first, then the rest. */
export function contextItems({ primary, menu, inline }) {
  return [...(primary ? [primary] : []), ...inline, ...menu];
}
