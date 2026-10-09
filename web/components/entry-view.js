// One command's reference entry as DOM: name, the models that have it,
// stack effect, our description, where the ROM and the manuals put it,
// the examples generated on the emulator (each with "Try it"), the deep
// links into the manuals. Shared by the palette's detail pane and the
// explorer's Commands tab; it renders what web/reference.js reads.

import { exampleResult, exampleText, manualLinks, placement, stackVerified } from "../reference.js";
import { MODEL_TITLES } from "./sat-calculator.js";

export function el(tag, attrs = {}, ...children) {
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

/** "48SX" from "48sx". */
export const shortTitle = (m) => (MODEL_TITLES[m] ?? m).replace(/^HP /, "");

function link(url, text) {
  return el("a", { href: url, target: "_blank", rel: "noopener noreferrer", text });
}

/** The command's stack effect, with the calculator's arrow spaced as the data writes it. */
function stackBox(command) {
  const verified = stackVerified(command);
  return el("div", { class: "entry-stack-wrap" },
    el("pre", { class: "entry-stack", text: command.stack || "—" }),
    verified ? null : el("p", { class: "entry-note", text: "From the manuals; not checked by an example here." }));
}

/**
 * The entry of `command` from `index` (the shown model's). `ctx`:
 * `legends` (the skin's keys, for the keyboard placement), `onTry(example)`
 * (or null when nothing can be sent, with `whyNot`), `tried` (`{example,
 * text, error}` of the last "try it", to show its outcome).
 */
export function entryView(index, command, ctx = {}) {
  const data = index.data;
  const model = index.model;
  const where = placement(index, command, ctx.legends ?? null);
  const links = manualLinks(index, command, [model]);
  const others = manualLinks(index, command).filter((l) => !links.some((x) => x.id === l.id));
  const head = el("div", { class: "entry-head" },
    el("h3", { class: "entry-name", text: command.name }),
    el("div", { class: "entry-models", "aria-label": "Models with this command" },
      ...data.models.map((m) => {
        const has = Boolean(command.entry.models[m]);
        return el("span", {
          class: `chip${m === model ? " current" : ""}${has ? "" : " off"}`,
          title: has ? (m === model ? `${MODEL_TITLES[m]} (shown)` : MODEL_TITLES[m]) : `Not on the ${MODEL_TITLES[m]}`,
          text: shortTitle(m),
        });
      })));

  const whereRows = [];
  if (where.menus.length) {
    whereRows.push(el("dt", { text: "Menu" }), el("dd", {},
      ...where.menus.flatMap((m, i) => [i ? el("span", { class: "sep", text: " · " }) : null, el("span", { class: "menu-path", text: m })])));
  }
  if (where.key) {
    const k = where.key;
    const src = `${k.title}${k.page ? `, p. ${k.page}` : ""}`;
    whereRows.push(el("dt", { text: "Key" }), el("dd", {},
      el("span", { class: "menu-path", text: k.category }), " ",
      el("span", { class: "muted" }, "· ", k.url ? link(k.url, src) : src, k.other ? " · from another model's manual" : "")));
  }
  if (where.keyboard) {
    whereRows.push(el("dt", { text: "Key" }), el("dd", {}, el("span", { class: "menu-path", text: where.keyboard }), " ",
      el("span", { class: "muted", text: "(from the key labels)" })));
  }
  if (where.group) {
    whereRows.push(el("dt", { text: "Group" }), el("dd", {}, where.group, " ",
      el("span", { class: "muted", text: "(our grouping, not a calculator menu)" })));
  }

  const examples = command.per.examples ?? [];
  const exList = examples.length
    ? el("ol", { class: "entry-examples" }, ...examples.map((x, i) => exampleRow(x, i, ctx)))
    : null;
  const skip = !examples.length && command.per.skip
    ? el("p", { class: "entry-skip", text: `No example: ${command.per.skip}.` })
    : null;

  return el("div", { class: "entry" },
    head,
    stackBox(command),
    el("p", { class: "entry-desc", text: command.description }),
    whereRows.length ? el("dl", { class: "entry-where" }, ...whereRows) : null,
    el("h4", {}, "Examples", el("span", { class: "muted", text: examples.length ? `, run on the emulated ${MODEL_TITLES[model]}` : "" })),
    exList,
    skip,
    !examples.length && !skip ? el("p", { class: "entry-skip", text: "No example on this model." }) : null,
    links.length || others.length
      ? el("div", { class: "entry-manuals" },
        el("h4", { text: "In the manuals" }),
        el("ul", {},
          ...links.map((l) => el("li", {}, link(l.url, `${l.title}, p. ${l.page}`))),
          ...others.map((l) => el("li", { class: "muted" }, link(l.url, `${l.title}, p. ${l.page}`), ` (${l.models.map(shortTitle).join(", ")})`))))
      : null);
}

function exampleRow(x, i, ctx) {
  const r = exampleResult(x);
  const out = r.error
    ? el("div", { class: "ex-out error" }, el("span", { class: "ex-level", text: "Error" }), el("span", { class: "ex-text", text: r.error }))
    : el("div", { class: "ex-out" }, ...(r.lines.length
      ? r.lines.map((line, j) => el("div", { class: "ex-line" },
        el("span", { class: "ex-level", text: `${r.lines.length - j}:` }), el("span", { class: "ex-text", text: line })))
      : [el("span", { class: "muted ex-empty", text: "an empty stack" })]));
  const text = exampleText(x);
  const tried = ctx.tried?.example === x ? ctx.tried : null;
  let tryBtn = null;
  if (ctx.onTry) {
    tryBtn = el("button", { type: "button", class: "ex-try", title: `Type “${text}” and press ENTER`, text: "Try it" });
    tryBtn.addEventListener("click", (e) => {
      if (e.detail > 0) tryBtn.blur();
      ctx.onTry(x);
    });
  } else if (ctx.whyNot) {
    tryBtn = el("button", { type: "button", class: "ex-try", disabled: true, title: ctx.whyNot, text: "Try it" });
  }
  return el("li", { class: "ex", "data-example": i },
    el("div", { class: "ex-in" },
      el("code", { class: "ex-text", text: text }),
      x.effect ? el("span", { class: "ex-effect", text: x.effect }) : null),
    el("div", { class: "ex-arrow", "aria-hidden": "true", text: "→" }),
    out,
    tryBtn,
    tried ? el("p", { class: `ex-tried${tried.error ? " error" : ""}`, text: tried.text }) : null);
}
