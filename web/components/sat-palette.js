// <sat-palette>: the command palette, opened with its key (Cmd/Ctrl+K
// unless rebound) or its
// button. One input over the calculator: it suggests the model's
// commands (the reference), the user's variables and the app's actions
// while typing, shows the selected entry beside the list, and sends the
// choice to the calculator through the protocol's typing verbs
// (`PaletteModel`, web/palette.js). A modal <dialog>: while it is open
// the keys are its own (sat-calculator.js leaves events inside a
// dialog alone) and it gives them back on close. Light DOM.
//
// Editor mode: the input grows to a multi-line RPL editor
// (components/rpl-editor.js, web/editor.js) on Shift+Enter, on Enter
// with a delimiter still open, on a pasted line break, or when text is
// pulled in to edit (`openEditor`): the calculator's command line, a
// variable or a stack level. It sends the text back with `saveEdit`
// (Save, Cmd/Ctrl+S or Cmd/Ctrl+Enter) and closes once that went
// through; it shows the calculator's error and keeps the text when it
// does not compile, and keeps a history of sent text (Alt+↑/↓).

import { comboLabel, numberDigit } from "../bindings.js";
import { EditSession, FORMAT_KEY, History, afterSave, editorKey, format, fromDigraphs, indentAfter, pullEdit, saveSession, sentence, targetTitle, unclosed } from "../editor.js";
import { PaletteModel, stop } from "../palette.js";
import { menuCommands } from "../reference.js";
import { el, entryView } from "./entry-view.js";
import { icon, iconEl } from "./icons.js";
import { RplEditor } from "./rpl-editor.js";
import { MODEL_TITLES } from "./sat-calculator.js";

/* Below this width the palette is a phone sheet: the list alone under
   the input, the entry as a second step (style.css, "Narrow screens"). */
const NARROW = "(max-width: 759px)";

const TEMPLATE = `
  <dialog class="palette" aria-label="Search">
    <div class="palette-box">
      <div class="palette-input">
        <span class="palette-glyph" aria-hidden="true">${icon("chevron-right")}</span>
        <input type="text" aria-label="Command, variable, action, or text to send" placeholder="Command, variable, action, or text to send" autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" enterkeyhint="go">
        <span class="palette-model"></span>
        <kbd class="palette-key" title="Opens and closes this palette"></kbd>
        <kbd class="palette-esc">esc</kbd>
        <button type="button" class="icon palette-close" title="Close" aria-label="Close">${icon("close")}</button>
      </div>
      <div class="palette-editor" hidden>
        <div class="editor-head">
          <span class="palette-glyph" aria-hidden="true">${icon("edit")}</span>
          <span class="editor-title"></span>
          <span class="editor-dirty" hidden title="Changed since it was opened or last saved">unsaved</span>
          <kbd class="palette-esc">esc</kbd>
          <button type="button" class="icon editor-close" title="Close" aria-label="Close">${icon("close")}</button>
        </div>
        <div class="editor-area"></div>
        <div class="editor-bar">
          <p class="editor-hints"></p>
          <div class="editor-buttons">
            <button type="button" data-ed="format">Format</button>
            <button type="button" data-ed="secondary"></button>
            <button type="button" data-ed="primary" class="primary"></button>
          </div>
        </div>
      </div>
      <p class="palette-state" hidden></p>
      <div class="palette-body">
        <div class="palette-list" role="listbox" aria-label="Suggestions"></div>
        <div class="palette-detail"></div>
      </div>
      <p class="palette-notice" role="status" hidden></p>
      <div class="palette-foot">
        <p class="palette-hints"></p>
      </div>
    </div>
  </dialog>`;

const KIND_LABELS = { command: "command", variable: "variable", action: "action", menu: "menu", send: "send" };

export class SatPalette extends HTMLElement {
  /**
   * `reference` is a `ReferenceLoader`; `actions` the app's
   * `[{id, title, description?, keywords?, run()}]`; `onMenu(menu)` opens
   * a menu in the explorer's Commands tab; `bindings` (web/bindings.js)
   * gives its key and the row numbers' modifier.
   */
  attach(backend, store, { reference, actions = [], onMenu = null, bindings }) {
    this.backend = backend;
    this.bindings = bindings;
    this.store = store;
    this.reference = reference;
    this.onMenu = onMenu;
    /** The actions, or a function giving them (titles follow the state). */
    this.actions = actions;
    this.innerHTML = TEMPLATE;
    const $ = (sel) => this.querySelector(sel);
    this.ui = {
      dialog: $("dialog"),
      input: $("input"),
      model: $(".palette-model"),
      state: $(".palette-state"),
      list: $(".palette-list"),
      detail: $(".palette-detail"),
      notice: $(".palette-notice"),
      hints: $(".palette-hints"),
      box: $(".palette-box"),
      inputRow: $(".palette-input"),
      editor: $(".palette-editor"),
      editorTitle: $(".editor-title"),
      editorDirty: $(".editor-dirty"),
      editorHints: $(".editor-hints"),
      format: $('[data-ed="format"]'),
      primary: $('[data-ed="primary"]'),
      secondary: $('[data-ed="secondary"]'),
    };
    this.model = new PaletteModel(backend, store, { actions: typeof actions === "function" ? [] : actions });
    this.model.onChange = () => this.render();
    /** The skin's keys of the shown model, for the keyboard placement. */
    this.legends = null;
    this.legendsModel = null;
    /** The last "try it" and its outcome, shown under the example. */
    this.tried = null;
    this.isMac = bindings.isMac;
    this.renderedModel = null;
    this.lastSearchMs = 0;
    /** The phone sheet: a tapped row opens its entry as a second step. */
    this.narrow = window.matchMedia(NARROW);
    this.detailOpen = false;
    this.narrow.addEventListener("change", () => {
      if (!this.narrow.matches) this.detailOpen = false;
      this.render();
    });
    // The sheet's height follows the visual viewport, so the on-screen
    // keyboard shortens the list instead of covering it.
    this.onViewport = () => this.fitViewport();

    this.ui.input.addEventListener("input", () => this.model.setQuery(this.ui.input.value));
    // A pasted line break: the text goes to the editor whole (an input
    // would make it a space).
    this.ui.input.addEventListener("paste", (e) => {
      const text = e.clipboardData?.getData("text/plain") ?? "";
      if (!/[\r\n]/.test(text)) return;
      e.preventDefault();
      const v = this.ui.input.value;
      const at = this.ui.input.selectionStart ?? v.length;
      this.enterEditor(v.slice(0, at) + text.replace(/\r\n?/g, "\n") + v.slice(this.ui.input.selectionEnd ?? at));
    });

    /** The editor mode: what is edited (`EditSession`), or null in search mode. */
    this.session = null;
    /** A close was asked for with unsaved changes: the next one discards them. */
    this.discarding = false;
    /** A save is on its way. */
    this.saving = false;
    /** Counts opens; `closed` is the last one whose close was handled (a late `close` event of an earlier one is not). */
    this.opens = 0;
    this.closed = 0;
    /** Gives the element the focus goes back to when the editor closes (`openEditor`'s `returnFocus`), or null. */
    this.returnFocus = null;
    let storage = null;
    try { storage = window.localStorage; } catch { /* storage blocked: no history */ }
    this.history = new History(storage);
    this.contextCache = { index: null, vars: null, value: null };
    this.editor = new RplEditor($(".editor-area"), {
      context: () => this.highlightContext(),
      index: () => this.model.index,
      variables: () => this.model.variables,
      onChange: (text) => this.onEdit(text),
      onKey: (e) => this.onEditorKey(e),
    });
    $(".editor-close").addEventListener("click", () => this.close());
    $(".editor-bar").addEventListener("click", (e) => {
      const b = e.target.closest("button[data-ed]");
      if (!b || b.disabled) return;
      if (b.dataset.ed === "format") this.formatAll();
      else this.editorAction(b.dataset.ed);
    });
    $(".palette-close").addEventListener("click", () => this.close());
    this.ui.dialog.addEventListener("keydown", (e) => this.onKey(e));
    this.ui.dialog.addEventListener("cancel", (e) => {
      e.preventDefault();
      this.close();
    });
    this.ui.dialog.addEventListener("click", (e) => {
      if (e.target === this.ui.dialog) this.close();
    });
    this.ui.dialog.addEventListener("close", () => this.afterClose());
    // Escape closes even when nothing inside the dialog has the focus
    // (after a click on a button, which does not keep it).
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && this.isOpen() && !this.ui.dialog.contains(e.target)) {
        e.preventDefault();
        this.close();
      }
    });
    // Rows: the pointer selects, a click chooses; neither takes the focus from the input.
    this.ui.list.addEventListener("mousedown", (e) => e.preventDefault());
    this.ui.list.addEventListener("mousemove", (e) => {
      const row = e.target.closest("[data-row]");
      if (row && Number(row.dataset.row) !== this.model.selected) this.model.select(Number(row.dataset.row));
    });
    this.ui.list.addEventListener("click", (e) => {
      const row = e.target.closest("[data-row]");
      if (!row) return;
      if (this.narrow.matches) this.openDetail(Number(row.dataset.row));
      else this.choose(this.model.rows[Number(row.dataset.row)], e.metaKey || e.ctrlKey);
    });
    this.ui.detail.addEventListener("click", (e) => {
      const b = e.target.closest("button[data-palette]");
      if (!b) return;
      if (b.dataset.palette === "back") this.closeDetail();
      else this.choose(this.model.row, b.dataset.palette === "opposite");
    });

    store.watch(["booted", "model"], () => {
      if (this.isOpen()) this.loadIndex();
    });
    store.watch(["memoryTree"], (s) => {
      if (this.isOpen() && s.memoryTree) this.model.setVariables(s.memoryTree);
    });
    store.watch(["busy", "writing"], () => this.renderFoot());
  }

  isOpen() {
    return this.ui.dialog.open;
  }

  /** The model whose reference is shown: the running one, else the selected one. */
  shownModel() {
    return this.store.state.booted ?? this.store.state.model;
  }

  toggle() {
    if (this.isOpen()) this.close();
    else this.open();
  }

  async open(query = "") {
    if (this.isOpen()) return;
    this.tried = null;
    this.detailOpen = false;
    this.ui.input.value = query;
    this.opens++;
    // The palette's own key beside esc: opened by a click, it teaches it.
    const key = this.bindings?.labelOf("palette") ?? "";
    const chip = this.querySelector(".palette-key");
    chip.textContent = key;
    chip.hidden = !key;
    this.ui.dialog.showModal();
    this.ui.input.focus();
    this.fitViewport();
    window.visualViewport?.addEventListener("resize", this.onViewport);
    window.visualViewport?.addEventListener("scroll", this.onViewport);
    this.renderState();
    const opened = this.model.open().then(() => this.setActions());
    this.loadIndex();
    this.loadVariables();
    await opened;
    if (query) this.model.setQuery(query);
    this.renderState();
  }

  /** The app's actions, and on a 49G in algebraic mode the switch to RPN first. */
  setActions() {
    const actions = typeof this.actions === "function" ? this.actions() : this.actions;
    const m = this.model;
    if (m.algebraic && m.canType) {
      actions.unshift({
        id: "rpn",
        title: "Switch the HP 49G to RPN mode",
        description: "Sets the 49G to RPN mode, so the commands and examples here work.",
        keywords: "rpn algebraic mode flag -95 cf",
        run: async () => {
          await m.send("run", "CF(-95)", { closeAfter: false });
          this.renderState();
        },
      });
    }
    m.setActions(actions);
    this.renderState();
  }

  close() {
    if (!this.isOpen()) return;
    if (this.session?.dirty && !this.discarding) {
      // Unsaved changes: say so; the next close discards them.
      this.discarding = true;
      const mod = this.isMac ? "⌘" : "Ctrl+";
      this.model.notice = { text: `${sentence(targetTitle(this.session.target))} has unsaved changes. ${mod}S saves them; Esc again discards them.`, error: true };
      this.renderFoot();
      this.editor.focus();
      return;
    }
    this.ui.dialog.close();
  }

  /**
   * The keys go back to the calculator: nothing in the page keeps the
   * focus, unless the editor was opened from a control that had it (the
   * memory view's Edit, by keyboard), which gets it back.
   */
  afterClose() {
    // A `close` event of an earlier open that arrives late: handled already, or the dialog is open again.
    if (this.isOpen() || this.closed === this.opens) return;
    this.closed = this.opens;
    const back = this.returnFocus?.();
    this.returnFocus = null;
    this.leaveEditor();
    window.visualViewport?.removeEventListener("resize", this.onViewport);
    window.visualViewport?.removeEventListener("scroll", this.onViewport);
    this.ui.dialog.style.removeProperty("--vvh");
    this.ui.dialog.style.removeProperty("--vvt");
    if (back instanceof HTMLElement && back.isConnected) back.focus({ preventScroll: true });
    if (document.activeElement !== back && document.activeElement instanceof HTMLElement) document.activeElement.blur();
    this.dispatchEvent(new CustomEvent("sat-palette-closed", { bubbles: true }));
  }

  /** The visual viewport's height and offset as `--vvh` and `--vvt` on the sheet (style.css uses them below 760 px). */
  fitViewport() {
    const vv = window.visualViewport;
    if (!vv || !this.isOpen()) return;
    this.ui.dialog.style.setProperty("--vvh", `${Math.round(vv.height)}px`);
    this.ui.dialog.style.setProperty("--vvt", `${Math.round(vv.offsetTop)}px`);
  }

  /** The phone sheet's second step: row `i`'s entry instead of the list. */
  openDetail(i) {
    this.model.select(i);
    this.detailOpen = true;
    // The keyboard goes away so the entry can be read.
    this.ui.input.blur();
    this.render();
    this.ui.detail.scrollTop = 0;
    this.ui.detail.querySelector("button[data-palette=back]")?.focus({ preventScroll: true });
  }

  closeDetail() {
    this.detailOpen = false;
    this.render();
    // The back button is gone: focus stays in the dialog so its keys
    // still work. On touch the dialog itself, not raising the keyboard.
    if (matchMedia("(pointer: coarse)").matches) {
      this.ui.dialog.tabIndex = -1;
      this.ui.dialog.focus({ preventScroll: true });
    } else {
      this.ui.input.focus({ preventScroll: true });
    }
  }

  async loadIndex() {
    const model = this.shownModel();
    if (this.renderedModel === model && this.model.index) return;
    this.renderedModel = model;
    try {
      const [index, skin] = await Promise.all([this.reference.index(model), this.backend.skin(model).catch(() => null)]);
      if (this.shownModel() !== model) return;
      this.legends = skin?.keys ?? null;
      this.legendsModel = model;
      this.model.setIndex(index);
    } catch (err) {
      if (this.shownModel() !== model) return;
      this.model.setLoadError(err);
    }
    this.renderState();
  }

  /** The variables of the current directory and its parents, read once per opening. */
  async loadVariables() {
    const s = this.store.state;
    if (!s.booted) {
      this.model.setVariables(null);
      return;
    }
    if (s.memoryTree) {
      this.model.setVariables(s.memoryTree);
      return;
    }
    try {
      const tree = await this.backend.memoryTree();
      if (this.isOpen()) this.model.setVariables(tree);
    } catch {
      // No user memory on this model, or not set up yet: no variables.
      this.model.setVariables(null);
    }
  }

  // ---------------------------------------------------------- keys

  onKey(e) {
    const mod = e.metaKey || e.ctrlKey;
    if (this.session) {
      // The editor's own keys are its own (onEditorKey); the palette key
      // closes, as in search mode.
      if (this.bindings.is("palette", e)) {
        e.preventDefault();
        e.stopPropagation();
        this.close();
      } else if (mod && e.key.toLowerCase() === "s") {
        e.preventDefault();
      }
      return;
    }
    if (this.bindings.is("palette", e)) {
      // Closes; marked handled so the document's listener (app.js) does not reopen it.
      e.preventDefault();
      e.stopPropagation();
      this.close();
      return;
    }
    const digit = numberDigit(e, this.bindings.numberModifier);
    if (digit !== null) {
      e.preventDefault();
      this.model.chooseNumber(digit).then((r) => this.after(r));
      return;
    }
    if (e.key === "Escape" && this.detailOpen) {
      // The sheet's second step goes back to the list first.
      e.preventDefault();
      e.stopPropagation();
      this.closeDetail();
      return;
    }
    switch (e.key) {
      case "ArrowDown": this.model.move(1); break;
      case "ArrowUp": this.model.move(-1); break;
      case "PageDown": this.model.move(8); break;
      case "PageUp": this.model.move(-8); break;
      case "Home": if (!e.target.matches("input")) this.model.select(0); else return; break;
      case "End": if (!e.target.matches("input")) this.model.select(this.model.rows.length - 1); else return; break;
      case "Enter":
        if (e.target instanceof HTMLButtonElement || e.target instanceof HTMLAnchorElement) return;
        // Shift+Enter, or a delimiter still open: the text needs more
        // lines, so the palette grows to the editor.
        if (e.target === this.ui.input && !mod && (e.shiftKey || unclosed(this.ui.input.value))) {
          this.enterEditor(this.ui.input.value, { newline: true });
          break;
        }
        this.choose(this.model.row, mod);
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  async choose(row, opposite = false) {
    if (!row || this.model.sending) return;
    const r = await this.model.choose(row, { opposite });
    this.after(r);
  }

  after(r) {
    if (r?.menu) {
      this.close();
      this.onMenu?.(r.menu);
      return;
    }
    if (r?.close) this.close();
  }

  async tryExample(example) {
    this.tried = { example, text: "Sending…", error: false };
    this.renderDetail();
    const r = await this.model.tryExample(example);
    const n = this.model.notice;
    this.tried = { example, text: n?.error ? n.text : "Sent. The result is on the calculator's display.", error: Boolean(n?.error) };
    this.model.notice = null;
    this.renderDetail();
    this.renderFoot();
    if (!r.close) this.loadVariables();
    // The next query can be typed at once.
    this.ui.input.focus();
  }

  // ---------------------------------------------------------- render

  render() {
    if (!this.isOpen()) return;
    // The editor's highlighting follows the index and the variables.
    if (this.session) this.editor.render();
    this.lastSearchMs = this.model.lastSearchMs ?? 0;
    if (!this.model.row) this.detailOpen = false;
    this.querySelector(".palette-box").classList.toggle("detail-open", this.detailOpen);
    this.renderList();
    this.renderDetail();
    this.renderFoot();
  }

  /** The line under the input: what this palette can do on this model now. */
  renderState() {
    const s = this.store.state;
    const m = this.model;
    const model = this.shownModel();
    this.ui.model.textContent = model ? MODEL_TITLES[model] ?? model : "";
    let text = "";
    if (m.loadError) text = `The command reference could not be read (${m.loadError}). Actions still work.`;
    else if (!m.index) text = "Reading the command reference…";
    else if (!s.booted && m.index.supported) text = `No calculator is running. You can read the ${MODEL_TITLES[model]} reference but not send anything. Choose a ROM to start.`;
    else if (!s.booted) text = `No calculator is running, and the ${MODEL_TITLES[model]} has no command reference (48SX, 48GX and 49G only). Actions are listed.`;
    else if (!m.index.supported && m.noTyping) {
      // The models without a command line say it plainly; any other reason the host gave is kept.
      text = /no RPL command line/.test(m.noTyping)
        ? `The ${MODEL_TITLES[s.booted]} has no command reference and no command line to type into. Actions are listed.`
        : `The ${MODEL_TITLES[s.booted]} has no command reference, and nothing can be sent: ${m.noTyping}. Actions are listed.`;
    }
    else if (!m.index.supported) text = `The ${MODEL_TITLES[s.booted]} has no command reference (48SX, 48GX and 49G only). Text can still be typed into it, and actions are listed.`;
    else if (m.noTyping) text = `Nothing can be sent: ${m.noTyping}. The reference can still be read.`;
    // Not in the editor: saving there sets the mode itself (the host's transfer).
    else if (m.algebraic && !this.session) text = "The 49G is in algebraic mode. Commands and examples here are written for RPN mode.";
    this.ui.state.textContent = text;
    this.ui.state.hidden = !text;
  }

  shortcutLabel(n) {
    return this.bindings.numberLabel(n);
  }

  renderList() {
    const m = this.model;
    const rows = m.rows.map((row, i) => this.rowView(row, i));
    if (!rows.length) {
      const q = m.query.trim();
      rows.push(el("p", { class: "palette-empty", text: q
        ? (m.index?.supported ? `No command, variable or action matches “${q}”.` : `No action matches “${q}”; this model has no command reference.`)
        : (m.index?.supported ? "Type a command (SIN, ->LIST, \\.S), part of its description, a variable, an action, or text to send (13 4 ^)." : "Type an action's name.") }));
    }
    this.ui.list.replaceChildren(...rows);
    const sel = this.ui.list.querySelector('[aria-selected="true"]');
    sel?.scrollIntoView({ block: "nearest" });
    this.ui.input.setAttribute("aria-activedescendant", sel?.id ?? "");
  }

  rowView(row, i) {
    const selected = i === this.model.selected;
    const off = row.kind === "action" && row.action.off;
    const node = el("div", {
      class: `prow prow-${row.kind}${off ? " prow-off" : ""}`,
      role: "option",
      id: `prow-${i}`,
      "aria-selected": String(selected),
      "aria-disabled": off ? "true" : null,
      "data-row": i,
    });
    // No kind with an empty query: every row is an action then.
    const kind = el("span", { class: "prow-kind", text: this.model.query.trim() ? KIND_LABELS[row.kind] : "" });
    let head;
    let desc = null;
    switch (row.kind) {
      case "command":
        head = el("div", { class: "prow-head" }, el("span", { class: "prow-name", text: row.name }), el("span", { class: "prow-stack", text: row.stack }));
        desc = row.description;
        break;
      case "variable":
        head = el("div", { class: "prow-head" }, el("span", { class: "prow-name", text: row.name }),
          el("span", { class: "prow-stack", text: row.type ? row.type.toLowerCase() : "" }));
        desc = `Variable in ${row.path.join(" › ")}`;
        break;
      case "menu":
        head = el("div", { class: "prow-head" }, el("span", { class: "prow-name", text: row.name }),
          el("span", { class: "prow-stack", text: `${row.count} commands` }));
        desc = "A calculator menu; opens it in the Reference tab.";
        break;
      case "send":
        head = el("div", { class: "prow-head" }, el("span", { class: "prow-name prow-text", text: row.name }));
        desc = this.model.verbFor(row) === "run" ? "Type it and press ENTER" : "Type it into the command line";
        break;
      default:
        head = el("div", { class: "prow-head" }, el("span", { class: "prow-name prow-title", text: row.name }));
        desc = off || row.description;
    }
    node.append(el("div", { class: "prow-main" }, head, desc ? el("div", { class: "prow-desc", text: desc }) : null), kind);
    node.append(i < 9 ? el("kbd", { class: "prow-hint", text: this.shortcutLabel(i + 1) }) : el("span", { class: "prow-hint" }));
    // The phone sheet: a tapped row opens its entry.
    node.append(iconEl("chevron-right", "prow-go"));
    return node;
  }

  renderDetail() {
    const m = this.model;
    const row = m.row;
    const box = this.ui.detail;
    if (!row) {
      box.replaceChildren();
      return;
    }
    const enterHint = this.enterHint(row);
    const show = (...nodes) => box.replaceChildren(this.backBar(row), ...nodes, this.actionBar(row));
    if (row.kind === "command") {
      const command = m.index.byName.get(row.name);
      const ctx = {
        legends: this.legendsModel === m.index.model ? this.legends : null,
        onTry: m.canType ? (x) => this.tryExample(x) : null,
        whyNot: m.canType ? null : (m.noTyping ?? "No calculator is running"),
        tried: this.tried,
      };
      show(entryView(m.index, command, ctx), enterHint);
      return;
    }
    if (row.kind === "variable") {
      show(el("div", { class: "entry" },
        el("div", { class: "entry-head" }, el("h3", { class: "entry-name", text: row.name }),
          el("span", { class: "chip current", text: "variable" })),
        el("p", { class: "entry-desc" }, `Your ${row.directory ? "directory" : "variable"} in `, el("span", { class: "menu-path", text: row.path.join(" › ") }),
          row.type ? `, a ${row.type.toLowerCase()}.` : "."),
        enterHint));
      return;
    }
    if (row.kind === "menu") {
      const names = menuCommands(row.menu).map((c) => c.name);
      show(el("div", { class: "entry" },
        el("div", { class: "entry-head" }, el("h3", { class: "entry-name", text: row.name }), el("span", { class: "chip current", text: "menu" })),
        el("p", { class: "entry-desc", text: `A menu of the ${MODEL_TITLES[m.index.model]}'s ROM${row.menu.children.length ? `, with ${row.menu.children.length} submenus` : ""}.` }),
        names.length ? el("p", { class: "entry-names", text: names.join("  ") }) : null,
        enterHint));
      return;
    }
    if (row.kind === "send") {
      show(el("div", { class: "entry" },
        el("div", { class: "entry-head" }, el("h3", { class: "entry-name", text: "Text to type" })),
        el("pre", { class: "entry-stack", text: row.name }),
        el("p", { class: "entry-desc", text: "Typed into the calculator key by key. A line break becomes the calculator's newline." }),
        enterHint));
      return;
    }
    show(el("div", { class: "entry" },
      el("div", { class: "entry-head" }, el("h3", { class: "entry-name entry-title", text: row.name }), el("span", { class: "chip current", text: "action" })),
      row.description ? el("p", { class: "entry-desc", text: row.description }) : null,
      row.action.off ? el("p", { class: "entry-desc muted", text: stop(row.action.off) }) : null,
      enterHint));
  }

  /** The phone sheet's way back to the list (shown below 760 px only). */
  backBar(row) {
    return el("div", { class: "palette-back" },
      el("button", { type: "button", class: "icon", "data-palette": "back", title: "Back to the list", "aria-label": "Back to the list" }, iconEl("chevron-left")),
      el("span", { class: "palette-back-name", text: row.kind === "send" ? "Text to type" : row.name }));
  }

  /** The phone sheet's buttons for what Enter and Cmd/Ctrl+Enter do (shown below 760 px only). */
  actionBar(row) {
    const m = this.model;
    const button = (label, which, primary) => el("button", { type: "button", class: primary ? "primary" : null, "data-palette": which, text: label });
    // An action that is off: its entry says why, nothing to press.
    if (row.kind === "action" && row.action.off) return el("div", { class: "palette-actions" });
    if (row.kind === "action") return el("div", { class: "palette-actions" }, button("Run", "choose", true));
    if (row.kind === "menu") return el("div", { class: "palette-actions" }, button("Open in the Reference tab", "choose", true));
    if (!m.canType) return el("div", { class: "palette-actions" }, el("p", { class: "muted", text: m.noTyping ? `Nothing can be sent: ${m.noTyping}.` : "Start the calculator to send this." }));
    const verb = m.verbFor(row);
    const what = row.kind === "send" ? "as typed" : row.kind === "variable" ? "its name" : row.name;
    const label = (v) => (v === "run" ? `Run ${what === "as typed" ? "" : what}`.trim() : `Insert ${what}`);
    return el("div", { class: "palette-actions" },
      button(label(verb === "run" ? "insert" : "run"), "opposite", false),
      button(label(verb), "choose", true));
  }

  /** What Enter and Cmd/Ctrl+Enter do to the selected row. */
  enterHint(row) {
    const m = this.model;
    const mod = this.isMac ? "⌘" : "Ctrl+";
    const key = (t) => el("kbd", { text: t });
    if (row.kind === "action" && row.action.off) return null;
    if (row.kind === "action") return el("p", { class: "entry-enter" }, key("Enter"), " runs this action.");
    if (row.kind === "menu") return el("p", { class: "entry-enter" }, key("Enter"), " opens it in the Reference tab beside the calculator.");
    if (!m.canType) return el("p", { class: "entry-enter muted", text: m.noTyping ? `Nothing can be sent: ${m.noTyping}.` : "Start the calculator to send this." });
    const verb = m.verbFor(row);
    const other = m.verbFor(row, true);
    const what = row.kind === "send" ? "the text" : row.kind === "variable" ? "its name" : row.name;
    const explain = (v) => (v === "run"
      ? (m.commandLine?.active ? `types ${what} after the open command line and presses ENTER` : `types ${what} and presses ENTER`)
      : (m.commandLine?.active ? `inserts ${what} at the cursor` : `starts a command line with ${what}`));
    return el("p", { class: "entry-enter" },
      key("Enter"), ` ${explain(verb)}; `, key(`${mod}Enter`), ` ${explain(other)}.`,
      m.commandLine?.active ? el("span", { class: "muted", text: ` The command line holds “${m.commandLine.text}”.` }) : null);
  }

  renderFoot() {
    const m = this.model;
    const s = this.store.state;
    const n = this.ui.notice;
    if (this.session) this.renderEditor();
    if (m.sending || this.saving) n.textContent = m.notice?.text ?? "Sending…";
    else if (s.busy) n.textContent = s.writing ? "The calculator is busy with a transfer…" : "The calculator is busy typing…";
    else n.textContent = m.notice?.text ?? "";
    n.classList.toggle("error", Boolean(m.notice?.error) && !m.sending && !this.saving);
    n.classList.toggle("busy", m.sending || this.saving || s.busy);
    n.hidden = !n.textContent;
    const key = (t) => el("kbd", { text: t });
    const mod = this.isMac ? "⌘" : "Ctrl+";
    const hints = [key("↑"), key("↓"), " move · "];
    const verb = m.row ? m.verbFor(m.row) : null;
    if (m.canType && verb) {
      hints.push(key("Enter"), ` ${verb} · `, key(`${mod}Enter`), ` ${verb === "run" ? "insert" : "run"} · `);
    } else if (m.row) {
      hints.push(key("Enter"), m.row.kind === "menu" ? " open · " : m.row.kind === "action" ? " run · " : " choose · ");
    }
    hints.push(key(this.shortcutLabel("1–9")), " pick a row · ", key("Esc"), " close");
    this.ui.hints.replaceChildren(...hints);
  }

  // ---------------------------------------------------------- editor mode

  /** The Sets the highlighting needs: the model's command names and the variables' names (cached while they stay). */
  highlightContext() {
    const c = this.contextCache;
    if (c.index !== this.model.index || c.vars !== this.model.variables) {
      c.index = this.model.index;
      c.vars = this.model.variables;
      c.value = {
        commands: new Set((this.model.index?.commands ?? []).map((x) => x.name)),
        variables: new Set(this.model.variables.map((v) => v.name)),
      };
    }
    return c.value;
  }

  isEditing() {
    return this.session !== null;
  }

  /**
   * The editor with `text`: free text to send (`target` null), or what
   * `target` holds (`{kind: "cmdline"}`, `{kind: "variable", dir, name}`,
   * `{kind: "level", level}`). `newline` adds an indented line at the
   * end; `cursor` puts the cursor there; `was` is the object's identity
   * from `editText`.
   */
  enterEditor(text, { target = null, cursor = null, newline = false, broken = null, was = null } = {}) {
    // Free text from the input: its digraphs become characters, as typed in the editor.
    if (!target) text = fromDigraphs(text);
    this.session = new EditSession(target, text, was);
    this.session.broken = broken;
    this.discarding = false;
    this.history.reset();
    this.ui.box.classList.add("editing");
    this.ui.editor.hidden = false;
    this.renderState();
    this.model.notice = broken ? { text: broken, error: true } : null;
    this.editor.set(text, cursor ?? text.length);
    if (newline) this.editor.insert(`\n${indentAfter(text)}`);
    this.render();
    this.renderFoot();
    this.editor.focus();
  }

  leaveEditor() {
    this.session = null;
    this.discarding = false;
    this.saving = false;
    this.ui.box.classList.remove("editing");
    this.ui.editor.hidden = true;
    this.renderState();
  }

  /**
   * Open the editor on what `target` holds: the calculator's command line
   * (its text and cursor, read from RAM), or a variable's or a stack
   * level's text (`editText`). Opens the palette first. `returnFocus()`
   * gives the element the focus goes back to when the editor closes
   * (null: none, the keys go to the calculator).
   */
  async openEditor(target, { returnFocus = null } = {}) {
    if (this.isOpen() && this.session?.dirty) {
      this.close();
      if (this.isOpen()) return;
    }
    if (!this.isOpen()) await this.open();
    this.returnFocus = returnFocus;
    try {
      if (target.kind === "cmdline") {
        const line = await this.backend.commandLine();
        if (!line.active) throw new Error("the calculator has no command line open");
        // The cursor counts the calculator's characters (code points).
        const cursor = [...line.text].slice(0, line.cursor).join("").length;
        this.enterEditor(line.text, { target, cursor });
      } else {
        const { text, was } = await pullEdit(this.backend, target);
        // A program comes in on one line: laid out by its structure.
        this.enterEditor(text.startsWith("«") ? format(text) : text, { target, cursor: 0, was });
      }
    } catch (err) {
      const why = String(err?.message ?? err);
      this.enterEditor("", { target, broken: `${sentence(targetTitle(target))} cannot be edited: ${why}` });
    }
  }

  onEdit(text) {
    if (!this.session) return;
    this.session.text = text;
    if (this.discarding) {
      this.discarding = false;
      this.model.notice = null;
    }
    this.renderFoot();
  }

  /** The editor's keys before its own: save, send, history. True when taken. */
  onEditorKey(e) {
    const what = editorKey(e, this.session?.target ?? null);
    if (!what) return false;
    e.preventDefault();
    if (what === "primary" || what === "secondary") {
      // The same as the button.
      this.editorAction(what);
    } else if (what === "older" || what === "newer") {
      const text = this.history.move(what === "older" ? -1 : 1, this.editor.value);
      if (text !== null) {
        this.editor.set(text);
        this.onEdit(text);
      }
    } else if (what === "format") {
      this.formatAll();
    }
    return true;
  }

  formatAll() {
    const text = this.editor.value;
    const out = format(text);
    if (out === text) return;
    this.editor.area.select();
    this.editor.insert(out);
    this.editor.area.setSelectionRange(0, 0);
    this.editor.area.scrollTop = 0;
  }

  /** The verbs of free text: what the primary and secondary buttons send. */
  freeVerbs() {
    const row = { kind: "send", name: this.editor.value };
    return [this.model.verbFor(row) ?? "run", this.model.verbFor(row, true) ?? "insert"];
  }

  async editorAction(which) {
    const sess = this.session;
    if (!sess || this.saving || this.model.sending) return;
    const text = this.editor.value;
    if (!sess.target) {
      if (!text.trim()) return;
      const [primary, secondary] = this.freeVerbs();
      const verb = which === "primary" ? primary : secondary;
      const r = await this.model.send(verb, text, { closeAfter: true });
      if (!this.model.notice?.error) this.history.push(text);
      if (r.close) this.close();
      else this.editor.focus();
      return;
    }
    if (which !== "primary" || sess.broken) return;
    this.saving = true;
    this.model.notice = { text: sess.target.kind === "cmdline" ? "Sending back…" : "Saving on the calculator…", error: false };
    this.renderFoot();
    const start = performance.now();
    // A save that closes the editor reads nothing back; one it stays
    // open after (typed while it saved) checks the next save against it.
    const r = await saveSession(this.backend, sess, text, { keep: () => this.editor.value !== text });
    this.saving = false;
    if (this.session !== sess) return;
    // Saved: the editor closes and the status line tells what was saved.
    // Not saved: it stays open with the error and the text. Typed while
    // it saved: open with the newer text, unsaved.
    if (r.ok) this.history.push(text);
    const changed = r.ok && this.editor.value !== text;
    if (changed) sess.text = this.editor.value;
    const next = afterSave(sess.target, r, performance.now() - start, { changed, broken: sess.broken });
    if (!next.close) {
      this.model.notice = next.notice;
      this.renderFoot();
      this.editor.focus();
      return;
    }
    if (next.message) this.store.set({ message: next.message, messageError: false });
    this.close();
  }

  renderEditor() {
    const sess = this.session;
    if (!sess) return;
    const ui = this.ui;
    const mod = this.isMac ? "⌘" : "Ctrl+";
    const key = (t) => el("kbd", { text: t });
    ui.editorTitle.textContent = sess.target ? `Editing ${targetTitle(sess.target)}` : "Text to send";
    ui.format.title = `Re-indent by structure (${comboLabel(FORMAT_KEY, { isMac: this.isMac })})`;
    ui.editorDirty.hidden = !sess.dirty;
    ui.dialog.setAttribute("aria-label", sess.target ? `Editor: ${targetTitle(sess.target)}` : "Editor");
    const can = this.model.canType || (sess.target && sess.target.kind !== "cmdline");
    const busy = this.saving || this.model.sending;
    const hints = [];
    if (!sess.target) {
      const [primary, secondary] = this.freeVerbs();
      const label = (v) => (v === "run" ? "Run" : "Insert");
      ui.primary.textContent = label(primary);
      ui.primary.title = `${primary === "run" ? "Type it and press ENTER" : "Type it into the command line"} (${mod}Enter)`;
      ui.secondary.textContent = label(secondary);
      ui.secondary.title = `${secondary === "run" ? "Type it and press ENTER" : "Type it into the command line"} (${mod}Shift+Enter)`;
      ui.secondary.hidden = false;
      ui.primary.disabled = busy || !can || !this.editor.value.trim();
      ui.secondary.disabled = ui.primary.disabled;
      hints.push(key(`${mod}Enter`), ` ${label(primary).toLowerCase()} · `);
    } else {
      const back = sess.target.kind === "cmdline";
      ui.primary.textContent = back ? "Send back" : "Save";
      ui.primary.title = back
        ? `Replace the command line's text; the calculator stays in edit mode (${mod}S)`
        : `Save it on the calculator and close (${mod}S). A syntax error changes nothing and keeps the editor open.`;
      ui.secondary.hidden = true;
      ui.primary.disabled = busy || Boolean(sess.broken) || !this.store.state.booted;
      hints.push(key(`${mod}S`), back ? " send back · " : " save · ");
    }
    hints.push(key("Tab"), " complete · ", key(this.isMac ? "⌥↑↓" : "Alt+↑↓"), " history · ", key("Esc"), " close");
    ui.editorHints.replaceChildren(...hints);
  }
}

customElements.define("sat-palette", SatPalette);
