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
 * "none" | "ask" | "loading" | "error" | "shown", object?, error?}`;
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
      el("p", { class: "preview-meta", text: `${entry.kind} · ${sizeCell(entry.size)} bytes · ${checksumCell(entry.checksum)}` })),
    ...actions);
  const body = el("div", { class: "preview-body" });
  switch (state.status) {
    case "none":
      body.append(el("p", { class: "muted", text: entry.directory ? "A directory: open it to see what it holds." : "Nothing to preview for this kind of object; get it as a file." }));
      break;
    case "ask": {
      const b = el("button", { type: "button", class: "preview-show" }, iconEl("search", "ic-sm"), el("span", { text: "Show what it holds" }));
      b.addEventListener("click", () => onShow?.());
      body.append(el("p", { class: "muted", text: "Larger objects are read only when asked: it takes a while at 9600 baud." }), b);
      break;
    }
    case "loading":
      body.append(el("p", { class: "muted", text: "Reading…" }));
      break;
    case "error":
      body.append(el("p", { class: "preview-error", text: `It cannot be shown: ${state.error}` }));
      break;
    default: {
      const room = Math.max(box.clientWidth - 40, 131);
      body.append(...previewBody(previewOf(state.object), room).filter(Boolean));
    }
  }
  box.replaceChildren(head, body);
}
