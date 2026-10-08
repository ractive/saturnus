// <sat-explorer>: the side layer beside the calculator. Three tabs on
// the calculator's user memory, read live from RAM: Variables (the HOME
// tree, the variables of a directory, a typed preview), Stack and Flags;
// and a Commands tab, the command reference by the ROM's menus (the same
// entries the palette shows). Renders from the store; reads go through
// `MemoryView` (memory.js), the reference through a `ReferenceLoader`
// (palette.js). Browsing directories here is navigation in the page; the
// calculator's own current directory is marked as such. The writes
// (store a file, also by dropping it on a directory; save a variable as a
// file, rename, purge, make a directory current, set or clear a flag) go
// through `MemoryWrites` (writes.js), one at a time behind a busy
// overlay. Light DOM.
//
// Keyboard: the calculator keeps the keys unless the focus is inside this
// element (sat-calculator.js leaves those events alone). A mouse click on
// a row or a tab does not take the focus; Tab, the search fields and
// the "keys to the memory view" shortcut (Alt+M unless rebound, app.js)
// do, an indicator in the tab bar says so, and Escape gives the keys back.

import { MODEL_TITLES } from "./sat-calculator.js";
import { ObjectLoader } from "../memory.js";
import { newDirectoryRefusal, pathText } from "../writes.js";
import {
  checksumText, directoryAt, findVariables, flagRows, previewOf, sizeText, summary, typeTitle,
} from "../objects.js";
import { NOT_IN_MENU, OTHER_MENUS, exampleText, findCommands, findMenu, flattenMenus, menuCommands } from "../reference.js";
import { IndexWatch } from "../palette.js";
import { entryView } from "./entry-view.js";
import { icon, iconEl } from "./icons.js";

const TEMPLATE = `
  <section class="layer" aria-label="Memory view">
    <header class="layer-head">
      <button type="button" class="layer-back" title="Back to the calculator">${icon("chevron-left")}Calculator</button>
      <h2 class="layer-title">Memory</h2>
      <span class="layer-model"></span>
      <button type="button" class="icon layer-close" title="Close the memory view" aria-label="Close the memory view">${icon("chevron-right")}</button>
    </header>
    <div class="layer-tabs">
      <div class="tabs" role="tablist" aria-label="Memory view">
        <button type="button" role="tab" data-tab="vars" id="tab-vars" aria-controls="pane-vars">Variables</button>
        <button type="button" role="tab" data-tab="stack" id="tab-stack" aria-controls="pane-stack">Stack<span class="tab-count"></span></button>
        <button type="button" role="tab" data-tab="flags" id="tab-flags" aria-controls="pane-flags">Flags<span class="tab-count"></span></button>
        <button type="button" role="tab" data-tab="commands" id="tab-commands" aria-controls="pane-commands">Commands</button>
      </div>
      <p class="layer-keys" aria-live="polite">${icon("keyboard")}<span class="layer-keys-text"></span></p>
    </div>
    <p class="layer-note" role="status" hidden></p>
    <p class="layer-msg" role="status" hidden></p>
    <div class="layer-busy" hidden>
      <p class="layer-busy-text"></p>
      <p class="layer-busy-detail">Through the calculator's Kermit server, out of sight; its screen comes back when this is done.</p>
    </div>
    <div class="layer-empty" hidden>
      <h3></h3>
      <p class="layer-empty-text"></p>
      <p class="layer-empty-detail"></p>
    </div>

    <div class="pane pane-vars" id="pane-vars" role="tabpanel" aria-labelledby="tab-vars">
      <div class="vars-bar">
        <nav class="crumbs" aria-label="Directory shown"></nav>
        <button type="button" class="vars-store write" title="Store a file from this computer in the directory shown (or drop files on a directory)">Store file…</button>
        <button type="button" class="vars-mkdir write" title="Create an empty directory in the directory shown">New directory…</button>
        <input type="file" class="vars-file" multiple hidden>
        <input class="find vars-find" type="search" placeholder="Find a variable" aria-label="Find a variable in all directories" autocomplete="off" spellcheck="false">
      </div>
      <div class="vars-new"></div>
      <p class="vars-where"></p>
      <div class="vars-split">
        <div class="tree" role="tree" aria-label="Directories"></div>
        <div class="list-wrap">
          <table class="list">
            <thead><tr><th scope="col">Name</th><th scope="col">Type</th><th scope="col" class="num">Size</th><th scope="col" class="num sum">Checksum</th></tr></thead>
            <tbody></tbody>
          </table>
          <p class="list-empty" hidden></p>
        </div>
      </div>
      <div class="preview" aria-live="polite"></div>
    </div>

    <div class="pane pane-stack" id="pane-stack" role="tabpanel" aria-labelledby="tab-stack" hidden>
      <div class="levels-wrap"><ol class="levels" role="listbox" aria-label="Stack levels"></ol></div>
      <div class="preview" aria-live="polite"></div>
    </div>

    <div class="pane pane-flags" id="pane-flags" role="tabpanel" aria-labelledby="tab-flags" hidden>
      <div class="flags-bar">
        <input class="find flags-find" type="search" placeholder="Find a flag" aria-label="Find a flag by number or meaning" autocomplete="off" spellcheck="false">
        <button type="button" class="flags-only" aria-pressed="false">Only set</button>
      </div>
      <div class="flags-scroll" tabindex="0" aria-label="Flags"></div>
    </div>

    <div class="pane pane-commands" id="pane-commands" role="tabpanel" aria-labelledby="tab-commands" hidden>
      <div class="cmds-bar">
        <input class="find cmds-find" type="search" placeholder="Find a command" aria-label="Find a command by name or description" autocomplete="off" spellcheck="false">
        <span class="cmds-count"></span>
      </div>
      <p class="cmds-note" hidden></p>
      <div class="cmds-split">
        <div class="tree cmds-menus" role="tree" aria-label="Menus"></div>
        <div class="list-wrap">
          <table class="list cmds-list">
            <thead><tr><th scope="col">Name</th><th scope="col">Stack</th></tr></thead>
            <tbody></tbody>
          </table>
          <p class="list-empty" hidden></p>
        </div>
      </div>
      <div class="preview cmds-entry" aria-live="polite"></div>
    </div>
  </section>`;

const TABS = ["vars", "stack", "flags", "commands"];

function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === false || v === null || v === undefined) continue;
    if (k === "class") e.className = v;
    else if (k === "text") e.textContent = v;
    else e.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children) if (c !== null && c !== undefined && c !== false) e.append(c);
  return e;
}

/** The calculator's type names in sentence case: "Real Number" as "Real number". */
const typeName = (t) => String(t).replace(/ ([A-Z])(?=[a-z])/g, (_, c) => ` ${c.toLowerCase()}`);
const same = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);
const sentence = (s) => {
  const t = String(s ?? "").trim();
  return t ? t[0].toUpperCase() + t.slice(1) + (/[.!?]$/.test(t) ? "" : ".") : "";
};

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch { /* not allowed here: the old way */ }
  const area = el("textarea", { class: "copy-area", "aria-hidden": "true" });
  area.value = text;
  document.body.append(area);
  area.select();
  let ok = false;
  try { ok = document.execCommand("copy"); } finally { area.remove(); }
  if (!ok) throw new Error("the browser refused access to the clipboard");
}

export class SatExplorer extends HTMLElement {
  /**
   * `edit(target)` opens an object in the palette's editor (`{kind:
   * "variable", dir, name}` or `{kind: "level", level}`); without it there
   * are no Edit buttons.
   */
  attach(memory, store, prefs, { reference = null, backend = null, bindings = null, writes = null, edit = null } = {}) {
    this.bindings = bindings;
    this.writes = writes;
    this.edit = edit;
    this.memory = memory;
    this.store = store;
    this.prefs = prefs;
    this.reference = reference;
    this.backend = backend;
    this.innerHTML = TEMPLATE;
    const $ = (sel) => this.querySelector(sel);
    this.ui = {
      model: $(".layer-model"),
      tabs: [...this.querySelectorAll("[role=tab]")],
      keys: $(".layer-keys"),
      note: $(".layer-note"),
      msg: $(".layer-msg"),
      busy: $(".layer-busy"),
      storeButton: $(".vars-store"),
      mkdirButton: $(".vars-mkdir"),
      newRow: $(".vars-new"),
      fileInput: $(".vars-file"),
      empty: $(".layer-empty"),
      panes: Object.fromEntries(TABS.map((t) => [t, $(`.pane-${t}`)])),
      crumbs: $(".crumbs"),
      varsFind: $(".vars-find"),
      where: $(".vars-where"),
      tree: $(".tree"),
      list: $(".list"),
      listBody: $(".list tbody"),
      listEmpty: $(".list-empty"),
      varPreview: $(".pane-vars .preview"),
      levels: $(".levels"),
      stackPreview: $(".pane-stack .preview"),
      flagsFind: $(".flags-find"),
      flagsOnly: $(".flags-only"),
      flags: $(".flags-scroll"),
      title: $(".layer-title"),
      cmdsFind: $(".cmds-find"),
      cmdsCount: $(".cmds-count"),
      cmdsNote: $(".cmds-note"),
      cmdsMenus: $(".cmds-menus"),
      cmdsList: $(".cmds-list"),
      cmdsBody: $(".cmds-list tbody"),
      cmdsEmpty: $(".pane-commands .list-empty"),
      cmdsEntry: $(".cmds-entry"),
    };
    /** The Commands tab: the shown model's index (or why it failed, for that model only), the menu and command chosen, folded menus. */
    this.cmdWatch = reference ? new IndexWatch(reference) : null;
    this.cmdIndex = null;
    this.menuPath = null;
    this.cmdSelected = null;
    /** The placements by the manuals stay folded until asked for: the ROM's menus come first. */
    this.menuCollapsed = new Set([NOT_IN_MENU]);
    this.legends = null;
    this.tried = null;
    this.tab = TABS.includes(prefs.get("layerTab")) ? prefs.get("layerTab") : "vars";
    /** The directory shown, and whether it follows the calculator's. */
    this.browse = ["HOME"];
    this.follow = true;
    this.collapsed = new Set();
    /** The selected variable: `{path, name}`. */
    this.selected = null;
    /** The object shown for it: `{key, object?, error?}`. */
    this.loaded = null;
    this.level = 1;
    this.objects = new ObjectLoader((address) => memory.object(address), (state) => {
      if (!this.store.state.memoryTree) return;
      this.drawingFailure = state.error !== undefined;
      try { this.renderVars(); } finally { this.drawingFailure = false; }
    });
    this.drawingFailure = false;
    /** A rename or purge being asked about in the preview: `{path, name, mode, value}`. */
    this.editing = null;
    /** The name field of "New directory…", when open: `{value, error}`. */
    this.making = null;
    /** The directory just created, selected once the list shows it: `{path, name}`. */
    this.made = null;
    this.flagData = null;
    this.flagDataError = null;
    this.onlySet = false;

    $(".layer-close").addEventListener("click", () => this.close());
    $(".layer-back").addEventListener("click", () => this.close());
    for (const t of this.ui.tabs) {
      t.addEventListener("click", (e) => {
        this.setTab(t.dataset.tab);
        if (e.detail > 0) t.blur();
      });
    }
    $(".tabs").addEventListener("keydown", (e) => this.onTabKey(e));
    this.addEventListener("keydown", (e) => {
      if (e.key !== "Escape") return;
      // Escape first empties a search field, then gives the keys back.
      const t = e.target;
      if (t instanceof HTMLInputElement && t.value) {
        t.value = "";
        t.dispatchEvent(new Event("input", { bubbles: true }));
      } else if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur();
      }
      e.preventDefault();
    });
    this.addEventListener("focusin", () => this.showKeys());
    this.addEventListener("focusout", () => setTimeout(() => this.showKeys(), 0));

    this.ui.varsFind.addEventListener("input", () => this.renderVars());
    this.ui.crumbs.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-depth]");
      if (!b) return;
      this.go(this.browse.slice(0, Number(b.dataset.depth) + 1));
      if (e.detail > 0) b.blur();
    });
    this.ui.where.addEventListener("click", (e) => {
      const b = e.target.closest("button");
      if (!b) return;
      if (b.dataset.act === "cd") this.writes?.changeDir([...this.browse]);
      else this.go(this.currentPath());
      if (e.detail > 0) b.blur();
    });
    this.ui.storeButton.addEventListener("click", (e) => {
      if (e.detail > 0) this.ui.storeButton.blur();
      if (!this.writes) return;
      // The app asks for the file in its own dialog; the browser has the
      // page's file input.
      if (this.backend?.romSource === "dialog") this.writes.storeAsked([...this.browse]);
      else this.ui.fileInput.click();
    });
    this.ui.mkdirButton.addEventListener("click", (e) => {
      if (e.detail > 0) this.ui.mkdirButton.blur();
      if (!this.writes) return;
      this.making ??= { value: "", error: null };
      this.renderVars();
    });
    this.ui.fileInput.addEventListener("change", () => {
      const files = [...this.ui.fileInput.files];
      this.ui.fileInput.value = "";
      if (files.length) this.writes?.storeFiles([...this.browse], files);
    });
    this.dropTarget(this.ui.panes.vars);
    this.ui.flags.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-flag]");
      if (!b || !this.writes) return;
      if (e.detail > 0) b.blur();
      this.writes.setFlag(Number(b.dataset.flag), b.dataset.set !== "true");
    });
    // Rows are chosen by the mouse without taking the focus.
    for (const box of [this.ui.tree, this.ui.listBody, this.ui.levels]) {
      box.addEventListener("mousedown", (e) => e.preventDefault());
    }
    this.ui.tree.addEventListener("click", (e) => this.onTreeClick(e));
    this.ui.tree.addEventListener("keydown", (e) => this.onTreeKey(e));
    this.ui.listBody.addEventListener("click", (e) => this.onListClick(e));
    this.ui.listBody.addEventListener("dblclick", (e) => this.onListOpen(e));
    this.ui.listBody.addEventListener("keydown", (e) => this.onListKey(e));
    this.ui.levels.addEventListener("click", (e) => {
      const li = e.target.closest("li[data-level]");
      if (li) this.setLevel(Number(li.dataset.level));
    });
    this.ui.levels.addEventListener("keydown", (e) => this.onLevelKey(e));
    this.ui.flagsFind.addEventListener("input", () => this.renderFlags());
    this.ui.cmdsFind.addEventListener("input", () => this.renderCommands());
    this.ui.cmdsMenus.addEventListener("mousedown", (e) => e.preventDefault());
    this.ui.cmdsBody.addEventListener("mousedown", (e) => e.preventDefault());
    this.ui.cmdsMenus.addEventListener("click", (e) => this.onMenuClick(e));
    this.ui.cmdsMenus.addEventListener("keydown", (e) => this.onMenuKey(e));
    this.ui.cmdsBody.addEventListener("click", (e) => {
      const tr = e.target.closest("tr[data-name]");
      if (tr) this.selectCommand(tr.dataset.name);
    });
    this.ui.cmdsBody.addEventListener("keydown", (e) => this.onCommandKey(e));
    this.ui.flagsOnly.addEventListener("click", (e) => {
      this.onlySet = !this.onlySet;
      this.ui.flagsOnly.setAttribute("aria-pressed", String(this.onlySet));
      this.renderFlags();
      if (e.detail > 0) this.ui.flagsOnly.blur();
    });

    store.watch(
      ["layer", "booted", "model", "memorySupport", "memoryTree", "memoryStack", "memoryFlags", "memoryErrors", "memoryStale"],
      (s, changed) => {
        if (changed.has("booted")) this.reset();
        if (s.layer) this.render();
      },
    );
    store.watch(["writing", "writeMessage", "busy"], () => this.renderWriting());
    this.renderWriting();
    this.showTab();
    this.showKeys();
  }

  // ------------------------------------------------------------ state

  reset() {
    this.browse = ["HOME"];
    this.follow = true;
    this.collapsed.clear();
    this.selected = null;
    this.loaded = null;
    this.editing = null;
    this.making = null;
    this.made = null;
    this.objects.clear();
    this.level = 1;
    this.tried = null;
    this.cmdWatch?.reset();
    this.cmdIndex = null;
  }

  /** Show the Commands tab at the ROM menu `path` (`MTH BASE`), from the palette. */
  showMenu(path) {
    this.menuPath = path;
    this.cmdSelected = null;
    this.ui.cmdsFind.value = "";
    // Every menu above it is unfolded.
    for (const p of (this.cmdIndex && findMenu(this.cmdIndex.menus, path)?.above) ?? []) this.menuCollapsed.delete(p);
    this.setTab("commands");
  }

  close() {
    this.dispatchEvent(new CustomEvent("sat-layer", { bubbles: true, detail: false }));
  }

  /** Put the focus on the current tab (Alt+M). */
  focusIn() {
    this.ui.tabs.find((t) => t.dataset.tab === this.tab)?.focus();
  }

  hasFocus() {
    return this.contains(document.activeElement);
  }

  /** The indicator of where the keys go: short, the full sentence as its tooltip. */
  showKeys() {
    const inside = this.hasFocus();
    const key = this.bindings?.labelOf("layerFocus");
    this.ui.keys.classList.toggle("own", inside);
    this.ui.keys.querySelector(".layer-keys-text").textContent = inside ? "Keys: here" : "Keys: calculator";
    this.ui.keys.title = inside
      ? `Keys go to this panel. Esc${key ? ` or ${key}` : ""} gives them back.`
      : `Keys go to the calculator.${key ? ` ${key} moves them here.` : " Click into this panel to move them here."}`;
  }

  setTab(tab) {
    if (!TABS.includes(tab)) return;
    this.tab = tab;
    this.prefs.set("layerTab", tab);
    this.showTab();
    this.render();
  }

  showTab() {
    for (const t of this.ui.tabs) {
      const on = t.dataset.tab === this.tab;
      t.setAttribute("aria-selected", String(on));
      t.tabIndex = on ? 0 : -1;
    }
  }

  onTabKey(e) {
    const i = TABS.indexOf(this.tab);
    const next = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: TABS.length - 1 }[e.key];
    if (next === undefined) return;
    e.preventDefault();
    this.setTab(TABS[(next + TABS.length) % TABS.length]);
    this.focusIn();
  }

  currentPath() {
    return this.store.state.memoryTree?.path ?? ["HOME"];
  }

  /** Show the directory at `path` (page navigation; the calculator stays where it is). */
  go(path) {
    this.browse = [...path];
    this.follow = same(this.browse, this.currentPath());
    this.selected = null;
    if (this.making) this.making.error = null;
    this.ui.varsFind.value = "";
    this.renderVars();
  }

  select(path, name) {
    this.selected = { path: [...path], name };
    this.renderVars();
  }

  // ------------------------------------------------------------ render

  render() {
    const s = this.store.state;
    const ui = this.ui;
    const commands = this.tab === "commands";
    const shown = s.booted ?? s.model;
    ui.title.textContent = commands ? "Commands" : "Memory";
    ui.model.textContent = commands
      ? (shown ? `${MODEL_TITLES[shown] ?? shown} · reference` : "")
      : (s.booted ? `${MODEL_TITLES[s.booted] ?? s.booted}` : "");
    const empty = commands ? this.commandsEmptyState(s) : this.emptyState(s);
    ui.empty.hidden = !empty;
    if (empty) {
      ui.empty.querySelector("h3").textContent = empty.title;
      ui.empty.querySelector(".layer-empty-text").textContent = empty.text;
      ui.empty.querySelector(".layer-empty-detail").textContent = empty.detail ?? "";
    }
    for (const t of TABS) ui.panes[t].hidden = Boolean(empty) || t !== this.tab;
    ui.note.hidden = !s.memoryStale || Boolean(empty);
    ui.note.textContent = "The calculator is busy; this is its memory as it was. It follows when the calculator waits for a key.";
    const depth = s.memoryStack?.length;
    ui.tabs[1].querySelector(".tab-count").textContent = depth ? ` ${depth}` : "";
    const set = s.memoryFlags?.set?.length;
    ui.tabs[2].querySelector(".tab-count").textContent = set ? ` ${set}` : "";
    if (empty) return;
    if (this.tab === "vars") this.renderVars();
    else if (this.tab === "stack") this.renderStack();
    else if (this.tab === "flags") this.renderFlags();
    else this.renderCommands();
    this.renderWriting();
  }

  /** What the Commands tab says instead of its panes, or null. */
  commandsEmptyState(s) {
    const shown = s.booted ?? s.model;
    if (!this.reference) return { title: "No command reference", text: "This page was started without the reference data." };
    if (shown && !["48sx", "48gx", "49g"].includes(shown)) {
      return {
        title: `No command reference for the ${MODEL_TITLES[shown] ?? shown}`,
        text: "The reference covers the commands of the HP 48SX, 48GX and 49G, read from their ROMs.",
      };
    }
    const st = this.cmdWatch.state(shown);
    if (st.error) return { title: "The command reference could not be read", text: sentence(st.error), detail: "It is tried again when another model is shown or a ROM boots." };
    return null;
  }

  /** What the whole layer says instead of its tabs, or null. */
  emptyState(s) {
    if (!s.booted) {
      return {
        title: "No calculator is running",
        text: "Pick a model and a ROM file. The variables, the stack and the flags of a running HP 48SX, 48GX or 49G appear here and follow the calculator.",
      };
    }
    const sup = s.memorySupport;
    if (!sup) return { title: "Reading the calculator's memory…", text: "" };
    if (!sup.supported) {
      return {
        title: `No memory view for the ${MODEL_TITLES[s.booted] ?? s.booted}`,
        text: sentence(String(sup.reason ?? "").replace(/: no memory view$/, "")),
        detail: "The memory view reads the HOME directory, the stack and the flags of the HP 48SX, 48GX and 49G.",
      };
    }
    return null;
  }

  /** A pane's message for a part that could not be read. */
  readError(what, err) {
    const setup = /not set up|not plausible|not idle|no directory at HOME/i.test(err);
    return el("div", { class: "pane-error" },
      el("h3", { text: setup ? "The calculator's memory is not set up yet" : `Cannot read ${what}` }),
      el("p", { text: setup
        ? "Answer the calculator's prompt or let it finish starting; this view follows as soon as its memory can be read."
        : "This view shows nothing rather than something wrong. It tries again when the calculator's memory changes." }),
      el("p", { class: "detail", text: sentence(err) }));
  }

  // ---------------------------------------------------------- variables

  renderVars() {
    const s = this.store.state;
    const ui = this.ui;
    const split = this.querySelector(".vars-split");
    const err = s.memoryErrors.tree;
    const tree = s.memoryTree;
    for (const n of [split, ui.crumbs.parentElement, ui.where, ui.newRow]) n.hidden = !tree;
    this.querySelector(".pane-vars > .pane-error")?.remove();
    if (!tree) {
      ui.varPreview.replaceChildren();
      if (err) ui.panes.vars.prepend(this.readError("the variables", err));
      return;
    }
    const current = tree.path;
    if (this.follow) this.browse = [...current];
    // A directory that is gone: its nearest ancestor that is still there.
    while (this.browse.length > 1 && !directoryAt(tree.variables, this.browse)) this.browse.pop();
    const vars = directoryAt(tree.variables, this.browse) ?? [];
    const here = same(this.browse, current);

    ui.crumbs.replaceChildren(...this.browse.flatMap((name, i) => [
      ...(i ? [el("span", { class: "sep", "aria-hidden": "true", text: "›" })] : []),
      el("button", { type: "button", class: "crumb", "data-depth": i, "aria-current": i === this.browse.length - 1 ? "page" : null, text: name }),
    ]));
    ui.where.classList.toggle("here", here);
    ui.where.replaceChildren(...(here
      ? [el("span", { class: "dot", "aria-hidden": "true" }), "The calculator is in this directory."]
      : [el("span", { class: "dot", "aria-hidden": "true" }), `The calculator is in ${current.join(" › ")}. `,
        el("button", { type: "button", class: "link", text: "Show it" }), " · ",
        el("button", { type: "button", class: "link write", "data-act": "cd", text: "Change to this one",
          title: "Make the directory shown the calculator's current directory" })]));

    this.renderTree(tree.variables, current);

    const needle = ui.varsFind.value.trim();
    const rows = needle
      ? findVariables(tree.variables, needle)
      : vars.map((variable) => ({ variable, path: this.browse }));
    ui.list.classList.toggle("found", Boolean(needle));
    ui.list.querySelector("thead th").textContent = needle ? `Name (${rows.length} found)` : "Name";
    const made = this.made;
    if (made && rows.some((r) => r.variable.name === made.name && same(r.path, made.path))) {
      this.selected = { path: [...made.path], name: made.name };
      this.made = null;
    }
    if (this.selected && !rows.some((r) => r.variable.name === this.selected.name && same(r.path, this.selected.path))) {
      this.selected = null;
    }
    const focused = ui.listBody.contains(document.activeElement);
    ui.listBody.replaceChildren(...rows.map((r) => this.listRow(r, needle)));
    ui.listEmpty.hidden = rows.length > 0;
    ui.listEmpty.textContent = needle
      ? `No variable's name contains “${needle}”.`
      : `${this.browse.at(-1)} is empty.`;
    const rowEls = [...ui.listBody.children];
    const sel = rowEls.find((r) => r.getAttribute("aria-selected") === "true") ?? rowEls[0];
    if (sel) {
      sel.tabIndex = 0;
      if (focused) sel.focus();
    }
    this.renderVarPreview(tree, vars);
    ui.newRow.replaceChildren(...this.newDirRow(vars));
    this.ui.storeButton.disabled = this.writesOff();
    this.ui.mkdirButton.disabled = this.writesOff();
    for (const b of ui.where.querySelectorAll("button.write")) b.disabled = this.writesOff();
  }

  renderTree(variables, current) {
    const nodes = [];
    const walk = (vars, path, depth) => {
      const dirs = (vars ?? []).filter((v) => Array.isArray(v.variables));
      const key = path.join("/");
      const kids = path.length === 1 ? true : dirs.length > 0;
      const open = !this.collapsed.has(key);
      const shown = same(path, this.browse);
      const here = same(path, current);
      const row = el("div", {
        class: `node${shown ? " shown" : ""}${here ? " here" : ""}`,
        role: "treeitem",
        "aria-level": depth + 1,
        "aria-selected": String(shown),
        "aria-expanded": dirs.length ? String(open) : null,
        "data-path": JSON.stringify(path),
        style: `--d:${depth}`,
        tabindex: shown ? 0 : -1,
        title: here ? "The calculator's current directory" : null,
      },
      el("span", { class: `twist${dirs.length ? (open ? " open" : "") : " leaf"}`, "aria-hidden": "true" }, dirs.length ? iconEl("chevron-right") : null),
      el("span", { class: "name", text: path.at(-1) }),
      here ? el("span", { class: "here-mark", text: "current" }) : null);
      nodes.push(row);
      if (kids && open) for (const d of dirs) walk(d.variables, [...path, d.name], depth + 1);
    };
    walk(variables, ["HOME"], 0);
    const focused = this.ui.tree.contains(document.activeElement);
    this.ui.tree.replaceChildren(...nodes);
    if (focused) (this.ui.tree.querySelector(".shown") ?? nodes[0])?.focus();
  }

  listRow({ variable: v, path }, found) {
    const dir = Array.isArray(v.variables);
    const selected = this.selected?.name === v.name && same(this.selected.path, path);
    return el("tr", {
      class: dir ? "dir" : null,
      "aria-selected": String(selected),
      "data-name": v.name,
      "data-path": JSON.stringify(path),
      tabindex: -1,
    },
    el("td", { class: "name" },
      el("span", { class: `kind ${dir ? "kind-dir" : "kind-obj"}`, "aria-hidden": "true" }),
      el("span", { class: "obj-name", text: v.name }),
      found ? el("span", { class: "in", text: ` in ${path.join(" › ")}` }) : null),
    el("td", { class: "type", text: typeName(v.type) }),
    el("td", { class: "num", text: Number.isInteger(v.size) ? String(v.size) : v.size.toFixed(1) }),
    el("td", { class: "num sum", text: checksumText(v.checksum) }));
  }

  onTreeClick(e) {
    const node = e.target.closest(".node");
    if (!node) return;
    const path = JSON.parse(node.dataset.path);
    if (e.target.closest(".twist:not(.leaf)")) {
      const key = path.join("/");
      if (!this.collapsed.delete(key)) this.collapsed.add(key);
      this.renderVars();
      return;
    }
    this.go(path);
  }

  onTreeKey(e) {
    const nodes = [...this.ui.tree.querySelectorAll(".node")];
    const i = nodes.indexOf(document.activeElement);
    if (i < 0) return;
    const path = JSON.parse(nodes[i].dataset.path);
    const key = path.join("/");
    const expanded = nodes[i].getAttribute("aria-expanded");
    let to = null;
    switch (e.key) {
      case "ArrowDown": to = nodes[i + 1]; break;
      case "ArrowUp": to = nodes[i - 1]; break;
      case "Home": to = nodes[0]; break;
      case "End": to = nodes.at(-1); break;
      case "ArrowRight":
        if (expanded === "false") {
          this.collapsed.delete(key);
          this.renderVars();
        } else if (expanded === "true") to = nodes[i + 1];
        break;
      case "ArrowLeft":
        if (expanded === "true") {
          this.collapsed.add(key);
          this.renderVars();
        } else if (path.length > 1) {
          to = nodes.find((n) => n.dataset.path === JSON.stringify(path.slice(0, -1)));
        }
        break;
      case "Enter":
      case " ":
        this.go(path);
        break;
      default:
        return;
    }
    e.preventDefault();
    if (to) {
      // The directory under the focus is the one shown.
      this.go(JSON.parse(to.dataset.path));
    }
  }

  rowTarget(e) {
    const tr = e.target.closest("tr[data-name]");
    return tr ? { tr, name: tr.dataset.name, path: JSON.parse(tr.dataset.path) } : null;
  }

  onListClick(e) {
    const r = this.rowTarget(e);
    if (r) this.select(r.path, r.name);
  }

  onListOpen(e) {
    const r = this.rowTarget(e);
    if (r?.tr.classList.contains("dir")) this.go([...r.path, r.name]);
  }

  onListKey(e) {
    const rows = [...this.ui.listBody.children];
    const i = rows.indexOf(document.activeElement);
    if (i < 0) return;
    const pick = (row) => {
      if (!row) return;
      this.selected = { path: JSON.parse(row.dataset.path), name: row.dataset.name };
      this.renderVars();
      this.ui.listBody.querySelector('[aria-selected="true"]')?.focus();
    };
    switch (e.key) {
      case "ArrowDown": pick(rows[i + 1]); break;
      case "ArrowUp": pick(rows[i - 1]); break;
      case "Home": pick(rows[0]); break;
      case "End": pick(rows.at(-1)); break;
      case " ": pick(rows[i]); break;
      case "Enter":
        if (rows[i].classList.contains("dir")) {
          this.go([...JSON.parse(rows[i].dataset.path), rows[i].dataset.name]);
          this.ui.listBody.querySelector("tr")?.focus();
        } else pick(rows[i]);
        break;
      case "Backspace":
        if (this.browse.length > 1) {
          this.go(this.browse.slice(0, -1));
          this.ui.listBody.querySelector("tr")?.focus();
        }
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  renderVarPreview(tree, vars) {
    const box = this.ui.varPreview;
    const sel = this.selected;
    const v = sel ? directoryAt(tree.variables, sel.path)?.find((x) => x.name === sel.name) : null;
    if (!v) {
      this.loaded = null;
      const dirs = vars.filter((x) => Array.isArray(x.variables)).length;
      const n = vars.length;
      box.replaceChildren(el("div", { class: "preview-hint" },
        el("h3", { text: this.browse.join(" › ") }),
        el("p", { text: n
          ? `${n} ${n === 1 ? "variable" : "variables"}${dirs ? `, ${dirs} of them ${dirs === 1 ? "a directory" : "directories"}` : ""}, newest first, as the calculator lists them. Select one to see it.`
          : "No variables here yet. What the calculator stores appears within a second." })));
      return;
    }
    const meta = [typeName(v.type), sizeText(v.size), `checksum ${checksumText(v.checksum)}`];
    if (Array.isArray(v.variables)) {
      this.loaded = null;
      const open = el("button", { type: "button", text: "Open" });
      open.addEventListener("click", (e) => {
        this.go([...sel.path, v.name]);
        if (e.detail > 0) open.blur();
      });
      const cd = this.button("Make current", "Make it the calculator's current directory", () => this.writes.changeDir([...sel.path, v.name]));
      box.replaceChildren(this.previewHead(v.name, meta, this.actions(sel, v, open, cd)),
        ...this.editRow(sel, v),
        el("div", { class: "preview-body" }, v.variables.length
          ? el("ul", { class: "obj-dir" }, ...v.variables.map((c) => el("li", {},
            el("span", { class: "obj-name", text: c.name }), el("span", { class: "muted", text: typeName(c.type) }))))
          : el("p", { class: "muted", text: "An empty directory." })));
      return;
    }
    // The object is read again when the variable's content changed. A
    // failed read is not kept: it is tried again whenever this is drawn
    // for another reason than its own failure (the memory changed, the
    // variable was selected again).
    const key = `${v.address}:${v.checksum}:${v.size}`;
    this.loaded = this.objects.get(key, v.address, !this.drawingFailure);
    const save = this.button("Save as file", "Fetch it from the calculator into a file (HP binary)", () => this.writes.fetch([...sel.path], v.name));
    const [head, ...rest] = this.objectPreview(v.name, meta, this.loaded);
    // One row of buttons: Edit and Copy text (when the object has
    // arrived), then the writes.
    const copy = head.querySelector(".copy");
    const edit = this.editButton(this.loaded?.object, { kind: "variable", dir: [...sel.path], name: v.name });
    head.append(this.actions(sel, v, edit, copy, save));
    box.replaceChildren(head, ...this.editRow(sel, v), ...rest);
  }

  // ------------------------------------------------------------ writes

  /** Whether the write buttons are off: no writes, one running, or the calculator busy typing. */
  writesOff() {
    return !this.writes || Boolean(this.store.state.writing) || this.store.state.busy;
  }

  /**
   * The Edit button for `object` (once it has arrived) at `target`: opens
   * it in the palette's editor; disabled for an object without a text
   * form. Null without an editor or before the object is read.
   */
  editButton(object, target) {
    if (!this.edit || !object) return null;
    const textless = typeof object.text !== "string";
    const b = el("button", { type: "button", class: "write edit", disabled: textless || this.writesOff(),
      title: textless ? "This object has no text form to edit" : "Edit its text in the editor; saving compiles it on the calculator" });
    b.append(iconEl("edit", "ic-sm"), "Edit");
    if (textless) b.dataset.textless = "";
    b.addEventListener("click", (e) => {
      if (e.detail > 0) b.blur();
      this.edit(target);
    });
    return b;
  }

  /** A write's button: disabled while one runs. */
  button(text, title, run) {
    const b = el("button", { type: "button", class: "write", text, title, disabled: this.writesOff() });
    b.addEventListener("click", (e) => {
      if (e.detail > 0) b.blur();
      if (this.writes && !this.writes.busy()) run();
    });
    return b;
  }

  /** The preview's buttons for variable `v` at `sel`: `first` ones, then Rename and Purge. */
  actions(sel, v, ...first) {
    const ask = (mode) => () => {
      this.editing = { path: [...sel.path], name: v.name, mode, value: v.name };
      this.renderVars();
      this.querySelector(".edit-row input, .edit-row button.danger")?.focus();
    };
    const box = el("div", { class: "preview-actions" }, ...first,
      this.writes ? this.button("Rename", "Give it another name", ask("rename")) : null,
      this.writes ? this.button("Purge", Array.isArray(v.variables) ? "Delete the directory with everything in it" : "Delete it", ask("purge")) : null);
    return box;
  }

  /** The rename field or the purge question for `v`, when one is open. */
  editRow(sel, v) {
    const ed = this.editing;
    if (!ed || ed.name !== v.name || !same(ed.path, sel.path)) return [];
    const cancel = el("button", { type: "button", text: "Cancel" });
    cancel.addEventListener("click", () => {
      this.editing = null;
      this.renderVars();
    });
    if (ed.mode === "purge") {
      const n = v.variables?.length ?? 0;
      const go = this.button(`Purge ${v.name}`, "", () => {
        this.editing = null;
        this.writes.purge([...sel.path], v.name);
      });
      go.classList.add("danger");
      return [el("div", { class: "edit-row", role: "group", "aria-label": "Purge" },
        el("span", { text: Array.isArray(v.variables)
          ? `Purge the directory ${v.name}${n ? ` and the ${n} ${n === 1 ? "variable" : "variables"} in it` : ""}?`
          : `Purge ${v.name} from ${pathText(sel.path)}?` }),
        go, cancel)];
    }
    const input = el("input", { type: "text", value: ed.value, "aria-label": `New name for ${v.name}`, spellcheck: "false", autocomplete: "off" });
    input.addEventListener("input", () => { ed.value = input.value; });
    const ok = this.button("Rename", "", () => {
      const to = input.value.trim();
      this.editing = null;
      if (to && to !== v.name) this.writes.rename([...sel.path], v.name, to);
      else this.renderVars();
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") ok.click();
      else if (e.key === "Escape") {
        e.stopPropagation();
        cancel.click();
      }
    });
    queueMicrotask(() => {
      if (this.contains(input) && document.activeElement !== input && this.editing === ed) {
        input.focus();
        input.setSelectionRange(input.value.length, input.value.length);
      }
    });
    return [el("div", { class: "edit-row", role: "group", "aria-label": "Rename" }, input, ok, cancel)];
  }

  /** The name field of "New directory…", when it is open: it creates in the directory shown, whose variables are `vars`. */
  newDirRow(vars) {
    const mk = this.making;
    if (!mk || !this.writes) return [];
    const dir = [...this.browse];
    const input = el("input", { type: "text", value: mk.value, placeholder: "Name", "aria-label": `Name of the new directory in ${pathText(dir)}`,
      "aria-invalid": mk.error ? "true" : null, spellcheck: "false", autocomplete: "off" });
    input.addEventListener("input", () => { mk.value = input.value; });
    const cancel = el("button", { type: "button", text: "Cancel" });
    cancel.addEventListener("click", () => {
      this.making = null;
      this.renderVars();
    });
    const ok = this.button("Create", "", () => {
      const name = input.value.trim();
      mk.error = newDirectoryRefusal(name, vars, dir);
      if (mk.error) {
        this.renderVars();
        return;
      }
      this.making = null;
      this.made = { path: dir, name };
      this.writes.createDir(dir, name).then((r) => {
        if (r === null && this.made?.name === name) this.made = null;
      });
      this.renderVars();
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") ok.click();
      else if (e.key === "Escape") {
        e.stopPropagation();
        cancel.click();
      }
    });
    queueMicrotask(() => {
      if (this.contains(input) && document.activeElement !== input && this.making === mk) {
        input.focus();
        input.setSelectionRange(input.value.length, input.value.length);
      }
    });
    return [el("div", { class: "edit-row", role: "group", "aria-label": `New directory in ${pathText(dir)}` }, input, ok, cancel,
      mk.error ? el("span", { class: "edit-error", role: "alert", text: mk.error }) : null)];
  }

  /** Files dropped on `pane`: on a directory of the tree or the list, else on the directory shown. */
  dropTarget(pane) {
    const files = (e) => [...(e.dataTransfer?.types ?? [])].includes("Files");
    const target = (e) => {
      const node = e.target.closest?.(".node");
      if (node) return { el: node, dir: JSON.parse(node.dataset.path) };
      const row = e.target.closest?.("tr.dir[data-name]");
      if (row) return { el: row, dir: [...JSON.parse(row.dataset.path), row.dataset.name] };
      return { el: pane, dir: [...this.browse] };
    };
    const clear = () => {
      for (const n of this.querySelectorAll(".drop-over")) n.classList.remove("drop-over");
    };
    // Files dropped here are for the calculator, not ROMs for the page
    // (sat-controls takes drops on the rest of the page).
    pane.addEventListener("dragover", (e) => {
      if (!files(e) || !this.writes || !this.store.state.memoryTree) return;
      e.preventDefault();
      e.stopPropagation();
      document.body.classList.remove("rom-drop");
      e.dataTransfer.dropEffect = "copy";
      const t = target(e);
      if (!t.el.classList.contains("drop-over")) {
        clear();
        t.el.classList.add("drop-over");
        pane.dataset.dropInto = pathText(t.dir);
      }
    });
    pane.addEventListener("dragleave", (e) => {
      if (!pane.contains(e.relatedTarget)) clear();
    });
    pane.addEventListener("drop", (e) => {
      if (!files(e) || !this.writes || !this.store.state.memoryTree) return;
      e.preventDefault();
      e.stopPropagation();
      clear();
      const list = [...e.dataTransfer.files];
      if (list.length) this.writes.storeFiles(target(e).dir, list);
    });
  }

  /** The busy overlay and the last write's message. */
  renderWriting() {
    const s = this.store.state;
    const ui = this.ui;
    ui.busy.hidden = !s.writing;
    ui.busy.querySelector(".layer-busy-text").textContent = s.writing ?? "";
    this.classList.toggle("writing", Boolean(s.writing));
    const m = s.writeMessage;
    ui.msg.hidden = !m;
    ui.msg.textContent = m?.text ?? "";
    ui.msg.classList.toggle("error", Boolean(m?.error));
    const off = this.writesOff();
    for (const b of this.querySelectorAll("button.write")) b.disabled = off || b.dataset.textless !== undefined;
  }

  // ------------------------------------------------------------ previews

  /** `plain`: the title is the page's word (a stack level), not a calculator name. */
  previewHead(name, meta, action, plain = false) {
    return el("div", { class: "preview-head" },
      el("div", { class: "preview-title" },
        el("h3", { class: plain ? null : "obj-name", text: name }),
        el("p", { class: "preview-meta", text: meta.filter(Boolean).join(" · ") })),
      action);
  }

  copyButton(text) {
    const b = el("button", { type: "button", class: "copy", text: "Copy text", disabled: text === null,
      title: text === null ? "This object has no text form to copy" : "Copy the object's text form" });
    b.addEventListener("click", async (e) => {
      if (e.detail > 0) b.blur();
      try {
        await copyText(text);
        b.textContent = "Copied";
      } catch (err) {
        b.textContent = "Copy failed";
        b.title = String(err?.message ?? err);
      }
      setTimeout(() => { b.textContent = "Copy text"; }, 1600);
    });
    return b;
  }

  /** The nodes of a preview of `state.object` (or its error, or "reading"). */
  objectPreview(name, meta, state, plain = false) {
    if (state?.error) {
      return [this.previewHead(name, meta, null, plain), el("div", { class: "preview-body" },
        el("p", { class: "preview-error", text: "This object cannot be shown." }),
        el("p", { class: "detail", text: sentence(state.error) }))];
    }
    if (!state?.object) {
      return [this.previewHead(name, meta, null, plain), el("div", { class: "preview-body" }, el("p", { class: "muted", text: "Reading…" }))];
    }
    const p = previewOf(state.object);
    const head = this.previewHead(name, meta.length ? meta : [p.title], this.copyButton(p.copy), plain);
    return [head, el("div", { class: "preview-body" }, ...this.previewBody(p))];
  }

  previewBody(p) {
    const more = (n, what) => (n ? el("p", { class: "more", text: `… and ${n} more ${what}` }) : null);
    switch (p.kind) {
      case "text":
        return [el("pre", { class: "obj", text: p.text })];
      case "string":
        return [
          el("pre", { class: "obj obj-string", text: p.text }),
          el("p", { class: "more", text: `${p.length} ${p.length === 1 ? "character" : "characters"}${p.more ? `; the first ${p.text.length} are shown` : ""}` }),
        ];
      case "program":
        return [
          el("pre", { class: "obj obj-program" }, ...p.lines.map((l) => el("span", { class: "line", style: `--d:${l.depth}`, text: l.text }))),
          more(p.more, "lines"),
        ];
      case "list":
        return [
          p.count
            ? el("ol", { class: "obj-list" }, ...p.items.map((it) => el("li", { class: it.complete ? null : "partial" },
              el("span", { class: "obj", text: it.text }), el("span", { class: "muted", text: it.type }))))
            : el("p", { class: "muted", text: "An empty list." }),
          more(p.more, "elements"),
          p.copy === null && p.count
            ? el("p", { class: "more", text: "Elements shown in grey have no text form, so the list cannot be copied as text." })
            : null,
        ];
      case "matrix": {
        const body = el("tbody", {}, ...p.rows.map((row) => el("tr", {},
          ...row.map((c) => el("td", { text: c })), p.moreCols ? el("td", { class: "muted", text: "…" }) : null)));
        return [
          el("div", { class: "grid-wrap" }, el("table", { class: "obj-grid" }, body)),
          p.moreRows || p.moreCols
            ? el("p", { class: "more", text: `${p.dims.join(" × ")} in all; ${p.rows.length} ${p.rows.length === 1 ? "row" : "rows"} and ${p.rows[0]?.length ?? 0} columns are shown` })
            : null,
        ];
      }
      default: {
        const hex = p.hex
          ? el("details", { class: "nibbles" },
            el("summary", { text: `Its ${p.nibbles} nibbles as stored${p.truncated ? ` (the first ${p.hex.length})` : ""}` }),
            el("pre", { class: "obj hex", text: p.hex.replace(/(.{5})/g, "$1 ").trim() }))
          : null;
        return [el("p", { class: "unavailable", text: p.reason }), hex];
      }
    }
  }

  // -------------------------------------------------------------- stack

  setLevel(n) {
    this.level = n;
    this.renderStack();
  }

  onLevelKey(e) {
    const depth = this.store.state.memoryStack?.length ?? 0;
    // Level 1 is at the bottom, as on the calculator.
    const to = { ArrowUp: this.level + 1, ArrowDown: this.level - 1, Home: depth, End: 1 }[e.key];
    if (to === undefined) return;
    e.preventDefault();
    if (to < 1 || to > depth) return;
    this.setLevel(to);
    this.ui.levels.querySelector('[aria-selected="true"]')?.focus();
  }

  renderStack() {
    const s = this.store.state;
    const ui = this.ui;
    const pane = ui.panes.stack;
    pane.querySelector(":scope > .pane-error")?.remove();
    const levels = s.memoryStack;
    pane.querySelector(".levels-wrap").hidden = !levels;
    if (!levels) {
      ui.stackPreview.replaceChildren();
      if (s.memoryErrors.stack) pane.prepend(this.readError("the stack", s.memoryErrors.stack));
      return;
    }
    if (!levels.length) {
      ui.levels.replaceChildren();
      ui.stackPreview.replaceChildren(el("div", { class: "preview-hint" },
        el("h3", { text: "The stack is empty" }),
        el("p", { text: "What the calculator puts on its stack appears here within a second, level 1 at the bottom." })));
      return;
    }
    this.level = Math.min(Math.max(this.level, 1), levels.length);
    const focused = ui.levels.contains(document.activeElement);
    const items = [];
    for (let n = levels.length; n >= 1; n--) {
      const obj = levels[n - 1];
      const sum = summary(obj, 160);
      items.push(el("li", {
        role: "option",
        "data-level": n,
        "aria-selected": String(n === this.level),
        tabindex: n === this.level ? 0 : -1,
        class: sum.complete ? null : "partial",
      },
      el("span", { class: "level", text: `${n}:` }),
      el("span", { class: "obj", text: sum.text }),
      el("span", { class: "muted type", text: typeTitle(obj) })));
    }
    ui.levels.replaceChildren(...items);
    const sel = ui.levels.querySelector('[aria-selected="true"]');
    if (focused) sel?.focus();
    else sel?.scrollIntoView({ block: "nearest" });
    const [head, ...rest] = this.objectPreview(`Level ${this.level}`, [], { object: levels[this.level - 1] }, true);
    const edit = this.editButton(levels[this.level - 1], { kind: "level", level: this.level });
    if (edit) {
      // Edit beside Copy text.
      const copy = head.querySelector(".copy");
      head.append(el("div", { class: "preview-actions" }, edit, copy));
    }
    ui.stackPreview.replaceChildren(head, ...rest);
  }

  // -------------------------------------------------------------- flags

  async loadFlagData() {
    try {
      const res = await fetch(new URL("../flags.json", import.meta.url));
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      this.flagData = await res.json();
    } catch (err) {
      this.flagData = { topics: [], models: {} };
      this.flagDataError = String(err?.message ?? err);
    }
    if (this.store.state.layer && this.tab === "flags") this.renderFlags();
  }

  /** `inner` as a button that sets or clears `flag` (only the lamp without writes). */
  flagButton(flag, set, inner) {
    if (!this.writes) return inner;
    return el("button", { type: "button", class: "lamp-toggle write", "data-flag": flag, "data-set": String(set),
      disabled: this.writesOff(), title: `${set ? "Clear" : "Set"} flag ${flag}`, "aria-label": `Flag ${flag}, ${set ? "set" : "clear"}: ${set ? "clear" : "set"} it` }, inner);
  }

  /** A cell of a flag grid; a button that sets or clears it when writes are possible. */
  flagCell(flag, set, title) {
    const text = String(flag).replace("-", "−");
    if (!this.writes) return el("span", { class: `cell${set ? " on" : ""}`, title, text });
    return el("button", { type: "button", class: `cell write${set ? " on" : ""}`, "data-flag": flag, "data-set": String(set),
      disabled: this.writesOff(), title: `${title}; click to ${set ? "clear" : "set"} it`, text });
  }

  lamp(on, label) {
    return el("span", { class: `lamp${on ? " on" : ""}`, role: "img", "aria-label": label ?? (on ? "set" : "clear") });
  }

  renderFlags() {
    const s = this.store.state;
    const box = this.ui.flags;
    const flags = s.memoryFlags;
    this.querySelector(".flags-bar").hidden = !flags;
    if (!flags) {
      box.replaceChildren(s.memoryErrors.flags ? this.readError("the flags", s.memoryErrors.flags) : "");
      return;
    }
    if (!this.flagData) {
      this.flagLoading ??= this.loadFlagData();
      box.replaceChildren(el("p", { class: "muted", text: "Reading the flag meanings…" }));
      return;
    }
    const entry = this.flagData.models[s.booted];
    const { topics, undocumented } = flagRows(entry, this.flagData.topics, flags);
    const on = new Set(flags.set);
    const needle = this.ui.flagsFind.value.trim().toLowerCase();
    const match = (row) => {
      if (this.onlySet && !row.bits.some(Boolean)) return false;
      if (!needle) return true;
      return [row.label, row.name, row.now, row.other, row.field].some((t) => String(t ?? "").toLowerCase().includes(needle))
        || (/^-?\d+$/.test(needle) && Math.abs(Number(needle)) >= Math.abs(row.first) && Math.abs(Number(needle)) <= Math.abs(row.last));
    };
    const out = [];
    const sysCount = flags.system.length * 64;
    const userCount = flags.user.length * 64;
    const nSys = flags.set.filter((n) => n < 0).length;
    const nUser = flags.set.length - nSys;
    out.push(el("p", { class: "flags-sum" },
      el("strong", { text: String(nSys) }), ` of ${sysCount} system flags set · `,
      el("strong", { text: String(nUser) }), ` of ${userCount} user flags set`));
    if (entry?.basis) out.push(el("p", { class: "flags-basis", text: entry.basis }));
    if (this.writes) {
      out.push(el("p", { class: "flags-basis", text: "Click a lamp or a numbered cell to set or clear that flag; the calculator's own SF and CF do it, out of sight." }));
    }
    if (this.flagDataError) out.push(el("p", { class: "flags-basis", text: `The flag meanings could not be read (${this.flagDataError}); the states below are live.` }));
    const STATUS = {
      unknown: ["uncertain", "The guides do not establish this flag's meaning"],
      unused: ["unused", "Not used by this model"],
    };
    // A status most rows share is said once, above; only the others are tagged.
    const all = topics.flatMap((t) => t.rows);
    const common = Object.keys(STATUS).find((k) => all.filter((r) => r.status === k).length > all.length / 2);
    let shown = 0;
    for (const { topic, rows } of topics) {
      const mine = rows.filter(match);
      if (!mine.length) continue;
      shown += mine.length;
      out.push(el("section", { class: "flag-topic" },
        el("h3", { text: topic }),
        el("ul", { class: "flag-rows" }, ...mine.map((r) => {
          const st = r.status === common ? null : STATUS[r.status];
          const single = r.set !== null;
          const state = r.set === null ? null : el("span", { class: `state${r.set ? " on" : ""}`, text: r.set ? "set" : "clear" });
          return el("li", { class: `flag${r.bits.some(Boolean) ? " on" : ""}` },
            el("span", { class: "num", text: r.label.replaceAll("-", "−") }),
            el("span", { class: "lamps" }, single ? this.flagButton(r.first, r.set, this.lamp(r.set)) : null),
            el("div", { class: "what" },
              el("p", { class: "flag-name" }, r.name, state, st ? el("span", { class: "tag", title: st[1], text: st[0] }) : null),
              r.now ? el("p", { class: "now", text: r.now }) : null,
              r.other ? el("p", { class: "other", text: `${r.set ? "Clear" : "Set"}: ${r.other}` }) : null,
              single ? null : el("div", { class: "lamp-grid bits" }, ...r.bits.map((b, i) =>
                this.flagCell(r.first - i, b, `${r.first - i} ${b ? "set" : "clear"}`))),
              r.field ? el("p", { class: "now", text: r.field }) : null));
        }))));
    }
    const cells = (list) => el("div", { class: "lamp-grid" }, ...list.map(({ flag, set, title }) =>
      this.flagCell(flag, set, `${flag} ${set ? "set" : "clear"}${title ? `: ${title}` : ""}`)));
    const und = undocumented.filter((u) => (!this.onlySet || u.set) && (!needle || String(Math.abs(u.flag)) === needle.replace(/^-/, "")));
    if (und.length) {
      shown += und.length;
      out.push(el("section", { class: "flag-topic" },
        el("h3", { text: "Without a documented meaning" }),
        el("p", { class: "flags-basis", text: "System flags the guides leave unused or do not describe. A filled cell is set." }),
        cells(und.map((u) => ({ ...u, title: u.status === "unused" ? "not used" : "not documented" })))));
    }
    const users = [];
    for (let n = 1; n <= userCount; n++) users.push({ flag: n, set: on.has(n) });
    const usr = users.filter((u) => (!this.onlySet || u.set) && (!needle || String(u.flag) === needle));
    if (usr.length || (!needle && !this.onlySet)) {
      shown += usr.length;
      out.push(el("section", { class: "flag-topic" },
        el("h3", { text: "User flags" }),
        el("p", { class: "flags-basis", text: `Flags 1 to ${userCount} mean what your programs make them mean. A filled cell is set.` }),
        cells(usr)));
    }
    if (!shown) {
      out.push(el("p", { class: "muted", text: this.onlySet && !needle ? "No flag is set." : "No flag matches." }));
    }
    const top = box.scrollTop;
    box.replaceChildren(...out);
    box.scrollTop = top;
  }
}

customElements.define("sat-explorer", SatExplorer);

// ------------------------------------------------------------ commands

/** The line above the Commands tab's list for `menu`: what a heading or a numbered menu is; empty for a key's menu. */
function menuNote(menu) {
  if (menu.kind === "heading" && menu.name === OTHER_MENUS) {
    return "Menus built into the ROM that no key opens and no manual names; n MENU shows menu n on the calculator.";
  }
  if (menu.kind === "heading") return "Commands no ROM menu offers, grouped by where a manual puts them (or by us for browsing).";
  if (menu.kind === "placement") {
    return menu.name === "Keyboard"
      ? "Commands on a key rather than in a menu (named by a manual or by the keyboard's legends)."
      : `Commands no ROM menu offers; “${menu.name}” is where a manual puts them, or our own grouping for browsing.`;
  }
  const n = /^MENU (\d+)$/.exec(menu.name);
  if (n && menu.label) return `Built into the ROM; no key opens it. The manuals place most of its commands under ${menu.label}. ${n[1]} MENU shows it.`;
  if (n) return `Built into the ROM; no key opens it and no manual names it. ${n[1]} MENU shows it.`;
  return "";
}

Object.assign(SatExplorer.prototype, {
  /** The index of the shown model, loaded once per model (a failure is kept for that model only); renders when it arrives. */
  async loadCommands(model) {
    const [st, skin] = await Promise.all([this.cmdWatch.ensure(model), this.backend?.skin(model).catch(() => null) ?? null]);
    if ((this.store.state.booted ?? this.store.state.model) !== model) return;
    this.cmdIndex = st.index ?? null;
    this.legends = skin?.keys ?? null;
    if (this.store.state.layer && this.tab === "commands") this.render();
  },

  menus() {
    return flattenMenus(this.cmdIndex.menus);
  },

  currentMenu() {
    const all = this.menus();
    return all.find((m) => m.path === this.menuPath) ?? all[0] ?? null;
  },

  selectMenu(path) {
    this.menuPath = path;
    this.cmdSelected = null;
    this.renderCommands();
  },

  selectCommand(name) {
    this.cmdSelected = name;
    this.renderCommands();
  },

  renderCommands() {
    const s = this.store.state;
    const ui = this.ui;
    const model = s.booted ?? s.model;
    if (!this.cmdIndex || this.cmdIndex.model !== model) {
      if (this.cmdWatch.state(model).loading) this.loadCommands(model);
      ui.cmdsMenus.replaceChildren();
      ui.cmdsBody.replaceChildren();
      ui.cmdsEntry.replaceChildren(el("p", { class: "muted", style: "padding: 16px" , text: "Reading the command reference…" }));
      return;
    }
    const index = this.cmdIndex;
    const menu = this.currentMenu();
    this.menuPath = menu?.path ?? null;
    const needle = ui.cmdsFind.value.trim();
    ui.cmdsCount.textContent = `${index.commands.length} commands`;

    // The menu tree: the ROM's menus, then where the rest is placed.
    const nodes = [];
    const walk = (list, depth) => {
      for (const n of list) {
        const open = !this.menuCollapsed.has(n.path);
        const shown = n.path === this.menuPath;
        nodes.push(el("div", {
          class: `node${shown ? " shown" : ""}${n.kind === "menu" ? "" : ` other ${n.kind}`}`,
          role: "treeitem",
          "aria-level": depth + 1,
          "aria-selected": String(shown),
          "aria-expanded": n.children.length ? String(open) : null,
          "data-path": n.path,
          style: `--d:${depth}`,
          tabindex: shown ? 0 : -1,
          title: n.kind === "placement" ? `${n.commands.length} commands, placed by a manual or by us` : `${menuCommands(n).length} commands`,
        },
        el("span", { class: `twist${n.children.length ? (open ? " open" : "") : " leaf"}`, "aria-hidden": "true" }, n.children.length ? iconEl("chevron-right") : null),
        el("span", { class: "name", text: n.title ?? n.name })));
        if (open) walk(n.children, depth + 1);
      }
    };
    walk(index.menus, 0);
    const treeFocused = ui.cmdsMenus.contains(document.activeElement);
    ui.cmdsMenus.replaceChildren(...nodes);
    const shownNode = ui.cmdsMenus.querySelector(".shown");
    if (treeFocused) (shownNode ?? nodes[0])?.focus();
    else shownNode?.scrollIntoView({ block: "nearest" });
    const note = menu ? menuNote(menu) : "";
    ui.cmdsNote.hidden = !note;
    ui.cmdsNote.textContent = note;

    // The commands: of the menu and its submenus, or of the whole model
    // when searching, ranked as the palette ranks them.
    const rows = needle ? findCommands(index, needle) : (menu ? menuCommands(menu) : []);
    ui.cmdsList.classList.toggle("found", Boolean(needle));
    ui.cmdsList.querySelector("thead th").textContent = needle ? `Name (${rows.length} found)` : "Name";
    if (this.cmdSelected && !rows.some((c) => c.name === this.cmdSelected)) this.cmdSelected = null;
    const listFocused = ui.cmdsBody.contains(document.activeElement);
    ui.cmdsBody.replaceChildren(...rows.map((c) => el("tr", {
      "aria-selected": String(c.name === this.cmdSelected),
      "data-name": c.name,
      tabindex: -1,
    },
    el("td", { class: "name" }, el("span", { class: "obj-name", text: c.name })),
    el("td", { class: "stack", title: c.stack, text: c.stack }))));
    ui.cmdsEmpty.hidden = rows.length > 0;
    ui.cmdsEmpty.textContent = needle ? `No command matches “${needle}”.` : "No commands here.";
    const rowEls = [...ui.cmdsBody.children];
    const sel = rowEls.find((r) => r.getAttribute("aria-selected") === "true") ?? rowEls[0];
    if (sel) {
      sel.tabIndex = 0;
      if (listFocused) sel.focus();
    }

    // The entry.
    const command = this.cmdSelected ? index.byName.get(this.cmdSelected) : null;
    if (!command) {
      ui.cmdsEntry.replaceChildren(el("div", { class: "preview-hint" },
        el("h3", { text: needle ? `${rows.length} ${rows.length === 1 ? "command matches" : "commands match"}` : (menu?.kind === "menu" && menu.path.startsWith("MENU ") ? menu.title : menu?.path ?? "Commands") }),
        el("p", { text: needle
          ? "Names first, then descriptions, as the palette ranks them. Select one to see its entry."
          : `${rows.length} ${rows.length === 1 ? "command" : "commands"}${menu?.children.length ? ` in this menu and its ${menu.children.length} submenus` : ""}, as the ROM lists them. Select one to see its entry: stack effect, description, examples run on the emulator, the manual pages.` })));
      return;
    }
    const canType = Boolean(s.booted) && s.booted === model && this.backend;
    const top = ui.cmdsEntry.scrollTop;
    ui.cmdsEntry.replaceChildren(entryView(index, command, {
      legends: this.legends,
      onTry: canType ? (x) => this.tryExample(x) : null,
      whyNot: canType ? null : "Start this calculator to try an example",
      tried: this.tried,
    }));
    ui.cmdsEntry.scrollTop = top;
  },

  /** "Try it" from the tab: the example's text, run; the outcome shows under it. */
  async tryExample(example) {
    this.tried = { example, text: "Sending…", error: false };
    this.renderCommands();
    try {
      const r = await this.backend.run(exampleText(example));
      this.tried = r.error
        ? { example, text: `The calculator says: ${r.error}`, error: true }
        : { example, text: r.running ? "Sent; the calculator is still working on it." : "Sent; the calculator shows the result.", error: false };
    } catch (err) {
      this.tried = { example, text: String(err?.message ?? err), error: true };
    }
    this.renderCommands();
  },

  onMenuClick(e) {
    const node = e.target.closest(".node");
    if (!node) return;
    const path = node.dataset.path;
    if (e.target.closest(".twist:not(.leaf)")) {
      if (!this.menuCollapsed.delete(path)) this.menuCollapsed.add(path);
      this.renderCommands();
      return;
    }
    this.selectMenu(path);
  },

  onMenuKey(e) {
    const nodes = [...this.ui.cmdsMenus.querySelectorAll(".node")];
    const i = nodes.indexOf(document.activeElement);
    if (i < 0) return;
    const path = nodes[i].dataset.path;
    const expanded = nodes[i].getAttribute("aria-expanded");
    let to = null;
    switch (e.key) {
      case "ArrowDown": to = nodes[i + 1]; break;
      case "ArrowUp": to = nodes[i - 1]; break;
      case "Home": to = nodes[0]; break;
      case "End": to = nodes.at(-1); break;
      case "ArrowRight":
        if (expanded === "false") {
          this.menuCollapsed.delete(path);
          this.renderCommands();
        } else if (expanded === "true") to = nodes[i + 1];
        break;
      case "ArrowLeft":
        if (expanded === "true") {
          this.menuCollapsed.add(path);
          this.renderCommands();
        } else {
          const parent = findMenu(this.cmdIndex.menus, path)?.above.at(-1);
          to = nodes.find((n) => n.dataset.path === parent) ?? null;
        }
        break;
      case "Enter":
      case " ":
        this.selectMenu(path);
        break;
      default:
        return;
    }
    e.preventDefault();
    if (to) {
      this.selectMenu(to.dataset.path);
      this.ui.cmdsMenus.querySelector(".shown")?.focus();
    }
  },

  onCommandKey(e) {
    const rows = [...this.ui.cmdsBody.children];
    const i = rows.indexOf(document.activeElement);
    if (i < 0) return;
    const pick = (row) => {
      if (!row) return;
      this.cmdSelected = row.dataset.name;
      this.renderCommands();
      this.ui.cmdsBody.querySelector('[aria-selected="true"]')?.focus();
    };
    switch (e.key) {
      case "ArrowDown": pick(rows[i + 1]); break;
      case "ArrowUp": pick(rows[i - 1]); break;
      case "Home": pick(rows[0]); break;
      case "End": pick(rows.at(-1)); break;
      case " ":
      case "Enter": pick(rows[i]); break;
      default:
        return;
    }
    e.preventDefault();
  },
});
