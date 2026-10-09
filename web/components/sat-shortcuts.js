// <sat-shortcuts>: the "Keyboard shortcuts" dialog. Lists what the
// computer keyboard does: typing (fixed), the calculator keys a keyboard
// has no key for and the app's actions (both rebindable, web/bindings.js),
// and the palette's row-number modifier. A key is added by pressing it
// while the row records; each key shows its warnings (another action's,
// the browser's, a dead key). Clicks with a modifier on the drawn keys
// are listed as fixed (web/shiftclick.js). Opened by the panel's link,
// its own key and the palette. A modal <dialog>: the calculator leaves
// its keys alone. Light DOM.

import { ACTIONS, NUMBER_MODIFIERS, comboOf, numberLabel } from "../bindings.js";
import { clickLabel } from "../shiftclick.js";
import { el } from "./entry-view.js";
import { icon, iconEl } from "./icons.js";

const TEMPLATE = `
  <dialog class="shortcuts" aria-labelledby="shortcuts-title">
    <div class="shortcuts-box">
      <header class="dialog-head shortcuts-head">
        <h2 id="shortcuts-title">Keyboard shortcuts</h2>
        <button type="button" class="icon shortcuts-close" title="Close" aria-label="Close">${icon("close")}</button>
      </header>
      <div class="shortcuts-body">
        <p class="shortcuts-intro">A shortcut is a physical key, so it stays where it is on any layout; it is shown with the label your layout gives it. <strong>Add key</strong>, then press the key or combination; Esc cancels (<strong>Use Esc</strong> records Esc itself).</p>
        <section>
          <h3>Calculator keys</h3>
          <table class="shortcuts-table"><tbody class="group-calculator"></tbody></table>
        </section>
        <section>
          <h3>App</h3>
          <table class="shortcuts-table"><tbody class="group-app"></tbody></table>
        </section>
        <section>
          <h3>Typing <span class="muted">(fixed)</span></h3>
          <table class="shortcuts-table typing"><tbody>
            <tr><th scope="row"><kbd>a</kbd>–<kbd>z</kbd> <kbd>A</kbd>–<kbd>Z</kbd></th><td>Letters, through the calculator's alpha mode (and its shift for lowercase).</td></tr>
            <tr><th scope="row"><kbd>0</kbd>–<kbd>9</kbd> <kbd>.</kbd> <kbd>+</kbd> <kbd>-</kbd> <kbd>*</kbd> <kbd>/</kbd> <kbd>^</kbd> <kbd>'</kbd></th><td>As printed: the characters, wherever your layout has them.</td></tr>
            <tr><th scope="row"><kbd>Enter</kbd> <kbd>Space</kbd> <kbd>⌫</kbd> <kbd>Del</kbd> <kbd>↑</kbd><kbd>↓</kbd><kbd>←</kbd><kbd>→</kbd></th><td>ENTER, SPC, ⬅, DEL and the cursor keys.</td></tr>
            <tr><th scope="row"><kbd>F1</kbd>–<kbd>F6</kbd></th><td>The menu keys under the display.</td></tr>
            <tr><th scope="row">Paste</th><td>Types the clipboard's text into the command line.</td></tr>
          </tbody></table>
          <p class="muted">Click or tap the drawn keys for everything else.</p>
        </section>
        <section>
          <h3>Mouse <span class="muted">(fixed)</span></h3>
          <table class="shortcuts-table mouse"><tbody>
            <tr><th scope="row" class="click-ctrl"></th><td>A drawn key's left-shifted function (↰): the shift, then the key. With one shift (38G, 39G, 40G, 42S), its shifted function.</td></tr>
            <tr><th scope="row" class="click-alt"></th><td>A drawn key's right-shifted function (↱); with one shift, the same as the other.</td></tr>
          </tbody></table>
          <p class="muted">The shift is pressed only when it is not on already. Holding the modifier lights the labels it reaches.</p>
        </section>
      </div>
      <footer class="dialog-foot shortcuts-foot">
        <p class="shortcuts-where muted"></p>
        <button type="button" class="shortcuts-reset">Reset to defaults</button>
        <button type="button" class="shortcuts-done primary">Done</button>
      </footer>
    </div>
  </dialog>`;

export class SatShortcuts extends HTMLElement {
  /** `bindings` is the page's `Bindings`; `where` says where they are kept. */
  attach(bindings, { where = "" } = {}) {
    this.bindings = bindings;
    this.innerHTML = TEMPLATE;
    const $ = (sel) => this.querySelector(sel);
    this.ui = {
      dialog: $("dialog"),
      calculator: $(".group-calculator"),
      app: $(".group-app"),
      where: $(".shortcuts-where"),
    };
    this.ui.where.textContent = where;
    for (const mod of ["ctrl", "alt"]) {
      const [key, click] = clickLabel(mod, bindings.isMac).split(/[-+]/);
      $(`.click-${mod}`).replaceChildren(el("kbd", { text: key }), ` + ${click}`);
    }
    /** The action recording a key, or null. */
    this.recording = null;
    /** The key last added, highlighted with its warnings. */
    this.added = null;
    this.onRecordKey = (e) => this.recordKey(e);

    $(".shortcuts-close").addEventListener("click", () => this.close());
    $(".shortcuts-done").addEventListener("click", () => this.close());
    $(".shortcuts-reset").addEventListener("click", () => {
      this.stopRecording();
      this.added = null;
      bindings.reset();
    });
    this.ui.dialog.addEventListener("close", () => {
      this.stopRecording();
      if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    });
    this.ui.dialog.addEventListener("click", (e) => {
      if (e.target === this.ui.dialog) {
        this.close();
        return;
      }
      const b = e.target.closest("button[data-act]");
      if (!b) {
        if (this.recording && !e.target.closest(".recording")) this.stopRecording();
        return;
      }
      const id = b.closest("tr[data-id]")?.dataset.id;
      switch (b.dataset.act) {
        case "add":
          if (this.recording === id) this.stopRecording();
          else this.startRecording(id);
          break;
        case "escape":
          this.record(id, "Escape");
          break;
        case "remove":
          this.stopRecording();
          bindings.remove(id, b.dataset.key);
          break;
        case "default":
          this.stopRecording();
          bindings.reset(id);
          break;
        default:
      }
    });
    this.ui.dialog.addEventListener("change", (e) => {
      if (e.target.matches("select.number-mod")) bindings.setNumberModifier(e.target.value);
    });
    bindings.onChange(() => this.render());
  }

  isOpen() {
    return this.ui.dialog.open;
  }

  open() {
    if (this.isOpen()) return;
    this.added = null;
    this.render();
    this.ui.dialog.showModal();
    this.querySelector(".shortcuts-done").focus();
  }

  close() {
    if (this.isOpen()) this.ui.dialog.close();
  }

  toggle() {
    if (this.isOpen()) this.close();
    else this.open();
  }

  // ---------------------------------------------------------- recording

  startRecording(id) {
    this.stopRecording();
    this.recording = id;
    // Before anything else sees the key: the dialog's Escape, Tab's focus move, the page's shortcuts.
    window.addEventListener("keydown", this.onRecordKey, true);
    this.render();
    this.querySelector(".recording")?.focus();
  }

  stopRecording() {
    if (!this.recording) return;
    this.recording = null;
    window.removeEventListener("keydown", this.onRecordKey, true);
    this.render();
  }

  recordKey(e) {
    e.preventDefault();
    e.stopImmediatePropagation();
    const combo = comboOf(e);
    // A modifier alone: wait for the key it goes with.
    if (!combo) return;
    const id = this.recording;
    // Plain Esc cancels, so the keyboard can always get out ("Use Esc" records it).
    if (combo === "Escape") {
      this.stopRecording();
      this.focusAdd(id);
      return;
    }
    this.record(id, combo);
  }

  record(id, combo) {
    this.recording = null;
    window.removeEventListener("keydown", this.onRecordKey, true);
    this.added = { id, combo };
    this.bindings.add(id, combo);
    this.render();
    this.focusAdd(id);
  }

  /** Focus row `id`'s Add key button (a re-render replaced the button that had it). */
  focusAdd(id) {
    this.querySelector(`tr[data-id="${id}"] button[data-act=add]`)?.focus();
  }

  // ---------------------------------------------------------- render

  render() {
    const b = this.bindings;
    // The rows are rebuilt: a focused chip or link inside one gives its focus to the row's Add key.
    const had = this.contains(document.activeElement) ? document.activeElement.closest("tr[data-id]")?.dataset.id : null;
    for (const group of ["calculator", "app"]) {
      const rows = ACTIONS.filter((a) => a.group === group).map((a) => this.row(a));
      if (group === "app") rows.push(this.numberRow());
      this.ui[group].replaceChildren(...rows);
    }
    this.ui.dialog.querySelector(".shortcuts-reset").disabled = ACTIONS.every((a) => b.isDefault(a.id)) && !b.numberChanged;
    if (had && !this.contains(document.activeElement)) this.focusAdd(had);
  }

  row(a) {
    const b = this.bindings;
    const keys = b.keys(a.id);
    const notes = [];
    const chips = keys.map((k) => {
      const warnings = b.warnings(a.id, k);
      const fresh = this.added?.id === a.id && this.added.combo === k;
      // A dead key and Esc's fullscreen caveat are notes; the rest are warnings.
      const minor = (w) => w.kind === "dead" || (w.kind === "reserved" && k === "Escape");
      for (const w of warnings) notes.push(el("li", { class: `warn warn-${w.kind}${minor(w) ? " minor" : ""}${fresh ? " fresh" : ""}` }, el("kbd", { text: b.label(k) }), ` ${w.text}`));
      const serious = warnings.some((w) => !minor(w));
      return el("span", { class: `chip-key${serious ? " warned" : ""}${fresh ? " fresh" : ""}` },
        el("kbd", { text: b.label(k), title: k }),
        el("button", { type: "button", class: "icon chip-remove", "data-act": "remove", "data-key": k, title: `Remove ${b.label(k)}`, "aria-label": `Remove ${b.label(k)} from ${a.title}` }, iconEl("close")));
    });
    const recording = this.recording === a.id;
    const add = el("button", {
      type: "button",
      class: `chip-add${recording ? " recording" : ""}`,
      "data-act": "add",
      "aria-label": recording ? `Press the key for ${a.title}; Esc or a click cancels` : `Add a key for ${a.title}`,
    }, recording ? "Press a key… (Esc cancels)" : "Add key");
    const useEsc = recording && !keys.includes("Escape")
      ? el("button", { type: "button", class: "chip-add", "data-act": "escape", "aria-label": `Use Esc for ${a.title}` }, "Use Esc")
      : null;
    return el("tr", { "data-id": a.id },
      el("th", { scope: "row" }, el("span", { class: "sc-title", text: a.title }), el("span", { class: "sc-desc", text: a.description })),
      el("td", { class: "sc-keys" },
        el("div", { class: "chips" }, ...chips, keys.length ? null : el("span", { class: "muted", text: "no key" }), add, useEsc),
        notes.length ? el("ul", { class: "warns" }, ...notes) : null),
      el("td", { class: "sc-reset" }, b.isDefault(a.id) ? null : el("button", { type: "button", class: "link", "data-act": "default", title: `Back to ${b.defaults(a.id).map((k) => b.label(k)).join(", ")}` }, "Default")));
  }

  numberRow() {
    const b = this.bindings;
    const isMac = b.isMac;
    const select = el("select", { class: "number-mod", "aria-label": "Modifier of the palette's row numbers" },
      ...NUMBER_MODIFIERS.map((m) => {
        const o = el("option", { value: m, text: `${numberLabel(m, isMac, "1")} … ${numberLabel(m, isMac, "9")}` });
        if (m === b.numberModifier) o.selected = true;
        return o;
      }));
    const warning = b.numberWarning();
    return el("tr", { "data-id": "numbers" },
      el("th", { scope: "row" }, el("span", { class: "sc-title", text: "Palette rows" }), el("span", { class: "sc-desc", text: "Choose one of the first nine rows of the command palette." })),
      el("td", { class: "sc-keys" }, select, warning ? el("ul", { class: "warns" }, el("li", { class: "warn warn-reserved", text: warning })) : null),
      el("td", { class: "sc-reset" }));
  }
}

customElements.define("sat-shortcuts", SatShortcuts);
