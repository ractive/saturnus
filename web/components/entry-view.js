// One command's reference entry as DOM: name (and long name), the models that have it,
// Run and Type for the command itself, stack effect, our description,
// where the ROM and the manuals put it, the examples generated on the
// emulator (each with "Try it"), the deep links into the manuals. Shared by the palette's detail pane and the
// explorer's Commands tab; it renders what web/reference.js reads.

import { exampleBefore, exampleResult, exampleText, legendText, manualLinks, placement, stackVerified } from "../reference.js";
import { MODEL_TITLES } from "../norom.js";
import { el } from "../shared/dom.js";

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
  if (index.keyed) return keyedEntryView(index, command, ctx);
  const data = index.data;
  const model = index.model;
  const where = placement(index, command, ctx.legends ?? null);
  const links = manualLinks(index, command, [model]);
  const others = manualLinks(index, command).filter((l) => !links.some((x) => x.id === l.id));
  const head = el("div", { class: "entry-head" },
    // The mnemonic, then what it stands for where that says more: "RSD · Residual".
    el("h3", { class: "entry-name" }, command.name,
      command.longName ? el("span", { class: "entry-long" }, " ", el("span", { class: "sep", "aria-hidden": "true", text: "· " }), command.longName) : null),
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
    sendRow(command, ctx),
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

/**
 * Run and Type for the command itself, on the calculator's stack as it
 * is (`ctx.onSend(verb)`, verb "run" or "insert"; off with `whyNotSend`),
 * and the outcome of the last one (`ctx.sent`). None in the palette,
 * whose Enter does the same.
 */
function sendRow(command, ctx) {
  if (!ctx.onSend && !ctx.whyNotSend) return null;
  const button = (verb, text, title, primary) => {
    const b = el("button", { type: "button", class: primary ? "primary small" : "small", disabled: !ctx.onSend || ctx.sent?.busy, title: ctx.onSend ? title : ctx.whyNotSend, text });
    b.addEventListener("click", (e) => {
      if (e.detail > 0) b.blur();
      ctx.onSend?.(verb);
    });
    return b;
  };
  return el("div", { class: "entry-send" },
    button("run", "Run", `Type ${command.name} and press ENTER: it works on the stack as it is (Enter in the list)`, true),
    button("insert", "Type", `Type ${command.name} into the command line, without running it (⌘/Ctrl+Enter in the list)`, false),
    ctx.sent ? el("span", { class: `entry-sent${ctx.sent.error ? " error" : ""}`, role: "status", text: ctx.sent.text }) : null);
}

/** Stack levels as the calculator shows them, the deepest first and level 1 last. */
function stackSide(lines, cls, label) {
  return el("div", { class: `ex-stack ${cls}`, role: "group", "aria-label": label }, ...(lines.length
    ? lines.map((line, j) => el("div", { class: "ex-line" },
      el("span", { class: "ex-level", text: `${lines.length - j}:` }), el("span", { class: "ex-text", text: line })))
    : [el("span", { class: "muted ex-empty", text: "empty stack" })]));
}

/**
 * One example as the calculator shows it: the stack before, the command
 * that ran on it, the stack after (or the error); a setup run first as
 * a line above.
 */
function exampleRow(x, i, ctx) {
  const r = exampleResult(x);
  const before = stackSide(exampleBefore(x), "ex-before", "Stack before");
  const after = r.error
    ? el("div", { class: "ex-stack ex-after error", role: "group", "aria-label": "Error" },
      el("span", { class: "ex-level", text: "Error:" }), el("span", { class: "ex-text", text: r.error }))
    : stackSide(r.lines, "ex-after", "Stack after");
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
    x.setup ? el("p", { class: "ex-setup" }, "After ", el("code", { text: x.setup })) : null,
    before,
    el("div", { class: "ex-cmd", title: x.effect || null },
      el("code", { class: "ex-run", text: x.run }),
      el("span", { class: "ex-arrow", "aria-hidden": "true", text: "→" })),
    after,
    tryBtn,
    tried ? el("p", { class: `ex-tried${tried.error ? " error" : ""}`, text: tried.text }) : null);
}

/**
 * A key-driven model's function (the 42S; the 38G, 39G and 40G): name and
 * long name, its keys (and what they put on HOME's edit line), stack
 * effect, what it asks for, our description, its menu, the
 * examples run on the emulator as the screen showed them before and
 * after the keys (each with "Try it", which presses the example's keys
 * and the function's), and the manual's page.
 */
function keyedEntryView(index, command, ctx) {
  const entry = command.entry;
  const km = index.km;
  const model = index.model;
  const head = el("div", { class: "entry-head" },
    el("h3", { class: "entry-name" }, command.name,
      command.longName ? el("span", { class: "entry-long" }, " ", el("span", { class: "sep", "aria-hidden": "true", text: "· " }), command.longName) : null),
    el("div", { class: "entry-models", "aria-label": "Model" },
      el("span", { class: "chip current", title: `${MODEL_TITLES[model] ?? model} (shown)`, text: shortTitle(model) })));
  const keys = command.byName
    ? el("dd", {}, el("kbd", { text: "XEQ" }), " ", el("code", { text: `"${entry.xeq}"` }),
      el("span", { class: "muted", text: " · run by name: XEQ, then the name through the ALPHA menu, then ENTER" }))
    : el("dd", { class: "entry-keys" }, ...(command.steps ?? []).map((s) => el("kbd", { text: s.label ?? (s.key === "shift" ? "⇧" : s.key.toUpperCase()) })));
  // A function typed into an equation (the Solver's) has no keys.
  const typed = !command.steps && !command.byName;
  const where = typed ? [] : [el("dt", { text: "Keys" }), keys];
  // The 38G, 39G and 40G: what the keys put on HOME's edit line.
  if (entry.inserts) where.push(el("dt", { text: "Puts" }), el("dd", {}, el("code", { text: entry.inserts.trimEnd() }), el("span", { class: "muted", text: " on the edit line" })));
  if (entry.param) where.push(el("dt", { text: "Asks for" }), el("dd", { text: entry.param }));
  if (entry.menu) where.push(el("dt", { text: "Menu" }), el("dd", {}, el("span", { class: "menu-path", text: entry.menu })));
  const manual = index.data.manuals?.[km.manual];
  const examples = command.per.examples ?? [];
  return el("div", { class: "entry" },
    head,
    command.stack ? el("div", { class: "entry-stack-wrap" }, el("pre", { class: "entry-stack", text: command.stack })) : null,
    el("p", { class: "entry-desc", text: command.description }),
    el("dl", { class: "entry-where" }, ...where),
    el("h4", {}, "Examples", el("span", { class: "muted", text: examples.length ? `, run on the emulated ${MODEL_TITLES[model]}` : "" })),
    examples.length
      ? el("ol", { class: "entry-examples" }, ...examples.map((x, i) => keyedExampleRow(command, x, i, ctx, Boolean(km.home))))
      : el("p", { class: "entry-skip", text: "No example for this function." }),
    manual && entry.page
      ? el("div", { class: "entry-manuals" }, el("h4", { text: "In the manual" }),
        el("ul", {}, el("li", {}, link(`${manual.url}#page=${entry.page}`, `${manual.title}, p. ${entry.page}`))))
      : null);
}

/** The display's lines as the example showed them. */
function screenLines(lines, cls, label) {
  return el("div", { class: `ex-stack ex-screen ${cls}`, role: "group", "aria-label": label },
    ...lines.map((line) => el("div", { class: "ex-line" }, el("span", { class: "ex-text", text: line || " " }))));
}

/**
 * One example of a key-driven model: the screen after its input keys, the
 * function's keys (and the `then` keys after them: the arguments and
 * ENTER in HOME), the screen after.
 */
function keyedExampleRow(command, x, i, ctx, home = false) {
  // A menu key in the input reads as the label its box showed (the generator kept them).
  const input = x.labels?.length ? x.labels.join(" ") : legendText(x.input, ctx.legends ?? null);
  const then = x.then?.length ? ` ${legendText(x.then, ctx.legends ?? null)}` : "";
  const tried = ctx.tried?.example === x ? ctx.tried : null;
  let tryBtn = null;
  if (ctx.onTry) {
    tryBtn = el("button", { type: "button", class: "ex-try", title: `Press ${input ? `${input} ` : ""}${command.keysText}${then}`, text: "Try it" });
    tryBtn.addEventListener("click", (e) => {
      if (e.detail > 0) tryBtn.blur();
      ctx.onTry(x);
    });
  } else if (ctx.whyNot) {
    tryBtn = el("button", { type: "button", class: "ex-try", disabled: true, title: ctx.whyNot, text: "Try it" });
  }
  return el("li", { class: "ex", "data-example": i },
    home && !input
      ? el("p", { class: "ex-setup", text: "In HOME, with an empty edit line" })
      : el("p", { class: "ex-setup" }, home ? "In HOME, after " : "After ", el("code", { text: input })),
    // HOME with nothing on it has nothing to show before.
    home && x.before.every((l) => !l.trim()) ? null : screenLines(x.before, "ex-before", "Display before"),
    el("div", { class: "ex-cmd" },
      el("code", { class: "ex-run", text: command.keysText + then }),
      el("span", { class: "ex-arrow", "aria-hidden": "true", text: "→" })),
    screenLines(x.after, "ex-after", "Display after"),
    tryBtn,
    tried ? el("p", { class: `ex-tried${tried.error ? " error" : ""}`, text: tried.text }) : null);
}
