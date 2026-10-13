// The preview of a calculator variable: its name, type, size and
// checksum, and what it holds: a GROB as a picture, a data object as
// text, a program as the calculator's text. The object comes decoded by
// the wasm bindings (`describe_file`, a program's text by `text_of`) and
// is shown through the emulator's own preview rules
// (shared/objects.js `previewOf`) and pictures (shared/graphic.js).

import { el } from "../shared/dom.js";
import { checksumCell, sizeCell } from "../shared/device.js";
import { graphicBits, graphicDrawable, graphicRgba, graphicScale, graphicText } from "../shared/graphic.js";
import { previewOf } from "../shared/objects.js";
import { iconEl } from "../shared/components/icons.js";

const grouped = (n) => Math.round(n).toLocaleString("en-US");

/**
 * What a preview being read says (iteration 45): `{line, amount, hint}`
 * for reading `r` (`{name, from, done, total, baud, emulated, waiting,
 * started, at}`, pane.js; `at` the time of the last packet). The amount in bytes of the
 * listing's size; the hint how long it takes: from the link's speed
 * before the first packet (a real cable only: an emulated calculator may
 * run faster than its baud rate), then from the rate measured; said only
 * when it is 2 s or more.
 */
export function readingText(r) {
  if (!r?.from) return { line: `Reading ${r?.name ?? ""}…`.replace(" …", "…"), amount: "", hint: "" };
  if (r.waiting) return { line: `Reading ${r.name} from the ${r.from}…`, amount: `Waiting: ${r.waiting.replace(/…$/, "")} first.`, hint: "" };
  const total = r.total ?? null;
  const done = Math.min(r.done ?? 0, total ?? Infinity);
  const amount = total ? `${grouped(done)} of ${grouped(total)} bytes` : done ? `${grouped(done)} bytes` : "";
  let hint = "";
  if (total) {
    const elapsed = r.started !== null && r.started !== undefined && r.at ? (r.at - r.started) / 1000 : 0;
    let left = null;
    if (done > 0 && elapsed >= 0.5) left = ((total - done) * elapsed) / done;
    else if (!r.emulated && r.baud) left = estimateSeconds(total, r.baud);
    if (left !== null && (left >= 2 || (elapsed > 0 && left >= 1))) {
      const s = Math.max(1, Math.round(left));
      const about = elapsed >= 0.5 ? `about ${s} s left` : `about ${s} s`;
      hint = r.emulated ? `${about[0].toUpperCase()}${about.slice(1)}.` : `Slow over the cable at ${r.baud} baud: ${about}.`;
    }
  }
  return { line: `Reading ${r.name} from the ${r.from}…`, amount, hint };
}

/**
 * How long reading `bytes` takes over Kermit at `baud`: a second for the
 * request and the packets around the data, then the data at about a
 * third of the line's speed (the calculator acknowledges each packet of
 * at most 94 bytes, and each turn costs time; measured on the recorded
 * traces of the emulated 48GX, crates/saturnus-tx/traces).
 */
export function estimateSeconds(bytes, baud) {
  return 1 + (bytes * 10) / (baud * 0.3);
}

/** Fill the reading parts of `box` (a preview being read, from `renderPreview`) for `r`. */
export function updateReading(box, r) {
  const t = readingText(r);
  const q = (s) => box.querySelector(s);
  const line = q(".reading-line");
  if (!line) return;
  if (line.textContent !== t.line) line.textContent = t.line;
  const amount = q(".reading-amount");
  if (amount.textContent !== t.amount) amount.textContent = t.amount;
  const hint = q(".reading-hint");
  if (hint.textContent !== t.hint) hint.textContent = t.hint;
  hint.hidden = !t.hint;
  const bar = q(".reading-bar");
  const fill = bar.firstElementChild;
  if (r.total && !r.waiting) {
    const pct = Math.round(Math.min((r.done ?? 0) / r.total, 1) * 100);
    bar.classList.remove("unknown");
    bar.setAttribute("aria-valuenow", String(pct));
    bar.setAttribute("aria-valuetext", t.amount || `${pct}%`);
    fill.style.width = `${pct}%`;
  } else {
    bar.classList.add("unknown");
    bar.removeAttribute("aria-valuenow");
    bar.setAttribute("aria-valuetext", t.amount || "waiting");
    fill.style.width = "";
  }
}

/** The LCD's colours (web/screenshot.js `LCD_BG`, `LCD_INK`), a picture's off and on. */
const COLORS = { on: [16, 20, 12], off: [183, 194, 162] };

/** The object a text transfer stands for, as `previewOf` takes it: a program, an algebraic, or text. */
export function textObject(kind, text) {
  const source = text.trim();
  if (kind === "Program" || source.startsWith("«")) return { type: "program", source };
  if (kind === "Algebraic") return { type: "algebraic", source };
  return { type: "unknown", source };
}

/** A canvas of graphic `g`, scaled sharp to fit `room` pixels across. */
function picture(g, room) {
  if (!graphicDrawable(g)) return null;
  const rgba = graphicRgba(g, COLORS, 1, graphicBits(g));
  const canvas = el("canvas", { class: "graphic", width: rgba.width, height: rgba.height, role: "img", "aria-label": graphicText(g) });
  canvas.getContext("2d")?.putImageData(new ImageData(rgba.data, rgba.width, rgba.height), 0, 0);
  const scale = graphicScale(g.width, g.height, room);
  canvas.style.width = `${g.width * scale}px`;
  canvas.dataset.scale = String(scale);
  return canvas;
}

/** The body of preview `p` (`previewOf`'s result). */
export function previewBody(p, room = 260) {
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
      return [el("pre", { class: "obj obj-program" }, ...p.lines.map((l) => el("span", { class: "line", style: `--d:${l.depth}`, text: l.text }))), more(p.more, "lines")];
    case "list":
      return [
        p.count
          ? el("ol", { class: "obj-list" }, ...p.items.map((it) => el("li", { class: it.complete ? null : "partial" }, el("span", { class: "obj", text: it.text }), el("span", { class: "muted", text: it.type }))))
          : el("p", { class: "muted", text: "An empty list." }),
        more(p.more, "elements"),
      ];
    case "matrix":
      return [
        el("div", { class: "grid-wrap" }, el("table", { class: "obj-grid" }, el("tbody", {}, ...p.rows.map((row) => el("tr", {}, ...row.map((c) => el("td", { text: c }))))))),
        p.moreRows || p.moreCols ? el("p", { class: "more", text: `${p.dims.join(" × ")} in all` }) : null,
      ];
    case "graphic": {
      const canvas = picture(p.graphic, room);
      return canvas ? [el("div", { class: "graphic-wrap" }, canvas), el("p", { class: "more", text: `${p.graphic.width} × ${p.graphic.height} pixels` })] : [el("p", { class: "unavailable", text: p.reason ?? "An empty picture." })];
    }
    default:
      return [el("p", { class: "unavailable", text: p.reason ?? "Nothing to show." })];
  }
}

/**
 * Fill `box` for `entry` (a listed variable) in `state`: `{status:
 * "none" | "ask" | "loading" | "error" | "shown", object?, error?,
 * reason?}` (`reason`: why a shown object has no text, in place of the
 * shared one);
 * `onShow()` fetches it (the button of a large one), `actions` are
 * buttons for the head.
 */
export function renderPreview(box, entry, state, { onShow = null, actions = [] } = {}) {
  if (!entry) {
    box.hidden = true;
    box.replaceChildren();
    return;
  }
  box.hidden = false;
  const head = el("div", { class: "preview-head" },
    el("div", { class: "preview-title" },
      el("h2", { text: entry.name }),
      el("p", { class: "preview-meta", text: [entry.kind, entry.size === null || entry.size === undefined ? "" : `${sizeCell(entry.size)} bytes`, checksumCell(entry.checksum)].filter(Boolean).join(" · ") })),
    ...actions);
  const body = el("div", { class: "preview-body" });
  switch (state.status) {
    case "none":
      body.append(el("p", { class: "muted", text: state.note ?? (entry.directory ? "A directory: open it to see what it holds." : "Nothing to preview for this kind of object; copy it or download it.") }));
      break;
    case "ask": {
      const b = el("button", { type: "button", class: "preview-show" }, iconEl("search", "ic-sm"), el("span", { text: "Show what it holds" }));
      b.addEventListener("click", () => onShow?.());
      body.append(el("p", { class: "muted", text: state.note ?? "Larger objects are read only when asked: it takes a while at 9600 baud." }), b);
      break;
    }
    case "loading": {
      // Said where the click was: what is read, from where, how far (iteration 45).
      const r = state.reading ?? { name: entry.name };
      const box2 = el("div", { class: "reading", role: "status" },
        el("p", { class: "reading-line" }),
        el("div", { class: "reading-bar unknown", role: "progressbar", "aria-label": `Reading ${entry.name}`, "aria-valuemin": "0", "aria-valuemax": "100" }, el("span", {})),
        el("p", { class: "reading-amount" }),
        el("p", { class: "reading-hint muted", hidden: true }));
      body.append(box2);
      updateReading(box2, r);
      break;
    }
    case "error":
      body.append(el("p", { class: "preview-error", text: `It cannot be shown: ${state.error}` }));
      break;
    default: {
      const room = Math.max(box.clientWidth - 40, 131);
      const p = previewOf(state.object);
      // The page's own reason (a program from a folder without its ROM's names).
      if (state.reason && p.kind === "unavailable") p.reason = state.reason;
      body.append(...previewBody(p, room).filter(Boolean));
    }
  }
  box.replaceChildren(head, body);
}
