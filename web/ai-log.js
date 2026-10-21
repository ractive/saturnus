// "The AI's calls" (iteration 56): what the AI apps do through AI access
// (web/ai.js), for those who want to watch: every tool call with its
// arguments and how it ended, and the connection's events. `AiLog` keeps
// the last 200 entries in memory (nothing stored: empty after a reload);
// `AiLogView` is the drawer in Calculator (index.html `#ai-log`), closed
// by default, open or closed kept per browser. Made with AI access, so it
// holds what happened from the start; never the address, a token or a
// pairing code (the events carry none). Text only, never HTML.

import { el } from "./shared/dom.js";

/** An AI app's name for the sidebar and the log: its own, else its host. */
export function appLabel(client) {
  return client?.name || client?.host || "AI app";
}

/**
 * What Calculator's sidebar says for AI access `a` (web/app.js
 * `watchAi`): the entry's text, its dot (`on` the accent, `lost` the
 * warning colour), the actions below it (`show`, `connected`, `lost`) or
 * a note (why it went off), and the sentence the live region reads.
 */
export function aiView(a) {
  const state = a.state;
  if (a.lost) return { entry: "AI access lost · reconnecting…", dot: "lost", actions: "lost", said: "AI access lost. Reconnecting." };
  if (state === "connecting") return { entry: "AI access: connecting…", said: "AI access: connecting." };
  if (state === "connected") {
    const grants = a.mcp?.grants ?? [];
    if (!grants.length) return { entry: "AI access on · no app yet", actions: "show", said: "AI access on. No app connected yet." };
    const entry = grants.length === 1 ? `${appLabel(grants[0].client)} is connected` : `${grants.length} AI apps are connected`;
    return { entry, dot: "on", actions: "connected", said: `${entry}.` };
  }
  return { entry: "Connect your AI app…", note: a.error ?? null, said: a.error ? `AI access is off. ${a.error}` : "" };
}

/** How many entries are kept. */
export const LIMIT = 200;
/** Lines of a result shown before the rest is folded. */
const HEAD_LINES = 3;
/** Characters of a value or a result line shown before the rest is folded. */
const HEAD_CHARS = 160;

/** 13:05:42, local time. */
export function clockText(t) {
  const d = new Date(t);
  return [d.getHours(), d.getMinutes(), d.getSeconds()].map((n) => String(n).padStart(2, "0")).join(":");
}

/** 85 ms, 1.2 s. */
export function durationText(ms) {
  if (typeof ms !== "number" || !Number.isFinite(ms)) return "";
  return ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(1)} s`;
}

/** A tool call's arguments as `name: value` pairs; strings as they are (RPL text), the rest as JSON. */
export function argPairs(args) {
  return Object.entries(args ?? {}).map(([k, v]) => [k, typeof v === "string" ? v : JSON.stringify(v)]);
}

/** A tool result's text: its text blocks, an image or other block named, else its structured content. */
export function resultText(result) {
  const parts = [];
  for (const c of result?.content ?? []) {
    if (c?.type === "text") parts.push(String(c.text ?? ""));
    else if (c?.type === "image") parts.push(`[image, ${c.mimeType ?? "unknown type"}]`);
    else parts.push(`[${c?.type ?? "unknown"}]`);
  }
  if (!parts.length && result?.structuredContent) parts.push(JSON.stringify(result.structuredContent));
  return parts.join("\n");
}

/** A connection event in words. */
export function noteText(d) {
  switch (d.what) {
    case "connected": return `Connected to the relay ${d.host}`.trim();
    case "lost": return `Connection lost. ${d.text ?? ""}`.trim();
    case "retrying": return `Retrying to connect… (attempt ${d.attempt})`;
    case "wait": return `Next try in ${d.seconds} s`;
    case "paired": return `${d.app} connected`;
    case "unpaired": return `${d.app} disconnected`;
    case "off": return "AI access turned off";
    default: return String(d.what);
  }
}

/** How a call ended, in a word or two (null while it runs). */
export function outcomeText(e) {
  switch (e.outcome) {
    case "ok": return "done";
    case "error": return "error";
    case "declined": return "declined";
    case "cancelled": return "cancelled";
    default: return null;
  }
}

/** The entries of AI access `ai` (its `toolcall`, `toolcallend` and `note` events), the last `limit`. Events: `change`. */
export class AiLog extends EventTarget {
  constructor(ai, { now = () => Date.now(), limit = LIMIT } = {}) {
    super();
    this.now = now;
    this.limit = limit;
    this.entries = [];
    /** The number the next entry gets. */
    this.seq = 0;
    /** The calls not ended yet, by id. */
    this.pending = new Map();
    this.asks = (tool) => Boolean(ai.consequential?.has(tool));
    ai.addEventListener("toolcall", (e) => this.call(e.detail));
    ai.addEventListener("toolcallend", (e) => this.end(e.detail));
    ai.addEventListener("note", (e) => this.add({ kind: "event", text: noteText(e.detail), bad: e.detail.what === "lost" }));
  }

  add(entry) {
    entry.n = this.seq++;
    entry.t = this.now();
    this.entries.push(entry);
    // The oldest connection event goes first, so a long outage's retries do not push the calls out.
    const dropped = [];
    while (this.entries.length > this.limit) {
      const i = this.entries.findIndex((e, j) => e.kind === "event" && j < this.entries.length - 1);
      const [gone] = this.entries.splice(i < 0 ? 0 : i, 1);
      if (gone.kind === "call") this.pending.delete(gone.id);
      dropped.push(gone.n);
    }
    this.dispatchEvent(new CustomEvent("change", { detail: { entry, dropped } }));
  }

  call(d) {
    const entry = { kind: "call", id: d.id, tool: d.tool, title: d.title || d.tool, app: appLabel(d.client), args: argPairs(d.arguments), asks: this.asks(d.tool), outcome: null };
    this.pending.set(d.id, entry);
    this.add(entry);
  }

  end(d) {
    const entry = this.pending.get(d.id);
    if (!entry) return;
    this.pending.delete(d.id);
    entry.outcome = d.outcome;
    entry.result = d.outcome === "declined" || d.outcome === "cancelled" ? "" : resultText(d.result);
    entry.durationMs = d.durationMs;
    this.dispatchEvent(new CustomEvent("change", { detail: { entry, ended: true } }));
  }

  clear() {
    this.entries = [];
    this.pending.clear();
    this.dispatchEvent(new CustomEvent("change", { detail: { cleared: true } }));
  }

  /** The log as text (Copy log): one block per entry, nothing folded. */
  text() {
    const pad = " ".repeat(10);
    return this.entries.map((e) => {
      const head = `${clockText(e.t)}  `;
      if (e.kind === "event") return head + e.text;
      const lines = [`${head}${e.app}  ${e.title}  ${outcomeText(e) ?? "running"}${e.durationMs !== undefined ? `, ${durationText(e.durationMs)}` : ""}`];
      for (const [k, v] of e.args) lines.push(`${pad}${k}: ${v.replace(/\n/g, `\n${pad}  `)}`);
      if (e.result) lines.push(`${pad}→ ${e.result.replace(/\n/g, `\n${pad}  `)}`);
      return lines.join("\n");
    }).join("\n");
  }
}

/**
 * How `folded` splits `text`: the head shown, the rest folded, and the
 * button's label. Whole hidden lines are counted only when the head ends at
 * a line break; a head cut inside a long line says just "more".
 */
export function foldOf(text) {
  const lines = text.split("\n");
  let head = lines.slice(0, HEAD_LINES).join("\n");
  const cut = head.length > HEAD_CHARS;
  if (cut) head = head.slice(0, HEAD_CHARS);
  const n = cut ? 0 : lines.length - HEAD_LINES;
  return { head, rest: text.slice(head.length), more: n > 0 ? `${n} more ${n === 1 ? "line" : "lines"}` : "more" };
}

/** `text` with its first lines shown and the rest folded behind a button ("12 more lines"), or as it is when short. */
export function folded(text, cls) {
  const { head, rest, more: label } = foldOf(text);
  const box = el("span", { class: cls }, el("span", { class: "al-head", text: head }));
  if (!rest) return box;
  const more = el("button", { type: "button", class: "al-more", text: label });
  more.addEventListener("click", () => {
    const shown = el("span", { class: "al-rest", text: rest });
    more.replaceWith(shown);
  });
  box.append(more);
  return box;
}

/** One entry as a list item; `waiting` whether its confirmation is open. */
export function entryItem(e, waiting = false) {
  if (e.kind === "event") {
    return el("li", { class: `al-entry al-event${e.bad ? " bad" : ""}`, "data-n": String(e.n) },
      el("span", { class: "al-t", text: clockText(e.t) }),
      el("span", { class: "al-text", text: e.text }));
  }
  const word = outcomeText(e) ?? (waiting ? "waiting for you" : "running");
  const dur = e.durationMs !== undefined ? ` · ${durationText(e.durationMs)}` : "";
  const li = el("li", { class: `al-entry al-call${e.outcome === "error" ? " bad" : ""}${e.outcome ? "" : " pending"}`, "data-n": String(e.n), "data-id": String(e.id) },
    el("span", { class: "al-t", text: clockText(e.t) }),
    el("span", { class: "al-app", text: e.app }),
    el("span", { class: "al-tool", text: e.title }),
    el("span", { class: `al-out${waiting && !e.outcome ? " waiting" : ""}`, text: word + dur }));
  if (e.args.length) {
    li.append(el("span", { class: "al-args" }, ...e.args.map(([k, v]) => el("span", { class: "al-arg" }, el("span", { class: "al-key", text: `${k}: ` }), folded(v, "al-val")))));
  }
  if (e.result) li.append(folded(e.result, "al-result"));
  return li;
}

/** The stage's width from which the drawer goes beside the calculator, not over it. */
export const BESIDE_MIN = 820;
/** The gap around the drawer beside the calculator, in CSS pixels. */
const GAP = 12;

/**
 * Where the drawer goes on a stage of `stage` (a rect): beside the
 * calculator, in the stage's right part (`width` 42% of the stage, 360
 * to 640 px, its height from under the stage's tools, `below`, to the
 * bottom), when the stage is at least `BESIDE_MIN` wide and the page not
 * a phone's; else null (a sheet along the bottom, style.css).
 */
export function besideBox(stage, { phone = false, below = stage.top } = {}) {
  if (phone || stage.width < BESIDE_MIN) return null;
  const width = Math.round(Math.min(640, Math.max(360, stage.width * 0.42)));
  const top = Math.round(Math.max(stage.top, below) + GAP);
  return { left: Math.round(stage.right - width - GAP), top, width, height: Math.round(stage.bottom - GAP - top) };
}

/**
 * The drawer: `root` the section (index.html `#ai-log`), `log` the
 * AiLog, `say(text, {error})` a toast, `prefs` the page's preferences
 * (open or closed under `aiLog`), `doc` for the confirmation dialogs
 * (a call is "waiting for you" while tab-mcp's asks about it).
 */
export class AiLogView {
  constructor(root, { log, say = () => {}, prefs = null, doc = document }) {
    this.root = root;
    this.log = log;
    this.say = say;
    this.prefs = prefs;
    this.doc = doc;
    this.opener = null;
    const q = (s) => root.querySelector(s);
    this.ui = { list: q(".al-list"), empty: q(".al-empty"), copy: q(".al-copy"), clear: q(".al-clear"), close: q(".al-close"), scroll: q(".al-scroll") };
    this.ui.close.addEventListener("click", () => {
      this.close();
      if (this.opener?.isConnected) this.opener.focus();
    });
    this.ui.clear.addEventListener("click", () => log.clear());
    this.ui.copy.addEventListener("click", () => this.copy());
    log.addEventListener("change", (e) => {
      if (this.isOpen) this.update(e.detail);
    });
    // tab-mcp's confirmation is a dialog of its own at the end of the body: watched while open.
    this.watch = typeof MutationObserver === "function" ? new MutationObserver(() => this.refreshPending()) : null;
    // Beside the calculator where the stage has room: the stage keeps that part free (its padding), so the calculator fits in the rest.
    this.view = root.closest("#calculator-view");
    this.stage = this.view?.querySelector("#stage") ?? null;
    if (this.stage && typeof ResizeObserver === "function") new ResizeObserver(() => this.place()).observe(this.stage, { box: "border-box" });
    globalThis.addEventListener?.("resize", () => this.place());
  }

  get isOpen() { return !this.root.hidden; }

  /** Open it; `from` gets the focus back when it closes. */
  open({ from = null, remember = true } = {}) {
    if (from) this.opener = from;
    this.root.hidden = false;
    if (remember) this.prefs?.set("aiLog", "open");
    this.watch?.observe(this.doc.body, { childList: true });
    this.place();
    this.render();
  }

  /** Beside the calculator or along the bottom (`besideBox`), as the stage's size says now. */
  place() {
    const tools = this.stage?.querySelector(".stage-tools")?.getBoundingClientRect();
    const box = this.isOpen && this.stage ? besideBox(this.stage.getBoundingClientRect(), { phone: matchMedia("(max-width: 759px)").matches, below: tools?.height ? tools.bottom : undefined }) : null;
    this.view?.classList.toggle("ai-log-beside", Boolean(box));
    const s = this.root.style;
    if (box) {
      this.view.style.setProperty("--al-w", `${box.width + 2 * GAP}px`);
      for (const k of ["left", "top", "width", "height"]) s.setProperty(k, `${box[k]}px`);
      s.setProperty("right", "auto");
      s.setProperty("bottom", "auto");
    } else {
      this.view?.style.removeProperty("--al-w");
      for (const k of ["left", "top", "width", "height", "right", "bottom"]) s.removeProperty(k);
    }
  }

  close() {
    this.root.hidden = true;
    this.place();
    this.prefs?.remove("aiLog");
    this.watch?.disconnect();
    this.ui.list.replaceChildren();
  }

  toggle(opts) {
    if (this.isOpen) this.close();
    else this.open(opts);
  }

  /** The title of the call tab-mcp's confirmation asks about now, or null. */
  asking() {
    const d = this.doc.querySelector("dialog.tab-mcp-confirm");
    const m = /^Allow (.*)\?$/.exec(d?.querySelector("h2, h1, [id$='-title']")?.textContent ?? "");
    return d ? (m ? m[1] : "") : null;
  }

  /** Whether entry `e` waits for the user: it asks first, has not ended, and the open confirmation is about it (the oldest such call). */
  waiting(e, asking = this.asking()) {
    if (asking === null || e.kind !== "call" || e.outcome || !e.asks) return false;
    for (const p of this.log.pending.values()) {
      if (p.asks && (asking === "" || p.title === asking)) return p === e;
    }
    return false;
  }

  render() {
    const asking = this.asking();
    this.ui.list.replaceChildren(...this.log.entries.map((e) => entryItem(e, this.waiting(e, asking))));
    this.ui.empty.hidden = this.log.entries.length > 0;
    this.ui.scroll.scrollTop = this.ui.scroll.scrollHeight;
  }

  /** A new entry appended (and the ones the log dropped removed), an ended call redrawn in place, or all again. */
  update({ entry, ended, cleared, dropped } = {}) {
    if (cleared || !entry) return this.render();
    const sc = this.ui.scroll;
    const atEnd = sc.scrollHeight - sc.scrollTop - sc.clientHeight < 24;
    const list = this.ui.list;
    if (ended) {
      list.querySelector(`[data-n="${entry.n}"]`)?.replaceWith(entryItem(entry));
      this.refreshPending();
    } else {
      list.append(entryItem(entry, this.waiting(entry)));
      for (const n of dropped ?? []) list.querySelector(`[data-n="${n}"]`)?.remove();
    }
    this.ui.empty.hidden = this.log.entries.length > 0;
    if (atEnd) sc.scrollTop = sc.scrollHeight;
  }

  /** The running calls' "running" or "waiting for you" again (a confirmation opened or closed). */
  refreshPending() {
    if (!this.isOpen) return;
    const asking = this.asking();
    for (const e of this.log.pending.values()) {
      const li = this.ui.list.querySelector(`[data-n="${e.n}"]`);
      const out = li?.querySelector(".al-out");
      if (!out) continue;
      const w = this.waiting(e, asking);
      out.textContent = w ? "waiting for you" : "running";
      out.classList.toggle("waiting", w);
    }
  }

  async copy() {
    const text = this.log.text();
    if (!text) {
      this.say("The log is empty.");
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      const n = this.log.entries.length;
      this.say(`The log is copied: ${n} ${n === 1 ? "entry" : "entries"}.`);
    } catch (err) {
      this.say(`The log could not be copied: ${err?.message ?? err}`, { error: true });
    }
  }
}
