// <device-list>: the entries of one directory of a place (web/shared/
// device.js has the interface): a calculator's variables (name, type, size
// in bytes as the calculator counts them, checksum as BYTES shows it) or a
// folder's files (`columns` = FOLDER_COLUMNS: the modified date instead of
// the checksum). Directories first; a column heading sorts by it (again:
// the other way).
//
// Selection: a click or the arrow keys select one row; Ctrl or Cmd with a
// click adds or takes away a row, Shift with a click or an arrow selects a
// range, Ctrl/Cmd+A all of them (`select`, `{entry, entries}`). A
// directory opens on a double click or Enter (`open`, `{entry}`, also for
// the "up" row, whose entry is `{up: true}`), and each row has a "⋯"
// button and a context menu with what `actions(entry, entries)` returns
// (menu items, shared/components/menu.js). Rows can be dragged
// (`rowsDraggable`): the drag carries `DRAG_TYPE` with `{source, path,
// names}` (`dragSource` names the page and the pane), and `dragOut(entries)`
// may add a `DownloadURL` (Chromium: a drag to the desktop). Drops are the
// pane's to handle. Light DOM; its look is in shared/components.css
// (`.dlist`).

import { el } from "../dom.js";
import { COLUMNS, checksumCell, homeOf, modifiedCell, samePath, sizeCell, sortEntries } from "../device.js";
import { iconEl } from "./icons.js";
import { openMenu } from "./menu.js";

/** The drag data type of rows dragged out of a list. */
export const DRAG_TYPE = "application/x-saturnus-tx";

const CELL = {
  size: (e) => sizeCell(e.size),
  checksum: (e) => checksumCell(e.checksum),
  modified: (e) => modifiedCell(e.modified),
};

export class DeviceList extends HTMLElement {
  constructor() {
    super();
    this._device = null;
    this._path = [...homeOf(null)];
    this.listing = null;
    this.error = null;
    this.loading = false;
    this.sortKey = "name";
    this.ascending = true;
    /** The focused (last clicked) row's name, or null. */
    this.selected = null;
    /** Every selected row's name. */
    this.marked = new Set();
    /** Where a Shift selection starts. */
    this.anchor = null;
    /** The columns shown: `[key, heading]` (shared/device.js). */
    this.columns = COLUMNS;
    /** What a row's "⋯" and context menu offer: `(entry, entries) => items`. */
    this.actions = () => [];
    /** The text shown with no place. */
    this.emptyText = "";
    /** The text under an empty directory. */
    this.emptyDirText = "This directory is empty.";
    /** The table's name for a screen reader, given the path. */
    this.tableLabel = (path) => `Contents of ${path.join(" ")}`;
    /** Whether rows can be dragged; `dragSource` goes with them. */
    this.rowsDraggable = false;
    this.dragSource = null;
    /** `(entries) => "mime:name:url"` or null: a DownloadURL for a drag out. */
    this.dragOut = null;
    this.onChange = (e) => {
      if (!e.detail?.path || samePath(e.detail.path, this._path)) this.reload();
    };
    this.addEventListener("click", (e) => this.click(e));
    this.addEventListener("dblclick", (e) => this.dblclick(e));
    this.addEventListener("keydown", (e) => this.key(e));
    this.addEventListener("contextmenu", (e) => this.context(e));
    this.addEventListener("dragstart", (e) => this.dragstart(e));
  }

  connectedCallback() {
    this.classList.add("dlist");
    this.render();
  }

  get device() { return this._device; }
  set device(d) {
    this._device?.removeEventListener("change", this.onChange);
    this._device = d;
    d?.addEventListener("change", this.onChange);
    this._path = [...homeOf(d)];
    this.selected = null;
    this.marked = new Set();
    this.reload();
  }

  /** The directory shown. */
  get path() { return this._path; }
  set path(p) {
    if (samePath(p, this._path) && this.listing) return;
    this._path = [...p];
    this.selected = null;
    this.marked = new Set();
    this.reload();
  }

  /** The focused selected entry, or null. */
  get selection() {
    return this.listing?.entries.find((e) => e.name === this.selected) ?? null;
  }

  /** Every selected entry, in the list's order. */
  get selectedEntries() {
    if (!this.listing) return [];
    return sortEntries(this.listing.entries, this.sortKey, this.ascending).filter((e) => this.marked.has(e.name));
  }

  async reload() {
    this.listing = null;
    this.error = null;
    if (!this._device) {
      this.render();
      return;
    }
    const path = this._path;
    this.loading = true;
    this.render();
    try {
      const listing = await this._device.list(path);
      if (!samePath(path, this._path)) return;
      this.listing = listing;
      const names = new Set(listing.entries.map((e) => e.name));
      if (this.selected && !names.has(this.selected)) this.selected = null;
      this.marked = new Set([...this.marked].filter((n) => names.has(n)));
    } catch (err) {
      if (!samePath(path, this._path)) return;
      this.error = String(err?.message ?? err);
    }
    this.loading = false;
    this.render();
    this.dispatchEvent(new CustomEvent("listed", { detail: { path: [...path], listing: this.listing, error: this.error } }));
  }

  /** The names in the list's order. */
  orderedNames() {
    return sortEntries(this.listing?.entries ?? [], this.sortKey, this.ascending).map((e) => e.name);
  }

  /**
   * Select the row of `name` (null: none). `add` toggles it in the
   * selection, `range` selects from the focused row to it.
   */
  select(name, { focus = false, add = false, range = false } = {}) {
    const anchor = this.anchor ?? this.selected;
    if (name === null) this.marked = new Set();
    else if (range && anchor !== null && anchor !== undefined) {
      const names = this.orderedNames();
      const [a, b] = [names.indexOf(anchor), names.indexOf(name)].sort((x, y) => x - y);
      if (a >= 0 && b >= 0) this.marked = new Set([...(add ? this.marked : []), ...names.slice(a, b + 1)]);
      else this.marked = new Set([name]);
    } else if (add) {
      if (this.marked.has(name)) this.marked.delete(name);
      else this.marked.add(name);
    } else this.marked = new Set([name]);
    if (!range) this.anchor = name;
    this.selected = name !== null && this.marked.has(name) ? name : [...this.marked].at(-1) ?? null;
    this.paintSelection(focus);
    this.dispatchEvent(new CustomEvent("select", { detail: { entry: this.selection, entries: this.selectedEntries } }));
  }

  /** Select every entry. */
  selectAll() {
    this.marked = new Set(this.orderedNames());
    if (!this.selected) this.selected = this.orderedNames()[0] ?? null;
    this.paintSelection(false);
    this.dispatchEvent(new CustomEvent("select", { detail: { entry: this.selection, entries: this.selectedEntries } }));
  }

  paintSelection(focus) {
    for (const r of this.querySelectorAll("tr[data-name]")) {
      const on = this.marked.has(r.dataset.name);
      r.setAttribute("aria-selected", String(on));
      const here = r.dataset.name === this.selected;
      r.tabIndex = here ? 0 : -1;
      if (here && focus) r.focus();
    }
    if (!this.selected) this.querySelector("tbody tr")?.setAttribute("tabindex", "0");
  }

  sortBy(key) {
    if (this.sortKey === key) this.ascending = !this.ascending;
    else {
      this.sortKey = key;
      this.ascending = true;
    }
    this.render();
  }

  render() {
    if (!this.isConnected) return;
    if (!this._device) {
      this.replaceChildren(el("p", { class: "dlist-state muted", text: this.emptyText }));
      return;
    }
    if (this.error) {
      this.replaceChildren(el("p", { class: "dlist-state error", role: "alert", text: `This directory could not be read: ${this.error}` }));
      return;
    }
    if (!this.listing) {
      this.replaceChildren(el("p", { class: "dlist-state muted", text: "Reading the directory…" }));
      return;
    }
    const head = el("tr", {});
    for (const [key, title] of this.columns) {
      const sorted = this.sortKey === key;
      const th = el("th", { scope: "col", class: `col-${key}`, "aria-sort": sorted ? (this.ascending ? "ascending" : "descending") : null });
      const b = el("button", { type: "button", class: "dlist-sort", "data-sort": key }, title);
      if (sorted) b.append(iconEl(this.ascending ? "chevron-down" : "chevron-up", "ic-sm"));
      th.append(b);
      head.append(th);
    }
    head.append(el("th", { scope: "col", class: "col-more" }, el("span", { class: "visually-hidden", text: "Actions" })));
    const body = el("tbody", {});
    const span = String(this.columns.length + 1);
    if (this._path.length > 1) {
      const up = el("tr", { class: "dlist-up", "data-up": "", tabindex: "-1" });
      up.append(el("td", { class: "col-name", colspan: span }, iconEl("chevron-left", "ic-sm"), el("span", { text: "Up one level" })));
      body.append(up);
    }
    const entries = sortEntries(this.listing.entries, this.sortKey, this.ascending);
    for (const e of entries) {
      const on = this.marked.has(e.name);
      const row = el("tr", { "data-name": e.name, "aria-selected": String(on), tabindex: e.name === this.selected ? "0" : "-1", class: e.directory ? "dir" : null, draggable: this.rowsDraggable ? "true" : null });
      const name = el("td", { class: "col-name" }, iconEl(e.directory ? "folder" : "file", "ic-sm"), el("span", { class: "dlist-name", text: e.name }), el("span", { class: "dlist-kind-inline muted", text: e.kind }));
      const more = el("button", { type: "button", class: "icon more", title: `What to do with ${e.name}`, "aria-label": `Actions for ${e.name}` }, iconEl("more"));
      row.append(name);
      for (const [key] of this.columns.slice(1)) {
        if (key === "kind") row.append(el("td", { class: "col-kind", text: e.kind }));
        else row.append(el("td", { class: `col-${key} num`, text: (CELL[key] ?? ((x) => String(x[key] ?? "")))(e) }));
      }
      row.append(el("td", { class: "col-more" }, more));
      body.append(row);
    }
    const table = el("table", { "aria-label": this.tableLabel(this._path), "aria-multiselectable": "true" }, el("thead", {}, head), body);
    const nodes = [el("div", { class: "dlist-wrap" }, table)];
    if (!entries.length) nodes.push(el("p", { class: "dlist-state muted", text: this.emptyDirText }));
    // One row takes the Tab: the selected one, else the first.
    if (!body.querySelector('tr[tabindex="0"]')) body.querySelector("tr")?.setAttribute("tabindex", "0");
    this.replaceChildren(...nodes);
  }

  rowOf(target) { return target.closest?.("tr[data-name], tr[data-up]") ?? null; }
  entryOf(row) {
    if (!row) return null;
    if (row.hasAttribute("data-up")) return { up: true };
    return this.listing?.entries.find((e) => e.name === row.dataset.name) ?? null;
  }

  menu(row, opts) {
    const entry = this.entryOf(row);
    if (!entry || entry.up) return;
    // A row outside the selection becomes it; one inside keeps the selection.
    if (!this.marked.has(entry.name)) this.select(entry.name);
    else {
      this.selected = entry.name;
      this.paintSelection(false);
    }
    const items = this.actions(entry, this.selectedEntries);
    if (!items.length) return;
    openMenu({ ...opts, label: `Actions for ${entry.name}`, items, returnFocus: () => row });
  }

  click(e) {
    const sort = e.target.closest?.("button.dlist-sort");
    if (sort) {
      this.sortBy(sort.dataset.sort);
      return;
    }
    const row = this.rowOf(e.target);
    if (!row) return;
    if (e.target.closest("button.more")) {
      this.menu(row, { anchor: e.target.closest("button.more") });
      return;
    }
    const entry = this.entryOf(row);
    if (entry?.up) this.dispatchEvent(new CustomEvent("open", { detail: { entry } }));
    else if (entry) this.select(entry.name, { add: e.ctrlKey || e.metaKey, range: e.shiftKey });
  }

  dblclick(e) {
    const entry = this.entryOf(this.rowOf(e.target));
    if (entry && (entry.up || entry.directory)) this.dispatchEvent(new CustomEvent("open", { detail: { entry } }));
  }

  context(e) {
    const row = this.rowOf(e.target);
    if (!row || row.hasAttribute("data-up")) return;
    e.preventDefault();
    this.menu(row, { at: { x: e.clientX, y: e.clientY } });
  }

  key(e) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a" && !e.target.closest("input, textarea")) {
      if (!this.listing?.entries.length) return;
      e.preventDefault();
      this.selectAll();
      return;
    }
    const row = this.rowOf(e.target);
    if (!row || e.target.closest("button")) return;
    const rows = [...this.querySelectorAll("tr[data-name], tr[data-up]")];
    const i = rows.indexOf(row);
    const go = (r) => {
      if (!r) return;
      for (const x of rows) x.tabIndex = -1;
      r.tabIndex = 0;
      r.focus();
      const entry = this.entryOf(r);
      if (entry && !entry.up) this.select(entry.name, { range: e.shiftKey });
    };
    switch (e.key) {
      case "ArrowDown": go(rows[i + 1]); break;
      case "ArrowUp": go(rows[i - 1]); break;
      case "Home": go(rows[0]); break;
      case "End": go(rows[rows.length - 1]); break;
      case "Enter": {
        const entry = this.entryOf(row);
        if (entry && (entry.up || entry.directory)) this.dispatchEvent(new CustomEvent("open", { detail: { entry } }));
        break;
      }
      case "Backspace":
        if (this._path.length > 1) this.dispatchEvent(new CustomEvent("open", { detail: { entry: { up: true } } }));
        break;
      case "ContextMenu":
        this.menu(row, { anchor: row.querySelector("button.more") ?? row });
        break;
      case "F10":
        if (!e.shiftKey) return;
        this.menu(row, { anchor: row.querySelector("button.more") ?? row });
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  dragstart(e) {
    const row = e.target.closest?.("tr[data-name]");
    if (!row || !this.rowsDraggable || !e.dataTransfer) return;
    const name = row.dataset.name;
    // A row outside the selection drags alone; one inside drags the selection.
    if (!this.marked.has(name)) this.select(name);
    const entries = this.selectedEntries;
    e.dataTransfer.effectAllowed = "copy";
    e.dataTransfer.setData(DRAG_TYPE, JSON.stringify({ source: this.dragSource, path: [...this._path], names: entries.map((x) => x.name) }));
    e.dataTransfer.setData("text/plain", entries.map((x) => x.name).join("\n"));
    const url = this.dragOut?.(entries);
    if (url) e.dataTransfer.setData("DownloadURL", url);
  }
}

if (!customElements.get("device-list")) customElements.define("device-list", DeviceList);
