// <sat-palette>: the command palette, opened with Cmd/Ctrl+K or its
// button. One input over the calculator: it suggests the model's
// commands (the reference), the user's variables and the app's actions
// while typing, shows the selected entry beside the list, and sends the
// choice to the calculator through the protocol's typing verbs
// (`PaletteModel`, web/palette.js). A modal <dialog>: while it is open
// the keys are its own (sat-calculator.js leaves events inside a
// dialog alone) and it gives them back on close. Light DOM.

import { PaletteModel } from "../palette.js";
import { menuCommands } from "../reference.js";
import { el, entryView } from "./entry-view.js";
import { MODEL_TITLES } from "./sat-calculator.js";

const TEMPLATE = `
  <dialog class="palette" aria-label="Command palette">
    <div class="palette-box">
      <div class="palette-input">
        <span class="palette-glyph" aria-hidden="true">›</span>
        <input type="text" aria-label="Command, variable, text to send, or an action" placeholder="Command, variable, text to send, or an action" autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" enterkeyhint="go">
        <span class="palette-model"></span>
        <kbd class="palette-esc">esc</kbd>
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
   * a menu in the explorer's Commands tab.
   */
  attach(backend, store, { reference, actions = [], onMenu = null }) {
    this.backend = backend;
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
    };
    this.model = new PaletteModel(backend, store, { actions: typeof actions === "function" ? [] : actions });
    this.model.onChange = () => this.render();
    /** The skin's keys of the shown model, for the keyboard placement. */
    this.legends = null;
    this.legendsModel = null;
    /** The last "try it" and its outcome, shown under the example. */
    this.tried = null;
    this.isMac = /Mac|iPhone|iPad/.test(navigator.platform ?? "");
    /** The number shortcuts: Cmd+digit in the desktop app on a Mac, Ctrl+digit elsewhere (browsers reserve Cmd+digit). */
    this.numberKey = backend.host === "tauri" && this.isMac ? "meta" : "ctrl";
    this.renderedModel = null;
    this.lastSearchMs = 0;

    this.ui.input.addEventListener("input", () => this.model.setQuery(this.ui.input.value));
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
      if (row) this.choose(this.model.rows[Number(row.dataset.row)], e.metaKey || e.ctrlKey);
    });

    store.watch(["booted", "model"], () => {
      if (this.isOpen()) this.loadIndex();
    });
    store.watch(["memoryTree"], (s) => {
      if (this.isOpen() && s.memoryTree) this.model.setVariables(s.memoryTree);
    });
    store.watch(["busy"], () => this.renderFoot());
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
    this.ui.input.value = query;
    this.ui.dialog.showModal();
    this.ui.input.focus();
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
        description: "Runs CF(-95). The command names and the examples are RPN text, which the 49G's algebraic mode does not parse; the mode's echo of the line stays on the stack.",
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
    if (this.isOpen()) this.ui.dialog.close();
  }

  /** The keys go back to the calculator: nothing in the page keeps the focus. */
  afterClose() {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    this.dispatchEvent(new CustomEvent("sat-palette-closed", { bubbles: true }));
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
    if (mod && !e.altKey && !e.shiftKey && e.code === "KeyK") {
      e.preventDefault();
      this.close();
      return;
    }
    const digit = /^Digit([1-9])$/.exec(e.code);
    if (digit && !e.altKey && !e.shiftKey && (this.numberKey === "meta" ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey)) {
      e.preventDefault();
      this.model.chooseNumber(Number(digit[1])).then((r) => this.after(r));
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
    this.tried = { example, text: n?.error ? n.text : "Sent; the calculator shows the result.", error: Boolean(n?.error) };
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
    this.lastSearchMs = this.model.lastSearchMs ?? 0;
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
    if (m.loadError) text = `The command reference could not be read (${m.loadError}); app actions still work.`;
    else if (!m.index) text = "Reading the command reference…";
    else if (!s.booted && m.index.supported) text = `No calculator is running: the ${MODEL_TITLES[model]}'s reference can be read, nothing can be sent. Choose a ROM to start.`;
    else if (!s.booted) text = `No calculator is running, and there is no command reference for the ${MODEL_TITLES[model]} (48SX, 48GX and 49G only). App actions are listed.`;
    else if (!m.index.supported && m.noTyping) text = `No command reference for the ${MODEL_TITLES[s.booted]}, and ${m.noTyping.replace(/^the /, "the ")}. App actions are listed.`;
    else if (!m.index.supported) text = `No command reference for the ${MODEL_TITLES[s.booted]} (48SX, 48GX and 49G only): text can be sent as typed, and app actions are listed.`;
    else if (m.noTyping) text = `Nothing can be sent: ${m.noTyping}. The reference can still be read.`;
    else if (m.algebraic) text = "The 49G is in algebraic mode: command names and examples are RPN text and will not parse until it is switched (the action “Switch the HP 49G to RPN mode”, or CF(-95)).";
    this.ui.state.textContent = text;
    this.ui.state.hidden = !text;
  }

  shortcutLabel(n) {
    if (this.numberKey === "meta") return `⌘${n}`;
    return this.isMac ? `⌃${n}` : `Ctrl+${n}`;
  }

  renderList() {
    const m = this.model;
    const rows = m.rows.map((row, i) => this.rowView(row, i));
    if (!rows.length) {
      const q = m.query.trim();
      rows.push(el("p", { class: "palette-empty", text: q
        ? (m.index?.supported ? `No command, variable or action matches “${q}”.` : `No action matches “${q}”; this model has no command reference.`)
        : (m.index?.supported ? "Type a command's name (PL, sto, ->LIST, \\.S), part of its description, a variable, text to send (13 4 ^), or an action." : "Type an action's name.") }));
    }
    this.ui.list.replaceChildren(...rows);
    const sel = this.ui.list.querySelector('[aria-selected="true"]');
    sel?.scrollIntoView({ block: "nearest" });
    this.ui.input.setAttribute("aria-activedescendant", sel?.id ?? "");
  }

  rowView(row, i) {
    const selected = i === this.model.selected;
    const node = el("div", {
      class: `prow prow-${row.kind}`,
      role: "option",
      id: `prow-${i}`,
      "aria-selected": String(selected),
      "data-row": i,
    });
    const kind = el("span", { class: "prow-kind", text: KIND_LABELS[row.kind] });
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
        desc = `Your variable in ${row.path.join(" › ")}`;
        break;
      case "menu":
        head = el("div", { class: "prow-head" }, el("span", { class: "prow-name", text: row.name }),
          el("span", { class: "prow-stack", text: `${row.count} commands` }));
        desc = "A menu of the ROM; opens it in the Commands tab.";
        break;
      case "send":
        head = el("div", { class: "prow-head" }, el("span", { class: "prow-name prow-text", text: row.name }));
        desc = this.model.verbFor(row) === "run" ? "Send as typed and press ENTER" : "Send as typed into the command line";
        break;
      default:
        head = el("div", { class: "prow-head" }, el("span", { class: "prow-name prow-title", text: row.name }));
        desc = row.description;
    }
    node.append(el("div", { class: "prow-main" }, head, desc ? el("div", { class: "prow-desc", text: desc }) : null), kind);
    node.append(i < 9 ? el("kbd", { class: "prow-hint", text: this.shortcutLabel(i + 1) }) : el("span", { class: "prow-hint" }));
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
    if (row.kind === "command") {
      const command = m.index.byName.get(row.name);
      const ctx = {
        legends: this.legendsModel === m.index.model ? this.legends : null,
        onTry: m.canType ? (x) => this.tryExample(x) : null,
        whyNot: m.canType ? null : (m.noTyping ?? "No calculator is running"),
        tried: this.tried,
      };
      box.replaceChildren(entryView(m.index, command, ctx), enterHint);
      return;
    }
    if (row.kind === "variable") {
      box.replaceChildren(el("div", { class: "entry" },
        el("div", { class: "entry-head" }, el("h3", { class: "entry-name", text: row.name }),
          el("span", { class: "chip current", text: "variable" })),
        el("p", { class: "entry-desc" }, `Your ${row.directory ? "directory" : "variable"} in `, el("span", { class: "menu-path", text: row.path.join(" › ") }),
          row.type ? `, a ${row.type.toLowerCase()}.` : "."),
        enterHint));
      return;
    }
    if (row.kind === "menu") {
      const names = menuCommands(row.menu).map((c) => c.name);
      box.replaceChildren(el("div", { class: "entry" },
        el("div", { class: "entry-head" }, el("h3", { class: "entry-name", text: row.name }), el("span", { class: "chip current", text: "menu" })),
        el("p", { class: "entry-desc", text: `A menu of the ${MODEL_TITLES[m.index.model]}'s ROM${row.menu.children.length ? `, with ${row.menu.children.length} submenus` : ""}.` }),
        names.length ? el("p", { class: "entry-names", text: names.join("  ") }) : null,
        enterHint));
      return;
    }
    if (row.kind === "send") {
      box.replaceChildren(el("div", { class: "entry" },
        el("div", { class: "entry-head" }, el("h3", { class: "entry-name", text: "Send as typed" })),
        el("pre", { class: "entry-stack", text: row.name }),
        el("p", { class: "entry-desc", text: "Typed into the calculator key by key, as the keyboard would; a newline in the text is the calculator's newline." }),
        enterHint));
      return;
    }
    box.replaceChildren(el("div", { class: "entry" },
      el("div", { class: "entry-head" }, el("h3", { class: "entry-name entry-title", text: row.name }), el("span", { class: "chip current", text: "action" })),
      row.description ? el("p", { class: "entry-desc", text: row.description }) : null,
      enterHint));
  }

  /** What Enter and Cmd/Ctrl+Enter do to the selected row. */
  enterHint(row) {
    const m = this.model;
    const mod = this.isMac ? "⌘" : "Ctrl+";
    const key = (t) => el("kbd", { text: t });
    if (row.kind === "action") return el("p", { class: "entry-enter" }, key("Enter"), " runs this action.");
    if (row.kind === "menu") return el("p", { class: "entry-enter" }, key("Enter"), " opens it in the Commands tab beside the calculator.");
    if (!m.canType) return el("p", { class: "entry-enter muted", text: m.noTyping ? `Nothing can be sent: ${m.noTyping}.` : "Start a calculator to send this." });
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
    if (m.sending) n.textContent = m.notice?.text ?? "Sending…";
    else if (s.busy) n.textContent = "The calculator is busy typing…";
    else n.textContent = m.notice?.text ?? "";
    n.classList.toggle("error", Boolean(m.notice?.error) && !m.sending);
    n.classList.toggle("busy", m.sending || s.busy);
    n.hidden = !n.textContent;
    const key = (t) => el("kbd", { text: t });
    const mod = this.isMac ? "⌘" : "Ctrl+";
    const hints = [key("↑"), key("↓"), " choose · "];
    const verb = m.row ? m.verbFor(m.row) : null;
    if (m.canType && verb) {
      hints.push(key("Enter"), ` ${verb} · `, key(`${mod}Enter`), ` ${verb === "run" ? "insert" : "run"} · `);
    } else {
      hints.push(key("Enter"), " choose · ");
    }
    hints.push(key(this.shortcutLabel("1–9")), " pick a row · ", key("Esc"), " close");
    this.ui.hints.replaceChildren(...hints);
  }
}

customElements.define("sat-palette", SatPalette);
