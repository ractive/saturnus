// <sat-about>: the About panel, a modal dialog with the project statement
// and every source and input the emulator was built from, read from
// about.json (generated from the hardware wiki by scripts/about-json.py),
// and the manuals the command reference links into (commands.json).

const TEMPLATE = `
  <dialog class="about" aria-labelledby="about-title">
    <div class="about-head">
      <img class="logo" src="logo.svg" alt="" width="40" height="40">
      <h2 id="about-title">About saturnus</h2>
      <button type="button" class="icon about-close" title="Close" aria-label="Close">✕</button>
    </div>
    <div class="about-body">
      <div class="about-statement"></div>
      <h3>Black-box oracle</h3>
      <ul class="about-oracles"></ul>
      <h3>Skin references</h3>
      <ul class="about-skins"></ul>
      <h3>Tools</h3>
      <ul class="about-tools"></ul>
      <h3>ROMs</h3>
      <p class="about-roms"></p>
      <h3>Manuals</h3>
      <p class="hint">HP's manuals, as published by their current host; the command palette links each command to its page. The reference's descriptions are saturnus's own, written from the ROMs' behaviour, with the manuals as fact sources.</p>
      <ul class="about-manuals"></ul>
      <h3>Literature <span class="about-count"></span></h3>
      <p class="hint">Every source page of the project's hardware wiki: what was read, where it is published, and the wiki pages that use it.</p>
      <ol class="about-sources"></ol>
    </div>
  </dialog>`;

function el(tag, attrs = {}, text = null) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  if (text !== null) e.textContent = text;
  return e;
}

function link(url, text) {
  const a = el("a", { href: url, target: "_blank", rel: "noopener noreferrer" }, text ?? url);
  return a;
}

export class SatAbout extends HTMLElement {
  connectedCallback() {
    if (this.dialog) return;
    this.innerHTML = TEMPLATE;
    this.dialog = this.querySelector("dialog");
    this.querySelector(".about-close").addEventListener("click", () => this.dialog.close());
    // A click on the backdrop closes it too.
    this.dialog.addEventListener("click", (e) => {
      if (e.target === this.dialog) this.dialog.close();
    });
    this.loaded = null;
    this.reference = null;
  }

  /** The `ReferenceLoader` whose data lists the manuals. */
  setReference(reference) {
    this.reference = reference;
  }

  async open() {
    this.loaded ??= this.load();
    this.dialog.showModal();
    await this.loaded;
  }

  async load() {
    let about;
    try {
      const res = await fetch(new URL("../about.json", import.meta.url));
      about = await res.json();
    } catch (err) {
      this.querySelector(".about-statement").textContent = `Cannot read about.json: ${err}`;
      this.loaded = null; // the next open tries again
      return;
    }
    const st = this.querySelector(".about-statement");
    for (const p of about.statement) st.append(el("p", {}, p));

    const ul = (sel, items, fill) => {
      const list = this.querySelector(sel);
      for (const it of items) {
        const li = el("li");
        fill(li, it);
        list.append(li);
      }
    };
    ul(".about-oracles", about.oracles, (li, o) => {
      li.append(link(o.url, o.name), ` (${o.licence}). ${o.use}`);
    });
    ul(".about-skins", about.skins, (li, s) => {
      li.append(el("strong", {}, s.model), ` ${s.reference}`);
    });
    ul(".about-tools", about.tools, (li, t) => {
      li.append(link(t.url, t.name), `. ${t.use}`);
    });
    this.querySelector(".about-roms").textContent = about.roms;
    this.loadManuals();
    this.querySelector(".about-count").textContent = `(${about.sources.length})`;
    ul(".about-sources", about.sources, (li, s) => {
      li.append(el("strong", {}, s.title));
      const who = [s.authors.join(", "), s.year].filter(Boolean).join(", ");
      if (who) li.append(` · ${who}`);
      const where = el("div", { class: "about-where" });
      if (s.url) where.append(link(s.url));
      if (s.archived) {
        if (s.url) where.append(" · ");
        where.append("a copy in the project's literature archive");
      }
      if (s.url || s.archived) li.append(where);
      if (s.usedFor.length) {
        const used = el("div", { class: "about-used" });
        used.append("Used for: ", s.usedFor.map((u) => u.title).join("; "));
        li.append(used);
      }
    });
  }
}

SatAbout.prototype.loadManuals = async function loadManuals() {
  const list = this.querySelector(".about-manuals");
  if (!this.reference) {
    list.append(el("li", { class: "about-where" }, "Not available in this page."));
    return;
  }
  try {
    const data = await this.reference.data();
    const titles = { "48sx": "HP 48SX", "48gx": "HP 48GX", "49g": "HP 49G" };
    for (const m of Object.values(data.manuals)) {
      const li = el("li");
      li.append(link(m.url, m.title), ` (${m.models.map((x) => titles[x] ?? x).join(", ")})`);
      list.append(li);
    }
  } catch (err) {
    list.append(el("li", { class: "about-where" }, `The manual list could not be read: ${err?.message ?? err}`));
  }
};

customElements.define("sat-about", SatAbout);
