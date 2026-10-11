// <device-list>: the variables of one directory of a device
// (web/shared/device.js has the Device interface): name, type, size in
// bytes as the calculator counts them, checksum as BYTES shows it.
// Directories first; a column heading sorts by it (again: the other way).
// A row is selected by a click or the arrow keys (`select`, `{entry}`),
// a directory opens on a double click or Enter (`open`, `{entry}`, also
// for the "up" row, whose entry is `{up: true}`), and each row has a "⋯"
// button and a context menu with what `actions(entry)` returns (menu
// items, shared/components/menu.js). Files dropped on it come out as
// `dropfiles` (`{files}`). Light DOM; its look is in
// shared/components.css (`.dlist`).

import { el } from "../dom.js";
import { COLUMNS, HOME, checksumCell, samePath, sizeCell, sortEntries } from "../device.js";
import { iconEl } from "./icons.js";
import { openMenu } from "./menu.js";

export class DeviceList extends HTMLElement {
  constructor() {
    super();
    this._device = null;
    this._path = [...HOME];
    this.listing = null;
    this.error = null;
    this.loading = false;
    this.sortKey = "name";
    this.ascending = true;
    this.selected = null;
    /** What a row's "⋯" and context menu offer: `(entry) => items`. */
    this.actions = () => [];
    /** The text shown with no device. */
    this.emptyText = "";
    this.onChange = (e) => {
      if (!e.detail?.path || samePath(e.detail.path, this._path)) this.reload();
    };
    this.addEventListener("click", (e) => this.click(e));
    this.addEventListener("dblclick", (e) => this.dblclick(e));
    this.addEventListener("keydown", (e) => this.key(e));
    this.addEventListener("contextmenu", (e) => this.context(e));
    this.addEventListener("dragover", (e) => this.dragover(e));
    this.addEventListener("dragleave", (e) => {
      if (!this.contains(e.relatedTarget)) this.classList.remove("drop");
    });
    this.addEventListener("drop", (e) => this.drop(e));
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
    this._path = [...HOME];
    this.selected = null;
    this.reload();
  }

  /** The directory shown. */
  get path() { return this._path; }
  set path(p) {
    if (samePath(p, this._path) && this.listing) return;
    this._path = [...p];
    this.selected = null;
    this.reload();
  }

  /** The selected entry, or null. */
  get selection() {
    return this.listing?.entries.find((e) => e.name === this.selected) ?? null;
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
      if (this.selected && !listing.entries.some((e) => e.name === this.selected)) this.selected = null;
    } catch (err) {
      if (!samePath(path, this._path)) return;
      this.error = String(err?.message ?? err);
    }
    this.loading = false;
    this.render();
    this.dispatchEvent(new CustomEvent("listed", { detail: { path: [...path], listing: this.listing, error: this.error } }));
  }

  /** Select the row of `name` (null: none). */
  select(name, { focus = false } = {}) {
    this.selected = name;
    for (const r of this.querySelectorAll("tr[data-name]")) {
      const on = r.dataset.name === name;
      r.setAttribute("aria-selected", String(on));
      r.tabIndex = on ? 0 : -1;
      if (on && focus) r.focus();
    }
    this.dispatchEvent(new CustomEvent("select", { detail: { entry: this.selection } }));
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
    for (const [key, title] of COLUMNS) {
      const sorted = this.sortKey === key;
      const th = el("th", { scope: "col", class: `col-${key}`, "aria-sort": sorted ? (this.ascending ? "ascending" : "descending") : null });
      const b = el("button", { type: "button", class: "dlist-sort", "data-sort": key }, title);
      if (sorted) b.append(iconEl(this.ascending ? "chevron-down" : "chevron-up", "ic-sm"));
      th.append(b);
      head.append(th);
    }
    head.append(el("th", { scope: "col", class: "col-more" }, el("span", { class: "visually-hidden", text: "Actions" })));
    const body = el("tbody", {});
    if (this._path.length > 1) {
      const up = el("tr", { class: "dlist-up", "data-up": "", tabindex: "-1" });
      up.append(el("td", { class: "col-name", colspan: "5" }, iconEl("chevron-left", "ic-sm"), el("span", { text: "Up one level" })));
      body.append(up);
    }
    const entries = sortEntries(this.listing.entries, this.sortKey, this.ascending);
    for (const e of entries) {
      const on = e.name === this.selected;
      const row = el("tr", { "data-name": e.name, "aria-selected": String(on), tabindex: on ? "0" : "-1", class: e.directory ? "dir" : null });
      const name = el("td", { class: "col-name" }, iconEl(e.directory ? "folder" : "file", "ic-sm"), el("span", { class: "dlist-name", text: e.name }), el("span", { class: "dlist-kind-inline muted", text: e.kind }));
      const more = el("button", { type: "button", class: "icon more", title: `What to do with ${e.name}`, "aria-label": `Actions for ${e.name}` }, iconEl("more"));
      row.append(name, el("td", { class: "col-kind", text: e.kind }), el("td", { class: "col-size num", text: sizeCell(e.size) }), el("td", { class: "col-checksum num", text: checksumCell(e.checksum) }), el("td", { class: "col-more" }, more));
      body.append(row);
    }
    const table = el("table", { "aria-label": `Variables in ${this._path.join(" ")}` }, el("thead", {}, head), body);
    const nodes = [el("div", { class: "dlist-wrap" }, table)];
    if (!entries.length) nodes.push(el("p", { class: "dlist-state muted", text: "This directory is empty. Drop a file here to put it on the calculator." }));
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
    this.select(entry.name);
    const items = this.actions(entry);
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
    else if (entry) this.select(entry.name);
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
      if (entry && !entry.up) this.select(entry.name);
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

  dragover(e) {
    if (!this._device || ![...(e.dataTransfer?.types ?? [])].includes("Files")) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    this.classList.add("drop");
  }

  drop(e) {
    this.classList.remove("drop");
    const files = [...(e.dataTransfer?.files ?? [])];
    if (!this._device || !files.length) return;
    e.preventDefault();
    this.dispatchEvent(new CustomEvent("dropfiles", { detail: { files, path: [...this._path] } }));
  }
}

if (!customElements.get("device-list")) customElements.define("device-list", DeviceList);
