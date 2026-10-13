// The cable panel (iteration 45): the lines of the CableLog (cable.js),
// one per packet, newest at the bottom, with Pause, Clear, Copy log and
// Close. Its look is its own on purpose (green on black in both themes,
// tx/style.css "cable"). It must not slow a transfer: while it is closed
// the log only keeps its ring buffer; while it is open, the lines that
// came are drawn once per animation frame, and the panel keeps at most as
// many rows as the log keeps lines. Text only, never HTML.

import { el } from "../shared/dom.js";
import { arrow, packetText, timeText } from "./cable.js";

const SOURCE = { left: "L", right: "R", serve: "S" };
const SOURCE_NAME = { left: "the left pane", right: "the right pane", serve: "the serving screen" };

/** One line as a list item. */
export function lineItem(l) {
  const dir = l.dir === "out" ? "to the calculator" : l.dir === "in" ? "from the calculator" : "";
  const li = el("li", { class: `cable-line${l.bad ? " bad" : ""}${l.dir === "link" ? " link" : ""}`, "data-n": String(l.n) });
  li.append(
    el("span", { class: "cl-t", text: timeText(l.t).trim() }),
    el("span", { class: "cl-src", text: SOURCE[l.source] ?? "", title: SOURCE_NAME[l.source] ?? "" }),
    el("span", { class: "cl-dir", text: arrow(l.dir), title: dir }),
    el("span", { class: "cl-type", text: packetText(l) }),
    el("span", { class: "cl-len", text: l.len !== undefined ? `${l.len} B` : "" }),
    el("span", { class: "cl-desc", text: l.desc ?? "" }),
  );
  if (l.text || l.hex) {
    li.append(el("span", { class: "cl-data" }, el("span", { class: "cl-text", text: l.text ?? "" }), el("span", { class: "cl-hex", text: l.hex ?? "" })));
  }
  return li;
}

export class CableView {
  /**
   * `root` the panel's section; `log` the CableLog; `toggle` the header's
   * "Show the cable" button; `say(text, {error})` a toast; `frame(fn)` the
   * animation frame (tests pass their own).
   */
  constructor(root, { log, toggle, say, frame = (fn) => requestAnimationFrame(fn) }) {
    this.root = root;
    this.log = log;
    this.toggleButton = toggle;
    this.say = say;
    this.frame = frame;
    this.paused = false;
    this.pending = false;
    /** The number of the next line to draw. */
    this.next = 0;
    const q = (s) => root.querySelector(s);
    this.ui = { list: q(".cable-list"), pause: q(".cable-pause"), clear: q(".cable-clear"), copy: q(".cable-copy"), close: q(".cable-close"), state: q(".cable-state") };
    toggle?.addEventListener("click", () => (this.isOpen ? this.close() : this.open()));
    this.ui.close.addEventListener("click", () => {
      this.close();
      this.toggleButton?.focus();
    });
    this.ui.pause.addEventListener("click", () => this.setPaused(!this.paused));
    this.ui.clear.addEventListener("click", () => this.clear());
    this.ui.copy.addEventListener("click", () => this.copy());
  }

  get isOpen() { return !this.root.hidden; }

  open() {
    this.root.hidden = false;
    this.label("Hide the cable", true);
    this.ui.list.replaceChildren();
    this.next = this.log.first;
    this.log.onAdd = () => this.schedule();
    this.draw();
    this.renderState();
  }

  close() {
    this.root.hidden = true;
    this.log.onAdd = null;
    this.label("Show the cable", false);
    this.ui.list.replaceChildren();
  }

  /** The header button says what a click does next. */
  label(text, open) {
    const b = this.toggleButton;
    if (!b) return;
    b.setAttribute("aria-expanded", String(open));
    b.setAttribute("aria-label", text);
    b.classList.toggle("on", open);
    const span = b.querySelector("span");
    if (span) span.textContent = text;
  }

  setPaused(paused) {
    this.paused = paused;
    this.ui.pause.setAttribute("aria-pressed", String(paused));
    this.ui.pause.querySelector("span").textContent = paused ? "Go on" : "Pause";
    this.renderState();
    if (!paused) this.schedule();
  }

  renderState() {
    const waiting = Math.max(this.log.seq - Math.max(this.next, this.log.first), 0);
    this.ui.state.textContent = this.paused ? (waiting ? `Paused: ${waiting} new ${waiting === 1 ? "line" : "lines"}` : "Paused") : "";
  }

  clear() {
    this.log.clear();
    this.next = this.log.seq;
    this.ui.list.replaceChildren();
    this.renderState();
  }

  async copy() {
    const text = this.log.text();
    if (!text) {
      this.say("The log is empty.");
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      const n = this.log.size;
      this.say(`The log is copied: ${n} ${n === 1 ? "line" : "lines"}.`);
    } catch (err) {
      this.say(`The log could not be copied: ${err?.message ?? err}`, { error: true });
    }
  }

  /** Draw at the next animation frame (once, however many lines come before it). */
  schedule() {
    if (this.paused) {
      // The count of what waits: once per frame, not once per packet.
      if (this.statePending) return;
      this.statePending = true;
      this.frame(() => {
        this.statePending = false;
        this.renderState();
      });
      return;
    }
    if (this.pending) return;
    this.pending = true;
    this.frame(() => {
      this.pending = false;
      if (this.isOpen && !this.paused) this.draw();
    });
  }

  /** Append the lines since the last drawn; drop the rows the log no longer keeps; follow the end unless scrolled up. */
  draw() {
    const list = this.ui.list;
    const scroller = this.ui.list.parentElement;
    const atEnd = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 24;
    const lines = this.log.since(this.next);
    this.next = this.log.seq;
    if (lines.length) {
      const frag = document.createDocumentFragment();
      for (const l of lines) frag.append(lineItem(l));
      list.append(frag);
    }
    const first = this.log.first;
    while (list.firstElementChild && Number(list.firstElementChild.dataset.n) < first) list.firstElementChild.remove();
    if (atEnd) scroller.scrollTop = scroller.scrollHeight;
    this.renderState();
  }
}
