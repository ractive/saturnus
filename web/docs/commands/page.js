// The command reference page of the docs (/saturnus/docs/commands/,
// iteration 57): a model picker, a search field, the model's commands
// and one command's entry. It runs the app's own code from app/ beside
// it on the site: the data (commands.json) and its loader
// (palette.js's ReferenceLoader), the lookup and ranking (reference.js's
// `search`) and the entry (components/entry-view.js), read-only: no Run,
// Type or Try it, which need a calculator. No ROM, no worker, no
// emulator. The address names the command: `#48gx/hms-` (slug.js).
// Tested by web/test/docs-page.test.mjs.

import { ReferenceLoader } from "../../app/palette.js";
import { search } from "../../app/reference.js";
import { entryView } from "../../app/components/entry-view.js";
import { MODEL_ORDER, MODEL_TITLES } from "../../app/norom.js";
import { el } from "../../app/shared/dom.js";
import { commandOfSlug, parseRefHash, refHash, slugMap } from "./slug.js";

const loader = new ReferenceLoader(new URL("../../app/commands.json", import.meta.url));
const picker = document.getElementById("ref-model");
const query = document.getElementById("ref-q");
const list = document.getElementById("ref-list");
const pane = document.getElementById("ref-entry");
const count = document.getElementById("ref-count");
const baseTitle = document.title;

/** The models with a reference, in the app's order. */
let models = [];
/** The shown model's index and slugs. */
let shown = { model: null, index: null, slugs: null };

/** The model the app last showed (the same site keeps it), else the first. */
function rememberedModel() {
  let m = null;
  try { m = localStorage.getItem("saturnus.model"); } catch { /* storage blocked */ }
  return models.includes(m) ? m : models[0];
}

/** The commands for the search field's text: all of them for none, else the app's ranking. */
function rows(index, q) {
  if (!q.trim()) return index.commands;
  return search(index, q, { typing: false, limit: Infinity }).filter((r) => r.kind === "command").map((r) => r.command);
}

/** A list row: the name (a link to its address), its long name, its stack effect. */
function row(command, current) {
  const a = el("a", { href: refHash(shown.model, shown.slugs.get(command.name)), class: "ref-row", "aria-current": current ? "true" : null },
    el("span", { class: "ref-name", text: command.name }),
    command.longName ? el("span", { class: "ref-long", text: command.longName }) : null,
    command.stack ? el("span", { class: "ref-stack", text: command.stack.split("\n")[0] }) : null);
  return el("li", {}, a);
}

function renderList(current = null) {
  const found = rows(shown.index, query.value);
  const title = MODEL_TITLES[shown.model] ?? shown.model;
  const q = query.value.trim();
  count.textContent = q
    ? `${found.length} of ${shown.index.commands.length} ${title} commands match “${q}”.`
    : `${shown.index.commands.length} commands on the ${title}.`;
  list.replaceChildren(...found.map((c) => row(c, c.name === current)));
  return found;
}

/** The entry of `name` on the shown model, or a hint when none is chosen. */
function renderEntry(name) {
  const command = name ? shown.index.byName.get(name) : null;
  if (!command) {
    pane.replaceChildren(el("p", { class: "ref-hint muted", text: "Choose a command from the list, or search for one by its name or what it does." }));
    document.title = baseTitle;
    return;
  }
  pane.replaceChildren(entryView(shown.index, command, {}));
  document.title = `${command.name} on the ${MODEL_TITLES[shown.model]} · ${baseTitle}`;
}

/** Scroll the list (not the page) so that `row` shows. */
function showInList(row) {
  if (!row) return;
  const r = row.getBoundingClientRect();
  const box = list.getBoundingClientRect();
  if (r.top < box.top || r.bottom > box.bottom) list.scrollTop += r.top - box.top - (list.clientHeight - r.height) / 2;
}

/** The command the address names on the shown model, or null. */
function addressed() {
  const { slug } = parseRefHash(location.hash);
  return slug ? commandOfSlug(shown.slugs, slug) : null;
}

/** Show what the address names: its model (else the remembered one) and command. */
async function fromAddress() {
  const { model } = parseRefHash(location.hash);
  const m = models.includes(model) ? model : rememberedModel();
  if (m !== shown.model) {
    const index = await loader.index(m);
    shown = { model: m, index, slugs: slugMap(index.commands.map((c) => c.name)) };
    picker.value = m;
  }
  const name = addressed();
  renderList(name);
  renderEntry(name);
  if (name) showInList(list.querySelector('[aria-current="true"]'));
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
  query.addEventListener("input", () => renderList(addressed()));
  query.addEventListener("keydown", (e) => {
    // Enter opens the first command found.
    if (e.key !== "Enter") return;
    const first = list.querySelector("a.ref-row");
    if (first) {
      e.preventDefault();
      location.hash = first.getAttribute("href");
    }
  });
  window.addEventListener("hashchange", () => { fromAddress(); });
  await fromAddress();
  document.body.dataset.ready = "true";
}

start().catch((err) => {
  count.textContent = `The command reference could not be loaded (${err?.message ?? err}). Reload the page to try again.`;
  count.classList.add("error");
});
