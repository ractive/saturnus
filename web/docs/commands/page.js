// The command reference page of the docs (/saturnus/docs/commands/,
// iteration 57, redesigned twice after the owner's reviews): a search
// field with the model beside it and its suggestions as you type (the
// ten best by the app's ranking), then two panes on a desktop: the
// model's commands by menu on the left (a sticky navigation, one folded
// group per menu of the calculator, every command a row) and the chosen
// command's entry on the right. On a phone one column: the entry under
// the search, the navigation in a sheet that "Browse" opens. It runs
// the app's own code from app/ beside it on the site: the data
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
const bar = document.getElementById("ref-bar");
const picker = document.getElementById("ref-model");
const query = document.getElementById("ref-q");
const suggest = document.getElementById("ref-suggest");
const pane = document.getElementById("ref-entry");
const side = document.getElementById("ref-side");
const nav = document.getElementById("ref-nav");
const count = document.getElementById("ref-count");
const browse = document.getElementById("ref-browse");
const sheet = document.getElementById("ref-sheet");
const sheetBody = sheet.querySelector(".ref-sheet-body");
/** The hairline above the bar: out of view once the bar is stuck; where the page docks. */
const sentinel = document.getElementById("ref-top");
const panes = side.parentElement;
const baseTitle = document.title;
/** One column (a phone): the navigation lives in the sheet. */
const narrow = matchMedia("(max-width: 859px)");

/** The models with a reference, in the app's order. */
let models = [];
/** The shown model's index and slugs. */
let shown = { model: null, index: null, slugs: null };
/** Every model's slugs once built (the entry's pills link through them). */
const slugsOf = new Map();
/** The suggestions shown: the rows and the selected one's position. */
let listed = { rows: [], total: 0, selected: -1 };
/** What the navigation shows: the model's menus (null), or every match of a query. */
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
const noun = () => (shown.index.keyed ? "functions" : "commands");

/** The count line: how many commands the model has, or how many (`total`) match the query. */
function renderCount(q = "", total = 0) {
  count.textContent = q
    ? `${total} of ${shown.index.commands.length} ${title()} ${noun()} match “${q}”.`
    : `${shown.index.commands.length} ${noun()} on the ${title()}.`;
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
    more.querySelector("button").addEventListener("click", () => showMatches(q, all, true));
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
    if (listed.total > listed.rows.length) showMatches(q, found(q), true);
    return;
  }
  const command = listed.rows[Math.max(0, i)];
  if (!command) return;
  closeSuggestions();
  location.hash = hrefOf(shown.model, command.name);
}

// -------------------------------------------------------------- navigation

/** A command as a row of the navigation: its name and long name, a link to its address. */
function item(command) {
  return el("li", {}, el("a", { href: hrefOf(shown.model, command.name), class: "ref-item", "data-name": command.name },
    el("span", { class: "ref-item-name", text: command.name }),
    command.longName ? el("span", { class: "ref-item-long", text: command.longName }) : null));
}

function list(commands) {
  return el("ul", { class: "ref-list" }, ...commands.map(item));
}

/** A folded group: its name and count in the summary, `body` under it. */
function fold(name, n, cls, ...body) {
  return el("details", { class: `ref-group${cls ? ` ${cls}` : ""}` },
    el("summary", {}, el("span", { class: "ref-group-name", text: name }), el("span", { class: "ref-n", text: String(n) })),
    ...body);
}

/** A menu of the tree as a group: its own commands, then each menu under it as a group of its own. */
function group(menu) {
  const body = [];
  if (menu.commands.length) body.push(list(menu.commands));
  for (const sub of menu.children) {
    const all = menuCommands(sub);
    body.push(fold(sub.title, all.length, "ref-sub", list(all)));
  }
  return fold(menu.title, menuCommands(menu).length, null, ...body);
}

/** Names A to Z: letters before operators, then as the locale sorts. */
function byName(a, b) {
  const op = (c) => (/^[A-Za-z]/.test(c.name) ? 0 : 1);
  return op(a) - op(b) || a.name.localeCompare(b.name);
}

/** The model's commands by menu: a group per menu (the headings' menus as groups of their own), all of them A to Z last. */
function renderNav() {
  matches = null;
  const groups = [];
  for (const node of shown.index.menus) {
    if (node.kind === "heading") {
      groups.push(el("h3", { class: "ref-heading", text: node.title }));
      for (const sub of node.children) groups.push(group(sub));
    } else groups.push(group(node));
  }
  const all = [...shown.index.commands].sort(byName);
  nav.replaceChildren(
    el("p", { class: "ref-nav-title", text: `The ${title()}'s ${all.length} ${noun()}, by menu` }),
    ...groups,
    fold("All of them, A to Z", all.length, "ref-all", list(all)));
  nav.dataset.shows = "menus";
}

/**
 * Every command matching `q` in the navigation, best first; the field
 * keeps its text. `open`: on a phone, the sheet opens to show them.
 */
function showMatches(q, commands, open = false) {
  matches = q;
  closeSuggestions();
  renderCount(q, commands.length);
  nav.replaceChildren(
    el("p", { class: "ref-nav-title", text: `${commands.length} ${title()} ${noun()} match “${q}”` }),
    list(commands),
    el("p", { class: "ref-nav-note muted", text: "Best first: names, then long names, then descriptions and examples. Clear the search to browse by menu." }));
  nav.dataset.shows = "matches";
  markCurrent(addressed());
  if (open && narrow.matches) openSheet();
}

/**
 * The rows of `name` marked as the current one, the group of the first
 * unfolded and the row brought into view (the one in a menu before the
 * one in the A to Z list, an unfolded group's before a folded one's).
 */
function markCurrent(name) {
  let target = null;
  for (const a of nav.querySelectorAll(".ref-item")) {
    const on = Boolean(name) && a.dataset.name === name;
    if (on) a.setAttribute("aria-current", "true");
    else a.removeAttribute("aria-current");
    if (!on) continue;
    const inAll = Boolean(a.closest(".ref-all"));
    const visible = [...ancestors(a)].every((d) => d.open);
    const rank = (inAll ? 2 : 0) + (visible ? 0 : 1);
    if (!target || rank < target.rank) target = { a, rank };
  }
  if (!target) return;
  for (const d of ancestors(target.a)) d.open = true;
  // The column scrolls on its own on a desktop (the page stays where it
  // is); in the sheet it is about to close.
  if (narrow.matches || nav.parentElement !== side) return;
  const r = target.a.getBoundingClientRect();
  const s = side.getBoundingClientRect();
  const top = Math.max(s.top, 0);
  const bottom = Math.min(s.bottom, window.innerHeight);
  if (r.top < top) side.scrollTop += r.top - top - 8;
  else if (r.bottom > bottom) side.scrollTop += r.bottom - bottom + 8;
}

/** The groups around `a`, innermost first. */
function* ancestors(a) {
  for (let d = a.parentElement?.closest("details"); d; d = d.parentElement?.closest("details")) yield d;
}

/** Whether `x` is on the screen: not in a folded group (whose content keeps its boxes in Chrome). */
function onScreen(x) {
  return x.checkVisibility ? x.checkVisibility() : x.getClientRects().length > 0;
}

/** The navigation's keys: the arrows move between its rows and groups, Home and End to the ends. */
function navKeys(e) {
  if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
  const items = [...nav.querySelectorAll("summary, a.ref-item")].filter(onScreen);
  if (!items.length) return;
  const i = items.indexOf(document.activeElement);
  let to;
  if (e.key === "Home") to = 0;
  else if (e.key === "End") to = items.length - 1;
  else if (e.key === "ArrowDown") to = i < 0 ? 0 : Math.min(i + 1, items.length - 1);
  else to = i < 0 ? 0 : Math.max(i - 1, 0);
  e.preventDefault();
  items[to].focus();
}

// ------------------------------------------------------------------- sheet

/** The navigation in the sheet, open (a phone), the current row in view and focused. */
function openSheet() {
  if (sheet.open) return;
  sheetBody.append(nav);
  sheet.showModal();
  const current = nav.querySelector('.ref-item[aria-current="true"]');
  if (current) {
    current.scrollIntoView({ block: "center" });
    current.focus({ preventScroll: true });
  }
}

/** The sheet closed: the navigation back in its column; a command chosen there is what to see. */
function sheetClosed() {
  side.append(nav);
  if (pane.querySelector(".entry")) showEntry();
}

// ------------------------------------------------------------------ entry

/** The entry of `name` on the shown model; nothing chosen or nothing at the address: a line saying so. */
function renderEntry(name, slug) {
  const command = name ? shown.index.byName.get(name) : null;
  if (!command) {
    pane.replaceChildren(el("div", { class: "ref-empty" },
      el("p", { class: `ref-empty-title${slug ? " ref-hint" : ""}`, text: slug ? `The ${title()} has no command at this address.` : "Pick a command" }),
      el("p", { class: "muted" }, "Search above, or ",
        el("span", { class: "ref-wide", text: "open a menu on the left." }),
        el("span", { class: "ref-narrow", text: "browse the menus." }))));
    pane.dataset.empty = "true";
    document.title = baseTitle;
    return;
  }
  // The pills lead to the same command on the other models.
  pane.replaceChildren(entryView(shown.index, command, {
    modelHref: (m) => hrefOf(m, command.name),
    examplesNote: ` · results from saturnus's emulated ${title()}`,
  }));
  // The card scrolls inside itself on a desktop: a new entry from its top.
  pane.scrollTop = 0;
  delete pane.dataset.empty;
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
  else {
    if (changed || matches !== null || !nav.dataset.shows) renderNav();
    markCurrent(name);
  }
  if (changed && matches === null) renderCount();
  // A command chosen in the sheet: the sheet closes, the entry is what to
  // see. On a desktop the first render leaves the page where it is, the
  // title and the lead above the bar; it docks on a choice.
  if (sheet.open) sheet.close();
  else if (name && (narrow.matches || document.body.dataset.ready)) showEntry();
}

/**
 * The entry in view. On a desktop the page docks: the bar at the top of
 * the screen, the two cards under it (docs.css, or fitted: `fitPanes`),
 * wherever the page was (below, in the notes, or above with cards that
 * do not fit the first view). On a
 * phone the entry scrolls into view under the bar when it is off the
 * screen.
 */
function showEntry() {
  if (!narrow.matches) {
    // Cards that fit the screen where they are need no docking.
    if (panes.getBoundingClientRect().bottom <= window.innerHeight + 1 && window.scrollY === 0) return;
    const dock = Math.ceil(sentinel.getBoundingClientRect().bottom + window.scrollY);
    if (Math.abs(window.scrollY - dock) > 1) window.scrollTo({ top: dock });
    return;
  }
  const top = pane.getBoundingClientRect().top;
  const under = bar.getBoundingClientRect().bottom;
  if (top < under || top > window.innerHeight * 0.6) pane.scrollIntoView({ block: "start" });
}

/**
 * The two cards' height on a desktop: down to the bottom of the screen
 * from where they start under the title, the lead and the bar, so the
 * first view shows them whole. A window too short for that (under
 * `FIT_MIN` px left) keeps the docked height of docs.css: the bar at the
 * top, the cards under it, after a choice. Fitted cards keep their height
 * when the page docks from further down: they then end above the bottom
 * of the screen, with the notes under them, rather than growing while
 * the page scrolls.
 */
const FIT_MIN = 480;
function fitPanes() {
  if (narrow.matches) { panes.style.removeProperty("--ref-panes-h"); return; }
  const top = panes.getBoundingClientRect().top + window.scrollY;
  const gap = parseFloat(getComputedStyle(panes).getPropertyValue("--s4")) || 16;
  const h = Math.floor(window.innerHeight - top - gap);
  if (h >= FIT_MIN) panes.style.setProperty("--ref-panes-h", `${h}px`);
  else panes.style.removeProperty("--ref-panes-h");
}

async function start() {
  fitPanes();
  window.addEventListener("resize", fitPanes);
  document.fonts?.ready.then(fitPanes);
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
    if (!query.value.trim() && matches !== null) {
      renderNav();
      markCurrent(addressed());
    }
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
  nav.addEventListener("keydown", navKeys);
  // Cmd-K (Ctrl-K off a Mac) starts a search, as in the app.
  query.setAttribute("aria-keyshortcuts", "Meta+K Control+K");
  document.addEventListener("keydown", (e) => {
    if (e.key.toLowerCase() !== "k" || !(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey) return;
    e.preventDefault();
    if (sheet.open) sheet.close();
    query.focus();
    query.select();
  });
  browse.addEventListener("click", openSheet);
  sheet.querySelector(".ref-sheet-close").addEventListener("click", () => sheet.close());
  // A tap on the backdrop closes the sheet (the dialog itself is the target there).
  sheet.addEventListener("click", (e) => { if (e.target === sheet) sheet.close(); });
  sheet.addEventListener("close", sheetClosed);
  narrow.addEventListener("change", () => { if (!narrow.matches && sheet.open) sheet.close(); });
  // The bar stuck at the top takes a rule under it.
  if ("IntersectionObserver" in window) {
    new IntersectionObserver(([e]) => bar.classList.toggle("stuck", !e.isIntersecting)).observe(sentinel);
  }
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
  if (others.length && pane.querySelector(".entry")) {
    const { slug } = parseRefHash(location.hash);
    renderEntry(addressed(), slug);
  }
  document.body.dataset.slugs = "true";
}

start().catch((err) => {
  count.textContent = `The command reference could not be loaded (${err?.message ?? err}). Reload the page to try again.`;
  count.classList.add("error");
  nav.replaceChildren();
});
