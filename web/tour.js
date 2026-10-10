// The tour of the page (kb iteration 35): its chapters and steps as data,
// which steps apply where, the words with their keys filled in, and a
// run through one chapter that skips a step whose anchor is not shown.
// No DOM: `<sat-tour>` (components/sat-tour.js) shows the steps, and
// web/test/tour.test.mjs tests this file.
//
// A step is `{id, anchor, title, text, setup, when}`: `anchor` a CSS
// selector of the element it rings, `text` one or two plain sentences
// (`{key:palette}` becomes " (⌘K)", the binding's label, or nothing),
// `setup` what must be open for the anchor to be seen (the page's
// `SETUPS`, which only open UI and never touch the calculator), `when`
// the conditions it shows under (host, pointer, phone, Mac). The list is
// plain data, so a later click-through recorder can walk it too.

import { WRITABLE_MODELS } from "./norom.js";

/**
 * What a setup opens. `panel`: the controls (the side panel, or the
 * phone's sheet); `roms`: and the "ROMs of every model" list in it;
 * `calculator`: the calculator in sight (the sheet closed, the memory
 * view closed where it would lie over it); `layer:<tab>`: the memory view
 * on that tab.
 */
export const SETUPS = ["panel", "roms", "calculator", "layer:vars", "layer:stack", "layer:flags"];

/** The tour's chapters; `needs: "memory"`: offered while a model with a memory view runs. */
export const CHAPTERS = [
  {
    id: "start",
    title: "Getting started",
    steps: [
      {
        id: "panel",
        anchor: "#panel .panel-head",
        title: "The controls",
        text: "The side panel holds the model and its ROM, Reset, saved states, the speed and the theme. ‹ hides it, ☰ brings it back.",
        setup: "panel",
        when: { phone: false },
      },
      {
        id: "sheet",
        anchor: "#bar-menu",
        title: "The controls",
        text: "☰ opens the controls: the model and its ROM, Reset, saved states, the speed and the theme.",
        setup: "calculator",
        when: { phone: true },
      },
      {
        id: "roms",
        anchor: "#roms > summary",
        title: "ROMs",
        text: "Each model runs HP's own ROM, which is not part of saturnus. This list shows which models have one.",
        setup: "roms",
      },
      {
        id: "download",
        anchor: "[data-tour=rom-download]",
        title: "Download",
        text: "Download opens the model's page on hpcalc.org, which hosts the ROMs with HP's permission. Download the zip there and unzip it.",
        setup: "roms",
        when: { host: "browser" },
      },
      {
        id: "download-app",
        anchor: ".rom-row-button[data-rom-act=download]",
        title: "Download",
        text: "Download… fetches the model's ROM from hpcalc.org, which hosts it with HP's permission, after asking first.",
        setup: "roms",
        when: { host: "app" },
      },
      {
        id: "choose",
        anchor: "#rom-pick",
        title: "Choose the file",
        text: "Choose… picks the ROM file, or drop it on the page. It is kept in this browser and never uploaded.",
        setup: "panel",
        when: { host: "browser" },
      },
      {
        id: "choose-app",
        anchor: "#rom-pick",
        title: "Choose the file",
        text: "Choose… picks a ROM file you already have. The app remembers where it is.",
        setup: "panel",
        when: { host: "app" },
      },
      {
        id: "search",
        anchor: "#palette-show",
        title: "Search",
        text: "Search{key:palette} finds the calculator's commands, your variables and every action of the page, and sends text to the calculator.",
        setup: "calculator",
        when: { phone: false },
      },
      {
        id: "search-phone",
        anchor: "#bar-palette",
        title: "Search",
        text: "Search finds the calculator's commands, your variables and every action of the page, and sends text to the calculator.",
        setup: "calculator",
        when: { phone: true },
      },
      {
        id: "shortcuts",
        anchor: "#shortcuts",
        title: "Keyboard shortcuts",
        text: "Keyboard shortcuts lists every key. There you can change the keys for ON, α, the shifts and the page's actions.",
        setup: "panel",
        when: { pointer: "fine" },
      },
      {
        id: "again",
        anchor: "#about",
        title: "The tour again",
        text: "Show me around, in About or in Search, starts this tour again.",
        setup: "panel",
      },
    ],
  },
  {
    id: "calculator",
    title: "The calculator",
    steps: [
      {
        id: "keys",
        anchor: "sat-calculator .skin",
        title: "Keys",
        text: "Click the keys, or type: letters, digits, Enter and the arrows go to the calculator. Esc is ON.",
        setup: "calculator",
        when: { pointer: "fine" },
      },
      {
        id: "keys-touch",
        anchor: "sat-calculator .skin",
        title: "Keys",
        text: "Tap the keys. A finger held on a key holds it down.",
        setup: "calculator",
        when: { pointer: "coarse" },
      },
      {
        id: "paste",
        anchor: "sat-calculator canvas",
        title: "Paste",
        text: "Paste ({mod}V) types the clipboard into the calculator's command line, key by key.",
        setup: "calculator",
        when: { pointer: "fine" },
      },
      {
        id: "shift-click",
        anchor: "sat-calculator .skin",
        title: "Shifted functions",
        text: "Ctrl+click a key for its left-shifted function, {alt}+click for its right-shifted one. Hold Ctrl or {alt} to see what they reach.",
        setup: "calculator",
        when: { pointer: "fine" },
      },
      {
        id: "edit",
        anchor: "#cmdline-edit",
        title: "Edit",
        text: "Edit{key:edit} opens the command line, or stack level 1, in a full editor. Send back puts the text in the calculator.",
        setup: "calculator",
        when: { phone: false },
      },
      {
        id: "edit-phone",
        anchor: "#bar-edit",
        title: "Edit",
        text: "Edit opens the command line, or stack level 1, in a full editor. Send back puts the text in the calculator.",
        setup: "calculator",
        when: { phone: true },
      },
      {
        id: "screen",
        anchor: "#panel .screen-actions",
        title: "Screen images",
        text: "Copy screen and Save screen take the display as a PNG image. A right-click on the display offers both.",
        setup: "panel",
        when: { pointer: "fine" },
      },
      {
        id: "screen-touch",
        anchor: "#panel .screen-actions",
        title: "Screen images",
        text: "Copy screen and Save screen take the display as a PNG image. A long press on the display offers both.",
        setup: "panel",
        when: { pointer: "coarse" },
      },
      {
        id: "fullscreen",
        anchor: "#panel #fullscreen",
        title: "Fullscreen",
        text: "Fullscreen{key:fullscreen} shows the calculator alone, edge to edge. The ✕ in its corner leaves it.",
        setup: "panel",
        when: { phone: false },
      },
      {
        id: "fullscreen-phone",
        anchor: "#bar-fullscreen",
        title: "Fullscreen",
        text: "Fullscreen shows the calculator alone, edge to edge. A swipe down on the display opens Search there.",
        setup: "calculator",
        when: { phone: true },
      },
      {
        id: "memory",
        anchor: "#layer-show",
        title: "The memory view",
        text: "The memory view shows the calculator's variables, stack and flags while it runs, and the command reference.",
        setup: "calculator",
        when: { phone: false },
      },
      {
        id: "memory-phone",
        anchor: "#bar-memory",
        title: "The memory view",
        text: "The memory view shows the calculator's variables, stack and flags while it runs, and the command reference.",
        setup: "calculator",
        when: { phone: true },
      },
    ],
  },
  {
    id: "memory",
    title: "The memory view",
    needs: "memory",
    steps: [
      {
        id: "vars",
        anchor: "#tab-vars",
        title: "Variables",
        text: "Variables shows the calculator's directories and the variables in them, read live.",
        setup: "layer:vars",
      },
      {
        id: "tree",
        anchor: "#pane-vars .tree",
        title: "Directories",
        text: "Browsing here does not change the calculator's directory; \"current\" marks the one it is in. Double-click a directory to open it.",
        setup: "layer:vars",
        when: { pointer: "fine" },
      },
      {
        id: "tree-touch",
        anchor: "#pane-vars .tree",
        title: "Directories",
        text: "Browsing here does not change the calculator's directory; \"current\" marks the one it is in. Tap a directory to open it.",
        setup: "layer:vars",
        when: { pointer: "coarse" },
      },
      {
        id: "preview",
        anchor: "#pane-vars .preview-head",
        title: "What can be done",
        text: "The first button does the next step: Edit a variable, Open a directory, or Make current, which changes the calculator's directory. ⋯ holds the rest.",
        setup: "layer:vars",
      },
      {
        id: "list",
        anchor: "#pane-vars .list-wrap",
        title: "Editing a variable",
        text: "Double-click a variable to edit it. Save stores the changed text back in the calculator.",
        setup: "layer:vars",
        when: { pointer: "fine" },
      },
      {
        id: "list-touch",
        anchor: "#pane-vars .list-wrap",
        title: "Editing a variable",
        text: "Tap a variable, then Edit. Save stores the changed text back in the calculator.",
        setup: "layer:vars",
        when: { pointer: "coarse" },
      },
      {
        id: "stack",
        anchor: "#pane-stack .levels-wrap",
        title: "Stack",
        text: "Stack shows the levels as the calculator has them, level 1 at the bottom. Select a level and press Edit to change it.",
        setup: "layer:stack",
      },
      {
        id: "flags",
        anchor: "#pane-flags .flags-scroll",
        title: "Flags",
        text: "Flags lists the system flags by topic, with what each state means. A click on a flag sets or clears it.",
        setup: "layer:flags",
      },
      {
        id: "keys-here",
        anchor: "#tab-vars",
        title: "Where the keys go",
        text: "Typing goes to the calculator until you click into the memory view. {key:layerFocus} moves the keys here and back; Esc gives them back.",
        setup: "layer:vars",
        when: { pointer: "fine" },
      },
    ],
  },
];

/**
 * Where the tour runs: `host` "browser" or "app", `pointer` "fine" or
 * "coarse", `phone` (below 760 px), `mac`, `booted` (the running model or
 * null) and `keys` (an action's key label, or "").
 */
export function context({ host = "browser", pointer = "fine", phone = false, mac = false, booted = null, keys = () => "" } = {}) {
  return { host, pointer, phone, mac, booted, keys };
}

/** Whether `step` shows under `ctx` (every condition of its `when` holds). */
export function applies(step, ctx) {
  return Object.entries(step.when ?? {}).every(([k, v]) => ctx[k] === v);
}

/** Whether `chapter` is offered: the memory view's while a model with one runs. */
export function offered(chapter, ctx) {
  return chapter.needs !== "memory" || WRITABLE_MODELS.has(ctx.booted);
}

/** The chapters offered under `ctx`. */
export function chaptersFor(ctx) {
  return CHAPTERS.filter((c) => offered(c, ctx));
}

/** The chapter with `id`, or undefined. */
export function chapter(id) {
  return CHAPTERS.find((c) => c.id === id);
}

/** The chapter offered after `id`, or null. */
export function nextChapter(id, ctx) {
  const list = chaptersFor(ctx);
  const i = list.findIndex((c) => c.id === id);
  return i >= 0 ? list[i + 1] ?? null : null;
}

/**
 * `text` with its placeholders filled: `{key:id}` the binding's label in
 * parentheses (nothing without one, or by touch), `{mod}` ⌘ or Ctrl+,
 * `{alt}` Option or Alt.
 */
export function fillText(text, ctx) {
  return text
    .replace(/\{key:(\w+)\}/g, (_, id) => {
      const k = ctx.pointer === "coarse" ? "" : ctx.keys(id);
      return k ? ` (${k})` : "";
    })
    .replace(/\{mod\}/g, ctx.mac ? "⌘" : "Ctrl+")
    .replace(/\{alt\}/g, ctx.mac ? "Option" : "Alt");
}

/**
 * One chapter's way through: the steps that apply, the one shown, and
 * those dropped on the way because their anchor was not shown (each
 * leaves the count). `show(step)` resolves to whether it could be shown.
 */
export class TourRun {
  constructor(chapterOf, ctx) {
    this.chapter = chapterOf;
    this.steps = chapterOf.steps.filter((s) => applies(s, ctx));
    this.index = -1;
    this.skipped = [];
  }

  get step() {
    return this.steps[this.index] ?? null;
  }

  get total() {
    return this.steps.length;
  }

  /** "3 of 7". */
  get counter() {
    return `${this.index + 1} of ${this.total}`;
  }

  get first() {
    return this.index <= 0;
  }

  get last() {
    return this.index === this.total - 1;
  }

  /**
   * Move by `dir` (1 or -1) to the next step `show` can show; a step it
   * cannot is dropped. Resolves to that step, or null when none is left
   * that way (past the last: the chapter is done; before the first: the
   * step shown stays, shown again).
   */
  async go(dir, show) {
    const from = this.step;
    let i = this.index + dir;
    while (i >= 0 && i < this.steps.length) {
      const s = this.steps[i];
      if (await show(s)) {
        this.index = i;
        return s;
      }
      this.skipped.push(s.id);
      this.steps.splice(i, 1);
      if (dir < 0) i--;
    }
    // Back from the first that can be shown: stay where we were.
    if (dir < 0 && from) {
      this.index = this.steps.indexOf(from);
      if (await show(from)) return from;
    }
    return null;
  }
}

/** The kept answer to the offer (`saturnus.tour`): shown while nothing is kept. */
export const OFFER_KEY = "saturnus.tour";

/** Whether the offer line is shown, given the kept value. */
export function offerShown(kept) {
  return kept !== "dismissed" && kept !== "taken";
}
