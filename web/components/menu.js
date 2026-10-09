// A menu of actions: under its button (the memory view's "⋯" and "New"),
// or at the pointer (a context menu). One at a time; `position: fixed`
// in the element it is put in, so no scrolling box around its button
// clips it. Keyboard as the WAI-ARIA menu pattern: arrows that wrap,
// Home/End, a letter jumps to the next item starting with it, Enter or
// Space runs an item, Escape closes and gives the focus back (and stops
// there: the memory view's own Escape gives the keys to the calculator).
// Tab, a click outside, a scroll and a resize close it too.

import { iconEl } from "./icons.js";

/**
 * Where a menu of `size` goes in `view` (both `{width, height}`): under
 * `anchor` (a rect; a point has `left === right`), its right edges
 * aligned (`align: "end"`) or its left ones (`"start"`), above it when
 * there is no room below, and never past the `gutter`. A point opens
 * to its right, or to its left when there is no room.
 */
export function placeMenu(anchor, size, view, { align = "end", gap = 4, gutter = 8 } = {}) {
  const point = anchor.left === anchor.right && anchor.top === anchor.bottom;
  const roomBelow = view.height - gutter - (anchor.bottom + gap);
  const roomAbove = anchor.top - gap - gutter;
  const flipped = size.height > roomBelow && roomAbove > roomBelow;
  let y = flipped ? anchor.top - gap - size.height : anchor.bottom + (point ? 0 : gap);
  y = Math.max(gutter, Math.min(y, view.height - gutter - size.height));
  let x;
  if (point) x = anchor.left + size.width > view.width - gutter ? anchor.left - size.width : anchor.left;
  else x = align === "end" ? anchor.right - size.width : anchor.left;
  x = Math.max(gutter, Math.min(x, view.width - gutter - size.width));
  return { x: Math.round(x), y: Math.round(y), flipped };
}

let open = null;
let serial = 0;

/** Close the open menu, if any (`restore`: the focus goes back). */
export function closeMenu(restore = false) {
  open?.close(restore);
}

/** The open menu's `key` (what it was opened for), or null. */
export function openMenuKey() {
  return open?.key ?? null;
}

/** Give the open menu new `items` when it is the one opened for `key`. */
export function updateMenu(key, items) {
  if (open && open.key === key) open.update(items);
}

/** The open menu's element id, or null. */
export function openMenuId() {
  return open?.menu.id ?? null;
}

/**
 * Open a menu. `items`: `{text, icon?, hint?, danger?, disabled?,
 * blocked?, title?, write?, run(keyboard)}`, `"-"` for a rule, or
 * `{note}` for a line of text. `write` items are switched off and on
 * with the writes (their `data-write`), `blocked` ones stay off.
 * `anchor` (an element) or `at` (`{x, y}`, the pointer) places it;
 * `host` holds it (the focus stays inside the host). `returnFocus()`
 * names the element that gets the focus back on Escape, on Tab and when
 * an item runs by the keyboard (its button or row); after a click on an
 * item or outside, the focus leaves (a click gives the page no focus).
 * `last` focuses the last item.
 */
export function openMenu({ anchor = null, at = null, align = "end", host = document.body, label, items, key = null, returnFocus = null, last = false, onClose = null }) {
  closeMenu();
  const menu = document.createElement("div");
  menu.className = "menu";
  menu.id = `menu-${++serial}`;
  menu.setAttribute("role", "menu");
  menu.setAttribute("aria-label", label);
  menu.setAttribute("aria-orientation", "vertical");
  let buttons = [];
  const fill = (list) => {
    menu.replaceChildren();
    menu.removeAttribute("aria-describedby");
    buttons = [];
    for (const it of list) {
      if (it === "-") {
        const s = document.createElement("div");
        s.setAttribute("role", "separator");
        menu.append(s);
      } else if (it.note) {
        const p = document.createElement("p");
        p.className = "menu-note";
        p.id = `${menu.id}-note`;
        p.setAttribute("role", "none");
        p.textContent = it.note;
        menu.setAttribute("aria-describedby", p.id);
        menu.append(p);
      } else {
        const b = document.createElement("button");
        b.type = "button";
        b.setAttribute("role", "menuitem");
        b.tabIndex = -1;
        if (it.danger) b.classList.add("danger");
        if (it.disabled) b.setAttribute("aria-disabled", "true");
        if (it.write) b.dataset.write = "";
        if (it.blocked) b.dataset.blocked = "";
        if (it.title) b.title = it.title;
        if (it.icon) b.append(iconEl(it.icon));
        const text = document.createElement("span");
        text.className = "menu-text";
        text.textContent = it.text;
        b.append(text);
        if (it.hint) {
          const k = document.createElement("kbd");
          k.className = "kbd-hint";
          k.textContent = it.hint;
          b.append(k);
        }
        b.addEventListener("click", (e) => {
          if (b.getAttribute("aria-disabled") === "true") return;
          // The focus goes back first (what the item opens may take it);
          // after a click it leaves, as a click gives the page none.
          close(e.detail === 0);
          it.run(e.detail === 0);
        });
        buttons.push(b);
        menu.append(b);
      }
    }
  };
  fill(items);
  menu.addEventListener("contextmenu", (e) => e.preventDefault());
  host.append(menu);

  // Placed where asked; any box that holds a fixed element in place of
  // the viewport (a transform, a containment) is measured and undone.
  const view = { width: document.documentElement.clientWidth, height: window.innerHeight };
  menu.style.maxWidth = `${view.width - 16}px`;
  menu.style.maxHeight = `${view.height - 16}px`;
  const r = anchor?.getBoundingClientRect();
  const rect = r ? { left: r.left, right: r.right, top: r.top, bottom: r.bottom } : { left: at.x, right: at.x, top: at.y, bottom: at.y };
  const place = () => {
    menu.style.left = "0px";
    menu.style.top = "0px";
    const size = menu.getBoundingClientRect();
    const p = placeMenu(rect, { width: size.width, height: size.height }, view, { align });
    menu.style.left = `${p.x - size.left}px`;
    menu.style.top = `${p.y - size.top}px`;
    menu.classList.toggle("flipped", p.flipped);
  };
  place();

  const focus = (i) => {
    if (!buttons.length) return;
    const n = ((i % buttons.length) + buttons.length) % buttons.length;
    for (const [j, b] of buttons.entries()) b.tabIndex = j === n ? 0 : -1;
    buttons[n].focus();
  };
  const index = () => buttons.indexOf(document.activeElement);
  menu.addEventListener("keydown", (e) => {
    const i = index();
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close(true);
      return;
    }
    if (e.key === "Tab") {
      // From the button (or row) it belongs to, Tab moves on as usual.
      close(true);
      return;
    }
    const to = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: buttons.length - 1 }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      e.stopPropagation();
      focus(to);
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      e.stopPropagation();
      buttons[i]?.click();
      return;
    }
    if (e.key.length === 1 && /\S/.test(e.key) && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      e.stopPropagation();
      const c = e.key.toLowerCase();
      for (let k = 1; k <= buttons.length; k++) {
        const b = buttons[(i + k) % buttons.length];
        if (b.textContent.trim().toLowerCase().startsWith(c)) {
          focus(buttons.indexOf(b));
          break;
        }
      }
    }
  });
  const outside = (e) => {
    if (menu.contains(e.target)) return;
    // A press on its own button (drawn again, maybe): its click closes it.
    if (anchor?.contains(e.target) || e.target.closest?.(`[aria-controls="${menu.id}"]`)) return;
    close(false);
  };
  const scrolled = (e) => {
    if (!menu.contains(e.target)) close(false);
  };
  const resized = () => close(false);
  document.addEventListener("pointerdown", outside, true);
  document.addEventListener("scroll", scrolled, true);
  window.addEventListener("resize", resized);

  function close(restore) {
    if (open?.menu !== menu) return;
    open = null;
    document.removeEventListener("pointerdown", outside, true);
    document.removeEventListener("scroll", scrolled, true);
    window.removeEventListener("resize", resized);
    const had = menu.contains(document.activeElement);
    menu.remove();
    const to = restore ? returnFocus?.() : null;
    if (to) to.focus();
    else if (had && document.activeElement instanceof HTMLElement) document.activeElement.blur();
    onClose?.();
  }
  /** New items in place (the object they act on was read meanwhile); the focus stays on its item. */
  const update = (list) => {
    const was = document.activeElement?.closest?.("[role=menuitem]")?.textContent ?? null;
    const at = buttons.findIndex((b) => b.textContent === was);
    fill(list);
    place();
    if (was === null) return;
    const i = buttons.findIndex((b) => b.textContent === was);
    focus(i >= 0 ? i : Math.min(Math.max(at, 0), buttons.length - 1));
  };
  open = { menu, key, close, update };
  focus(last ? buttons.length - 1 : 0);
  return { menu, close };
}
