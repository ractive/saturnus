// <device-tree>: a device's directories from HOME, as a tree
// (web/shared/device.js has the interface: a calculator, or a folder). Each directory is
// listed when it is opened, never the whole tree at once (a listing takes
// about a second at 9600 baud); the device caches listings and says
// "change" when one changed. Clicking a directory, or Enter on it, makes
// it the shown one: the element dispatches `navigate` with `{path}`.
// Keys as a tree has them: Up and Down move, Right opens (or goes to the
// first child), Left closes (or goes to the parent), Home and End.
// Light DOM; its look is in shared/components.css (`.dtree`).

import { el } from "../dom.js";
import { HOME, directoriesOf, homeOf, pathKey, samePath } from "../device.js";
import { iconEl } from "./icons.js";

export class DeviceTree extends HTMLElement {
  constructor() {
    super();
    this._device = null;
    this._current = [...HOME];
    /** Open directories, by key. */
    this.open = new Set([pathKey(HOME)]);
    /** Listings by key: a listing, or `{error}`, or `"loading"`. */
    this.listings = new Map();
    this.onChange = (e) => {
      const key = e.detail?.path ? pathKey(e.detail.path) : null;
      if (key) this.listings.delete(key);
      else this.listings.clear();
      this.render();
    };
    this.addEventListener("click", (e) => this.click(e));
    this.addEventListener("keydown", (e) => this.key(e));
  }

  connectedCallback() {
    this.classList.add("dtree");
    this.setAttribute("role", "tree");
    if (!this.hasAttribute("aria-label")) this.setAttribute("aria-label", "Directories");
    this.render();
  }

  /** The device shown (null: none). */
  get device() { return this._device; }
  set device(d) {
    this._device?.removeEventListener("change", this.onChange);
    this._device = d;
    d?.addEventListener("change", this.onChange);
    this.open = new Set([pathKey(homeOf(d))]);
    this.listings.clear();
    this._current = [...homeOf(d)];
    this.render();
  }

  /** The shown directory: its row is selected, its parents open. */
  get current() { return this._current; }
  set current(path) {
    this._current = [...path];
    for (let i = 1; i < path.length; i++) this.open.add(pathKey(path.slice(0, i)));
    this.render();
  }

  /** The listing of `path`, from the device (it caches). */
  async load(path) {
    const key = pathKey(path);
    if (this.listings.has(key) || !this._device) return;
    this.listings.set(key, "loading");
    try {
      this.listings.set(key, await this._device.list(path));
    } catch (err) {
      this.listings.set(key, { error: String(err?.message ?? err) });
    }
    this.render();
  }

  /** The rows shown, in order: `{path, depth}`. */
  rows() {
    const out = [];
    const walk = (path, depth) => {
      out.push({ path, depth });
      const key = pathKey(path);
      if (!this.open.has(key)) return;
      const listing = this.listings.get(key);
      if (!listing || listing === "loading" || listing.error) return;
      for (const d of directoriesOf(listing)) walk([...path, d.name], depth + 1);
    };
    if (this._device) walk([...homeOf(this._device)], 1);
    return out;
  }

  render() {
    if (!this.isConnected) return;
    const focused = this.querySelector("[role=treeitem]:focus")?.dataset.path ?? null;
    // The tree is its own scroller, and WebKit (Safari, the desktop app)
    // drops it to the top when its rows are replaced: put it back, so a
    // click or a listing arriving never moves it.
    const top = this.scrollTop;
    const items = [];
    for (const { path, depth } of this.rows()) {
      const key = pathKey(path);
      const isOpen = this.open.has(key);
      if (isOpen && !this.listings.has(key)) this.load(path);
      // `load` marks the listing "loading" at once: this row says so.
      const listing = this.listings.get(key);
      const known = listing && listing !== "loading" && !listing.error;
      const hasChildren = known ? directoriesOf(listing).length > 0 : true;
      const selected = samePath(path, this._current);
      const row = el("div", {
        role: "treeitem",
        class: "dtree-row",
        "aria-level": String(depth),
        "aria-selected": selected ? "true" : "false",
        "aria-expanded": hasChildren ? String(isOpen) : null,
        tabindex: selected ? "0" : "-1",
        "data-path": key,
        style: `--depth: ${depth - 1}`,
      });
      const toggle = el("span", { class: "dtree-toggle", "aria-hidden": "true" });
      if (hasChildren) toggle.append(iconEl(isOpen ? "chevron-down" : "chevron-right", "ic-sm"));
      const name = el("span", { class: "dtree-name", text: path[path.length - 1] });
      row.append(toggle, iconEl(isOpen ? "folder-open" : "folder", "ic-sm"), name);
      if (isOpen && listing === "loading") row.append(el("span", { class: "dtree-note muted", text: "…" }));
      if (listing?.error) {
        row.append(el("span", { class: "dtree-note error", text: "not read" }));
        row.title = listing.error;
      }
      items.push(row);
    }
    this.replaceChildren(...items);
    if (this.scrollTop !== top) this.scrollTop = top;
    if (focused) this.querySelector(`[data-path="${CSS.escape(focused)}"]`)?.focus({ preventScroll: true });
  }

  toggle(path) {
    const key = pathKey(path);
    if (this.open.has(key)) this.open.delete(key);
    else this.open.add(key);
    this.render();
  }

  navigate(path) {
    this._current = [...path];
    this.render();
    this.dispatchEvent(new CustomEvent("navigate", { detail: { path: [...path] }, bubbles: true }));
  }

  pathOf(row) { return row.dataset.path.split("/"); }

  click(e) {
    const row = e.target.closest?.("[role=treeitem]");
    if (!row) return;
    const path = this.pathOf(row);
    if (e.target.closest(".dtree-toggle") && row.hasAttribute("aria-expanded")) this.toggle(path);
    else this.navigate(path);
  }

  key(e) {
    const row = e.target.closest?.("[role=treeitem]");
    if (!row) return;
    const rows = [...this.querySelectorAll("[role=treeitem]")];
    const i = rows.indexOf(row);
    const path = this.pathOf(row);
    const focus = (r) => {
      if (!r) return;
      for (const x of rows) x.tabIndex = -1;
      r.tabIndex = 0;
      // Only as far as it takes to show the row.
      r.focus({ preventScroll: true });
      r.scrollIntoView({ block: "nearest" });
    };
    const expanded = row.getAttribute("aria-expanded");
    switch (e.key) {
      case "ArrowDown": focus(rows[i + 1]); break;
      case "ArrowUp": focus(rows[i - 1]); break;
      case "Home": focus(rows[0]); break;
      case "End": focus(rows[rows.length - 1]); break;
      case "ArrowRight":
        if (expanded === "false") this.toggle(path);
        else if (expanded === "true") focus(rows[i + 1]);
        break;
      case "ArrowLeft":
        if (expanded === "true") this.toggle(path);
        else if (path.length > 1) focus(this.querySelector(`[data-path="${CSS.escape(pathKey(path.slice(0, -1)))}"]`));
        break;
      case "Enter":
      case " ":
        this.navigate(path);
        break;
      default:
        return;
    }
    e.preventDefault();
  }
}

if (!customElements.get("device-tree")) customElements.define("device-tree", DeviceTree);
