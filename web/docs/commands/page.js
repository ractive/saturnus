// The command reference page of the docs (/saturnus/docs/commands/,
// iteration 57, redesigned after the owner's review): one search field
// with the model beside it, its suggestions as you type (the ten best
// by the app's ranking), the chosen command's entry under it, and the
// model's commands to browse by menu (one folded group per menu of the
// calculator, every command a chip) with the whole list A to Z last.
// It runs the app's own code from app/ beside it on the site: the data
// (commands.json) and its loader (palette.js's ReferenceLoader), the
// lookup and ranking (reference.js's `search`) and the entry
// (components/entry-view.js), read-only: no Run, Type or Try it, which
// need a calculator. No ROM, no worker, no emulator. The address names
// the command: `#48gx/hms-` (slug.js); the entry's model pills lead to
// the same command on the other models. Tested by
// web/test/docs-page.test.mjs.

import { ReferenceLoader } from "../../app/palette.js";
import { menuCommands, search } from "../../app/reference.js";
import { entryView } from "../../app/components/entry-view.js";
import { MODEL_ORDER, MODEL_TITLES } from "../../app/norom.js";
import { el } from "../../app/shared/dom.js";
import { commandOfSlug, parseRefHash, refHash, slugBase, slugMap } from "./slug.js";

/** How many suggestions the list shows; the rest behind "All n matches". */
const SUGGESTIONS = 10;

const loader = new ReferenceLoader(new URL("../../app/commands.json", import.meta.url));
const picker = document.getElementById("ref-model");
const query = document.getElementById("ref-q");
const suggest = document.getElementById("ref-suggest");
const pane = document.getElementById("ref-entry");
const browse = document.getElementById("ref-browse");
const count = document.getElementById("ref-count");
const baseTitle = document.title;

/** The models with a reference, in the app's order. */
let models = [];
/** The shown model's index and slugs. */
let shown = { model: null, index: null, slugs: null };
/** Every model's slugs once built (the entry's pills link through them). */
const slugsOf = new Map();
/** The suggestions shown: the rows and the selected one's position. */
let listed = { rows: [], total: 0, selected: -1 };
/** What the browse area shows: the model's menus, or every match of a query. */
let matches = null;

/** The model the app last showed (the same site keeps it), else the first. */
function rememberedModel() {
  let m = null;
  try { m = localStorage.getItem("saturnus.model"); } catch { /* storage blocked */ }
  return models.includes(m) ? m : models[0];
}

/** The commands of the shown model matching `q`, best first, by the app's ranking. */
function found(q) {
  return search(shown.index, q, { typing: false, limit: Infinity }).filter((r) => r.kind === "command").map((r) => r.command);
}

/** The address of `name` on `model`: its slug there when known, else the slug its name gives. */
function hrefOf(model, name) {
  const slugs = model === shown.model ? shown.slugs : slugsOf.get(model);
  return refHash(model, slugs?.get(name) ?? slugBase(name));
}

const title = () => MODEL_TITLES[shown.model] ?? shown.model;

/** The count line: how many commands the model has, or how many (`total`) match the query. */
function renderCount(q = "", total = 0) {
  count.textContent = q
    ? `${total} of ${shown.index.commands.length} ${title()} commands match “${q}”.`
    : `${shown.index.commands.length} commands on the ${title()}.`;
}

// ------------------------------------------------------------ suggestions

/** One suggestion: the name (a link to its address), its long name, its stack effect or keys. */
function suggestion(command, i) {
  const line = command.stack ?? command.keysText ?? "";
  const a = el("a", { href: hrefOf(shown.model, command.name), class: "ref-row", tabindex: "-1" },
    el("span", { class: "ref-name", text: command.name }),
    command.longName ? el("span", { class: "ref-long", text: command.longName }) : null,
    line ? el("span", { class: "ref-stack", text: line.split("\n")[0] }) : null);
  const li = el("li", { role: "option", id: `ref-opt-${i}`, "aria-selected": "false" }, a);
  // The pointer chooses without taking the focus off the field.
  li.addEventListener("mousedown", (e) => e.preventDefault());
  return li;
}

function select(i) {
  listed.selected = i;
  for (const [j, li] of [...suggest.children].entries()) {
    const on = j === i;
    li.setAttribute("aria-selected", on ? "true" : "false");
    if (on) li.scrollIntoView({ block: "nearest" });
  }
  if (i >= 0) query.setAttribute("aria-activedescendant", `ref-opt-${i}`);
  else query.removeAttribute("aria-activedescendant");
}

function closeSuggestions() {
  suggest.hidden = true;
  suggest.replaceChildren();
  listed = { rows: [], total: 0, selected: -1 };
  query.setAttribute("aria-expanded", "false");
  query.removeAttribute("aria-activedescendant");
}

/**
 * The suggestions for the field's text: the best ten, and a row for all
 * of them when there are more. Nothing found: the list stays closed and
 * the count line says so.
 */
function renderSuggestions() {
  const q = query.value.trim();
  if (!q) {
    closeSuggestions();
    renderCount();
    return;
  }
  const all = found(q);
  const rows = all.slice(0, SUGGESTIONS);
  if (!rows.length) {
    closeSuggestions();
    renderCount(q, 0);
    return;
  }
  listed = { rows, total: all.length, selected: -1 };
  renderCount(q, all.length);
  const items = rows.map(suggestion);
  if (all.length > rows.length) {
    const more = el("li", { role: "option", id: `ref-opt-${rows.length}`, "aria-selected": "false" },
      el("button", { type: "button", class: "ref-row ref-more", tabindex: "-1", text: `All ${all.length} matches` }));
    more.addEventListener("mousedown", (e) => e.preventDefault());
    more.querySelector("button").addEventListener("click", () => showMatches(q, all));
    items.push(more);
  }
  suggest.replaceChildren(...items);
  suggest.hidden = false;
  query.setAttribute("aria-expanded", "true");
}

/** Open suggestion `i` (the first one for none chosen): its address, or every match. */
function choose(i) {
  const q = query.value.trim();
  if (i >= listed.rows.length) {
    if (listed.total > listed.rows.length) showMatches(q, found(q));
    return;
  }
  const command = listed.rows[Math.max(0, i)];
  if (!command) return;
  closeSuggestions();
  location.hash = hrefOf(shown.model, command.name);
}

// ------------------------------------------------------------------ browse

/** A command as a chip: its name, a link to its address, the long name on hover. */
function chip(command, current) {
  return el("a", {
    href: hrefOf(shown.model, command.name), class: "ref-chip", title: command.longName || null, "aria-current": current ? "true" : null, text: command.name,
  });
}

function chips(commands, current) {
  return el("div", { class: "ref-chips" }, ...commands.map((c) => chip(c, c.name === current)));
}

/** A menu of the tree as a folded group: its own commands, then each menu under it. */
function group(menu, current) {
  const all = menuCommands(menu);
  const body = [];
  if (menu.commands.length) body.push(chips(menu.commands, current));
  for (const sub of menu.children) {
    body.push(el("div", { class: "ref-sub" }, el("h4", { text: sub.title }), chips(menuCommands(sub), current)));
  }
  return el("details", { class: "ref-group" },
    el("summary", {}, el("span", { class: "ref-group-name", text: menu.title }), el("span", { class: "ref-n", text: String(all.length) })),
    ...body);
}

/** Names A to Z: letters before operators, then as the locale sorts. */
function byName(a, b) {
  const op = (c) => (/^[A-Za-z]/.test(c.name) ? 0 : 1);
  return op(a) - op(b) || a.name.localeCompare(b.name);
}

/** The model's commands to browse: a group per menu (the headings' menus as groups of their own), all of them A to Z last. */
function renderBrowse(current) {
  matches = null;
  const groups = [];
  for (const node of shown.index.menus) {
    if (node.kind === "heading") {
      groups.push(el("h3", { class: "ref-heading", text: node.title }));
      for (const sub of node.children) groups.push(group(sub, current));
    } else groups.push(group(node, current));
  }
  const all = [...shown.index.commands].sort(byName);
  browse.replaceChildren(
    el("h2", { class: "ref-browse-title", text: `The ${title()}'s ${all.length} ${shown.index.keyed ? "functions" : "commands"}, by menu` }),
    ...groups,
    el("details", { class: "ref-group ref-all" },
      el("summary", {}, el("span", { class: "ref-group-name", text: "All of them, A to Z" }), el("span", { class: "ref-n", text: String(all.length) })),
      chips(all, current)));
}

/** Every command matching `q` in the browse area, best first; the field keeps its text. */
function showMatches(q, commands) {
  matches = q;
  closeSuggestions();
  renderCount(q, commands.length);
  const current = addressed();
  browse.replaceChildren(
    el("h2", { class: "ref-browse-title", text: `${commands.length} ${title()} commands match “${q}”` }),
    chips(commands, current),
    el("p", { class: "ref-browse-note muted", text: "Best first: names, then long names, then descriptions and examples. Clear the search to browse by menu." }));
}

// ------------------------------------------------------------------ entry

/** The entry of `name` on the shown model; the pane hides without one. */
function renderEntry(name, slug) {
  const command = name ? shown.index.byName.get(name) : null;
  if (!command) {
    pane.replaceChildren(slug
      ? el("p", { class: "ref-hint muted", text: `The ${title()} has no command at this address. Search for it above, or browse by menu below.` })
      : null);
    pane.hidden = !slug;
    document.title = baseTitle;
    return;
  }
  // The pills lead to the same command on the other models.
  pane.replaceChildren(entryView(shown.index, command, { modelHref: (m) => hrefOf(m, command.name) }));
  pane.hidden = false;
  document.title = `${command.name} on the ${title()} · ${baseTitle}`;
}

/** The command the address names on the shown model, or null. */
function addressed() {
  const { slug } = parseRefHash(location.hash);
  return slug ? commandOfSlug(shown.slugs, slug) : null;
}

/** Show what the address names: its model (else the remembered one) and command. */
async function fromAddress() {
  const { model, slug } = parseRefHash(location.hash);
  const m = models.includes(model) ? model : rememberedModel();
  const changed = m !== shown.model;
  if (changed) {
    const index = await loader.index(m);
    const slugs = slugMap(index.commands.map((c) => c.name));
    slugsOf.set(m, slugs);
    shown = { model: m, index, slugs };
    picker.value = m;
  }
  const name = addressed();
  renderEntry(name, slug);
  const q = query.value.trim();
  if (matches !== null && q) showMatches(q, found(q));
  else if (changed || matches !== null || !browse.childElementCount) renderBrowse(name);
  else {
    for (const a of browse.querySelectorAll(".ref-chip")) {
      if (a.textContent === name) a.setAttribute("aria-current", "true");
      else a.removeAttribute("aria-current");
    }
  }
  if (changed && matches === null) renderCount();
  // A chip far down the page: bring the entry into view.
  if (name && !pane.hidden) {
    const top = pane.getBoundingClientRect().top;
    if (top < 0 || top > window.innerHeight * 0.6) pane.scrollIntoView({ block: "start" });
  }
}

async function start() {
  const data = await loader.data();
  const has = new Set([...data.models, ...Object.keys(data.keyModels ?? {})]);
  models = MODEL_ORDER.filter((m) => has.has(m));
  picker.replaceChildren(...models.map((m) => el("option", { value: m, text: MODEL_TITLES[m] ?? m })));
  picker.addEventListener("change", () => {
    // The same command on the other model, when it has it.
    const { slug } = parseRefHash(location.hash);
    location.hash = refHash(picker.value, slug);
  });
  query.addEventListener("input", () => {
    renderSuggestions();
    // The search cleared: back to the menus.
    if (!query.value.trim() && matches !== null) renderBrowse(addressed());
  });
  query.addEventListener("focus", () => { if (query.value.trim() && suggest.hidden) renderSuggestions(); });
  query.addEventListener("keydown", (e) => {
    const n = suggest.hidden ? 0 : suggest.querySelectorAll('[role="option"]').length;
    if (e.key === "ArrowDown" && n) { e.preventDefault(); select((listed.selected + 1) % n); }
    else if (e.key === "ArrowUp" && n) { e.preventDefault(); select((listed.selected - 1 + n) % n); }
    else if (e.key === "Enter") {
      e.preventDefault();
      if (suggest.hidden && query.value.trim()) renderSuggestions();
      if (listed.rows.length || listed.total) choose(listed.selected);
    } else if (e.key === "Escape" && !suggest.hidden) { e.preventDefault(); closeSuggestions(); }
  });
  query.addEventListener("blur", () => closeSuggestions());
  suggest.addEventListener("click", (e) => { if (e.target.closest("a")) closeSuggestions(); });
  window.addEventListener("hashchange", () => { fromAddress(); });
  await fromAddress();
  document.body.dataset.ready = "true";
  // The other RPL models' slugs after the first render, for the entry's
  // pills (a name's slug differs where two names of a model share one:
  // the 49G's QR and qr); the pills shown are drawn again with them.
  const others = data.models.filter((m) => !slugsOf.has(m));
  await Promise.all(others.map(async (m) => {
    const index = await loader.index(m);
    slugsOf.set(m, slugMap(index.commands.map((c) => c.name)));
  }));
  if (others.length && !pane.hidden) {
    const { slug } = parseRefHash(location.hash);
    renderEntry(addressed(), slug);
  }
  document.body.dataset.slugs = "true";
}

start().catch((err) => {
  count.textContent = `The command reference could not be loaded (${err?.message ?? err}). Reload the page to try again.`;
  count.classList.add("error");
});
