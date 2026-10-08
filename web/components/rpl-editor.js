// The palette's editor: a plain <textarea> over a <pre> that shows the
// same text highlighted (web/editor.js), so typing, selection, undo and
// the on-screen keyboard stay the browser's own. Adds what an RPL editor
// needs on top: the digraphs typed as the calculator's characters, Enter
// indenting the new line, Tab completing (or indenting), the bracket
// pair at the cursor marked, and a strip of completions under the text.
// No framework, no library.

import { completions, digraph, fromDigraphs, highlight, indentAfter, INDENT, lineIndent, matchBracket, wordAt } from "../editor.js";
import { el } from "./entry-view.js";

export class RplEditor {
  /**
   * Builds the editor into `box`. `context()` gives `{commands,
   * variables}` (Sets, for the highlighting), `index()` the palette's
   * index and `variables()` the variable rows (for completion);
   * `onChange(text)` follows every edit, `onKey(e)` sees a key first and
   * returns true when it took it.
   */
  constructor(box, { context, index, variables, onChange = () => {}, onKey = () => false }) {
    this.context = context;
    this.index = index;
    this.variables = variables;
    this.onChange = onChange;
    this.onKey = onKey;
    this.hl = el("pre", { class: "rpl-hl", "aria-hidden": "true" });
    this.area = el("textarea", {
      class: "rpl-text",
      "aria-label": "RPL text",
      spellcheck: "false",
      autocapitalize: "off",
      autocomplete: "off",
      autocorrect: "off",
      wrap: "off",
      enterkeyhint: "enter",
    });
    this.strip = el("div", { class: "rpl-complete", role: "listbox", "aria-label": "Completions", hidden: true });
    box.replaceChildren(el("div", { class: "rpl-editor" }, this.hl, this.area), this.strip);
    /** The completions shown and the one Tab takes. */
    this.choices = [];
    this.chosen = 0;
    this.area.addEventListener("input", (e) => this.onInput(e));
    this.area.addEventListener("keydown", (e) => this.keydown(e));
    this.area.addEventListener("scroll", () => this.syncScroll());
    this.area.addEventListener("paste", (e) => this.paste(e));
    this.area.addEventListener("blur", () => this.hideCompletions());
    // The bracket pair follows the cursor.
    document.addEventListener("selectionchange", () => {
      if (document.activeElement === this.area) this.render();
    });
    this.strip.addEventListener("mousedown", (e) => e.preventDefault());
    this.strip.addEventListener("click", (e) => {
      const b = e.target.closest("[data-choice]");
      if (b) this.complete(Number(b.dataset.choice));
    });
  }

  get value() {
    return this.area.value;
  }

  /** Show `text` with the cursor at `cursor` (the end without one); not an edit. */
  set(text, cursor = text.length) {
    this.area.value = text;
    this.area.setSelectionRange(cursor, cursor);
    this.hideCompletions();
    this.render();
  }

  focus() {
    this.area.focus({ preventScroll: true });
  }

  /** Insert `text` at the selection as typing would (the browser's undo keeps it). */
  insert(text) {
    this.area.focus({ preventScroll: true });
    // execCommand keeps the edit in the textarea's undo history; where it
    // is gone, the plain way.
    if (!document.execCommand?.("insertText", false, text)) {
      const { selectionStart: s, selectionEnd: e } = this.area;
      this.area.setRangeText(text, s, e, "end");
      this.area.dispatchEvent(new Event("input", { bubbles: true }));
    }
  }

  /** Replace characters `from`..`to` with `text` as an edit. */
  replaceRange(from, to, text) {
    this.area.setSelectionRange(from, to);
    this.insert(text);
  }

  onInput(e) {
    // A digraph just typed becomes its character.
    if (e?.inputType === "insertText") {
      const pos = this.area.selectionStart;
      const d = digraph(this.area.value, pos);
      if (d) {
        // The ASCII before the cursor becomes one character (each is one
        // UTF-16 unit); its own input event draws it.
        const ascii = this.area.value.length - d.text.length + 1;
        this.replaceRange(pos - ascii, pos, d.text[d.cursor - 1]);
        return;
      }
      // A closer typed first on its line moves the line out to its level.
      if (/^[ \t]*[»}\]]$/.test(this.lineBefore()) && this.fixIndent()) return;
    }
    this.render();
    this.suggest();
    this.onChange(this.area.value);
  }

  paste(e) {
    const text = e.clipboardData?.getData("text/plain");
    if (text === undefined || text === null) return;
    e.preventDefault();
    this.insert(fromDigraphs(text.replace(/\r\n?/g, "\n")));
  }

  keydown(e) {
    if (this.onKey(e)) return;
    const shown = !this.strip.hidden && this.choices.length > 0;
    const plain = !e.metaKey && !e.ctrlKey && !e.altKey;
    if (shown && plain && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      const n = this.choices.length;
      this.chosen = (this.chosen + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      this.renderStrip();
      return;
    }
    if (shown && e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      this.hideCompletions();
      return;
    }
    if (e.key === "Tab" && plain && !e.shiftKey) {
      e.preventDefault();
      if (shown) this.complete(this.chosen);
      else this.insert(INDENT);
      return;
    }
    if (e.key === "Tab" && e.shiftKey && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      this.outdent();
      return;
    }
    if (e.key === "Enter" && plain && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      // The line just finished takes its level (an END, a NEXT, a »),
      // the new one the level of what is open.
      this.fixIndent();
      const before = this.area.value.slice(0, this.area.selectionStart);
      this.insert(`\n${indentAfter(before)}`);
    }
  }

  /** The current line up to the cursor. */
  lineBefore() {
    const v = this.area.value;
    const pos = this.area.selectionStart;
    return v.slice(v.lastIndexOf("\n", pos - 1) + 1, pos);
  }

  /** Give the cursor's line the indentation of its level; true when it changed. */
  fixIndent() {
    const v = this.area.value;
    const pos = this.area.selectionStart;
    if (pos !== this.area.selectionEnd) return false;
    const start = v.lastIndexOf("\n", pos - 1) + 1;
    let end = v.indexOf("\n", pos);
    if (end < 0) end = v.length;
    const have = v.slice(start, end).match(/^[ \t]*/)[0];
    const want = lineIndent(v, end);
    if (have === want || pos < start + have.length) return false;
    this.replaceRange(start, start + have.length, want);
    const to = pos - have.length + want.length;
    this.area.setSelectionRange(to, to);
    return true;
  }

  /** Take the line's first indentation step away (Shift+Tab). */
  outdent() {
    const v = this.area.value;
    const pos = this.area.selectionStart;
    const start = v.lastIndexOf("\n", pos - 1) + 1;
    const n = v.slice(start).match(/^ {1,2}/)?.[0].length ?? 0;
    if (!n) return;
    this.area.setSelectionRange(start, start + n);
    this.insert("");
    this.area.setSelectionRange(Math.max(start, pos - n), Math.max(start, pos - n));
  }

  /** The word at the cursor and what it may become. */
  suggest() {
    const pos = this.area.selectionStart;
    if (pos !== this.area.selectionEnd) {
      this.hideCompletions();
      return;
    }
    const w = wordAt(this.area.value, pos);
    this.choices = completions(this.index(), this.variables(), w.word);
    this.word = w;
    this.chosen = 0;
    this.renderStrip();
  }

  complete(i) {
    const c = this.choices[i];
    if (!c || !this.word) return;
    this.replaceRange(this.word.start, this.word.end, c.name);
    this.hideCompletions();
  }

  hideCompletions() {
    this.choices = [];
    this.strip.hidden = true;
  }

  renderStrip() {
    this.strip.hidden = this.choices.length === 0;
    this.strip.replaceChildren(...this.choices.map((c, i) => el("button", {
      type: "button",
      class: `rpl-choice rpl-${c.kind}`,
      role: "option",
      tabindex: -1,
      "aria-selected": String(i === this.chosen),
      "data-choice": i,
      title: c.detail ?? "",
    }, el("span", { class: "rpl-choice-name", text: c.name }), c.kind === "variable" ? el("span", { class: "rpl-choice-kind", text: "var" }) : null)));
  }

  /** Draw the underlay from the text, with the bracket pair at the cursor. */
  render() {
    const v = this.area.value;
    const pos = this.area.selectionStart;
    const pair = this.area.selectionStart === this.area.selectionEnd ? matchBracket(v, pos) : null;
    this.hl.innerHTML = highlight(v, this.context(), pair ?? []);
    this.syncScroll();
  }

  syncScroll() {
    this.hl.scrollTop = this.area.scrollTop;
    this.hl.scrollLeft = this.area.scrollLeft;
  }
}
