// <sat-explorer>: the memory view, a layer beside the calculator. Three
// tabs on the calculator's user memory, read live from RAM and never
// written: Variables (the HOME tree, the variables of a directory, a
// typed preview), Stack and Flags. Renders from the store; reads go
// through `MemoryView` (memory.js). Browsing directories here is
// navigation in the page; the calculator's own current directory is
// marked as such. Light DOM.
//
// Keyboard: the calculator keeps the keys unless the focus is inside this
// element (sat-calculator.js leaves those events alone). A mouse click on
// a row or a tab does not take the focus; Tab, the search fields and
// Alt+M (app.js) do, a strip says so, and Escape gives the keys back.

import { MODEL_TITLES } from "./sat-calculator.js";
import { ObjectLoader } from "../memory.js";
import {
  checksumText, directoryAt, findVariables, flagRows, previewOf, sizeText, summary, typeTitle,
} from "../objects.js";

const TEMPLATE = `
  <section class="layer" aria-label="Memory view">
    <header class="layer-head">
      <button type="button" class="layer-back" title="Back to the calculator">‹ Calculator</button>
      <h2 class="layer-title">Memory</h2>
      <span class="layer-model"></span>
      <button type="button" class="icon layer-close" title="Close the memory view" aria-label="Close the memory view">✕</button>
    </header>
    <div class="layer-tabs">
      <div class="tabs" role="tablist" aria-label="Memory view">
        <button type="button" role="tab" data-tab="vars" id="tab-vars" aria-controls="pane-vars">Variables</button>
        <button type="button" role="tab" data-tab="stack" id="tab-stack" aria-controls="pane-stack">Stack<span class="tab-count"></span></button>
        <button type="button" role="tab" data-tab="flags" id="tab-flags" aria-controls="pane-flags">Flags<span class="tab-count"></span></button>
      </div>
      <p class="layer-keys" aria-live="polite"></p>
    </div>
    <p class="layer-note" role="status" hidden></p>
    <div class="layer-empty" hidden>
      <h3></h3>
      <p class="layer-empty-text"></p>
      <p class="layer-empty-detail"></p>
    </div>

    <div class="pane pane-vars" id="pane-vars" role="tabpanel" aria-labelledby="tab-vars">
      <div class="vars-bar">
        <nav class="crumbs" aria-label="Directory shown"></nav>
        <input class="find vars-find" type="search" placeholder="Find a variable" aria-label="Find a variable in all directories" autocomplete="off" spellcheck="false">
      </div>
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
  </section>`;

const TABS = ["vars", "stack", "flags"];

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
  attach(memory, store, prefs) {
    this.memory = memory;
    this.store = store;
    this.prefs = prefs;
    this.innerHTML = TEMPLATE;
    const $ = (sel) => this.querySelector(sel);
    this.ui = {
      model: $(".layer-model"),
      tabs: [...this.querySelectorAll("[role=tab]")],
      keys: $(".layer-keys"),
      note: $(".layer-note"),
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
    };
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
      this.go(this.currentPath());
      if (e.detail > 0) b.blur();
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
    this.ui.flagsOnly.addEventListener("click", (e) => {
      this.onlySet = !this.onlySet;
      this.ui.flagsOnly.setAttribute("aria-pressed", String(this.onlySet));
      this.renderFlags();
      if (e.detail > 0) this.ui.flagsOnly.blur();
    });

    store.watch(
      ["layer", "booted", "memorySupport", "memoryTree", "memoryStack", "memoryFlags", "memoryErrors", "memoryStale"],
      (s, changed) => {
        if (changed.has("booted")) this.reset();
        if (s.layer) this.render();
      },
    );
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
    this.objects.clear();
    this.level = 1;
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

  showKeys() {
    const inside = this.hasFocus();
    this.ui.keys.classList.toggle("own", inside);
    this.ui.keys.replaceChildren(
      ...(inside
        ? ["Keys go to this panel. ", el("kbd", { text: "Esc" }), " gives them back."]
        : ["Keys go to the calculator. ", el("kbd", { text: "Alt" }), "+", el("kbd", { text: "M" }), " moves them here."]),
    );
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
    ui.model.textContent = s.booted ? `${MODEL_TITLES[s.booted] ?? s.booted} · read-only` : "";
    const empty = this.emptyState(s);
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
    else this.renderFlags();
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
    for (const n of [split, ui.crumbs.parentElement, ui.where]) n.hidden = !tree;
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
        el("button", { type: "button", class: "link", text: "Show it" })]));

    this.renderTree(tree.variables, current);

    const needle = ui.varsFind.value.trim();
    const rows = needle
      ? findVariables(tree.variables, needle)
      : vars.map((variable) => ({ variable, path: this.browse }));
    ui.list.classList.toggle("found", Boolean(needle));
    ui.list.querySelector("thead th").textContent = needle ? `Name (${rows.length} found)` : "Name";
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
      el("span", { class: `twist${dirs.length ? "" : " leaf"}`, "aria-hidden": "true", text: dirs.length ? (open ? "▾" : "▸") : "" }),
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
      box.replaceChildren(this.previewHead(v.name, meta, open),
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
    box.replaceChildren(...this.objectPreview(v.name, meta, this.loaded));
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
    ui.stackPreview.replaceChildren(...this.objectPreview(`Level ${this.level}`, [], { object: levels[this.level - 1] }, true));
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
            el("span", { class: "lamps" }, single ? this.lamp(r.set) : null),
            el("div", { class: "what" },
              el("p", { class: "flag-name" }, r.name, state, st ? el("span", { class: "tag", title: st[1], text: st[0] }) : null),
              r.now ? el("p", { class: "now", text: r.now }) : null,
              r.other ? el("p", { class: "other", text: `${r.set ? "Clear" : "Set"}: ${r.other}` }) : null,
              single ? null : el("div", { class: "lamp-grid bits" }, ...r.bits.map((b, i) =>
                el("span", { class: `cell${b ? " on" : ""}`, title: `${r.first - i} ${b ? "set" : "clear"}`, text: String(r.first - i).replace("-", "−") }))),
              r.field ? el("p", { class: "now", text: r.field }) : null));
        }))));
    }
    const cells = (list) => el("div", { class: "lamp-grid" }, ...list.map(({ flag, set, title }) =>
      el("span", { class: `cell${set ? " on" : ""}`, title: `${flag} ${set ? "set" : "clear"}${title ? `: ${title}` : ""}`,
        text: String(flag).replace("-", "−") })));
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
