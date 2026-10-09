// The command palette's model, without the DOM: the query and its ranked
// rows, the selection, the command line's state (read from the host when
// the palette opens and after every send) and what choosing a row does:
// `insert` or `run` through the protocol's typing verbs (web/protocol.md,
// "Typing"), an app action, or a menu to browse. `<sat-palette>` renders
// it; `web/test/` drives it with a fake backend.

import { buildIndex, enterPlan, enterVerb, exampleText, search } from "./reference.js";

const message = (err) => String(err?.message ?? err);

/**
 * One model's index for a view, with its failure remembered only for
 * that model: `ensure(model)` resolves to `{index}` or `{error}`, loads
 * again after a failure when asked for another model or after `reset()`,
 * and never for the same model twice while a load is in flight.
 */
export class IndexWatch {
  constructor(reference) {
    this.reference = reference;
    this.model = null;
    this.index = null;
    this.error = null;
    this.loading = null;
  }

  /** Forget what was loaded and any failure (a new ROM booted). */
  reset() {
    this.model = null;
    this.index = null;
    this.error = null;
    this.loading = null;
  }

  /** The state for `model` now: `{index}`, `{error}`, or `{loading: true}` while it loads. */
  state(model) {
    if (this.model === model && this.index) return { index: this.index };
    if (this.model === model && this.error) return { error: this.error };
    return { loading: true };
  }

  async ensure(model) {
    if (this.model === model && (this.index || this.error) && !this.loading) return this.state(model);
    if (this.model === model && this.loading) return this.loading;
    this.model = model;
    this.index = null;
    this.error = null;
    this.loading = (async () => {
      try {
        const index = await this.reference.index(model);
        if (this.model === model) this.index = index;
      } catch (err) {
        if (this.model === model) this.error = message(err);
      } finally {
        if (this.model === model) this.loading = null;
      }
      return this.state(model);
    })();
    return this.loading;
  }
}

/**
 * The reference data (web/commands.json), fetched once when first asked
 * for, with one index per model. Shared by the palette and the
 * explorer's Commands tab.
 */
export class ReferenceLoader {
  constructor(url) {
    this.url = url;
    this.loading = null;
    this.indexes = new Map();
  }

  /** The parsed data; one fetch, retried after a failure. */
  data() {
    this.loading ??= (async () => {
      const res = await fetch(this.url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return res.json();
    })().catch((err) => {
      this.loading = null;
      throw err;
    });
    return this.loading;
  }

  /** `buildIndex` for `model`, cached. */
  async index(model) {
    if (!this.indexes.has(model)) {
      const data = await this.data();
      if (!this.indexes.has(model)) this.indexes.set(model, buildIndex(data, model));
    }
    return this.indexes.get(model);
  }
}

export class PaletteModel {
  /**
   * `store` has `.state` (`model`, `booted`, `busy`); `actions` are
   * `[{id, title, description?, keywords?, run()}]`.
   */
  constructor(backend, store, { actions = [] } = {}) {
    this.backend = backend;
    this.store = store.state ? store : { state: store };
    this.actions = actions;
    /** The index of the shown model (`buildIndex`), or null while loading. */
    this.index = null;
    this.loadError = null;
    this.query = "";
    this.rows = [];
    this.selected = 0;
    /** `{active, text, cursor}` as last read, or null when unknown. */
    this.commandLine = null;
    /** Why nothing can be sent (`null` when it can). */
    this.noTyping = null;
    /** The 49G is in algebraic mode (flag -95 set): RPN text does not parse there. */
    this.algebraic = false;
    /** The user's variables, the current directory first (`setVariables`). */
    this.variables = [];
    /** A line under the list: `{text, error}` or null. */
    this.notice = null;
    /** A send is on its way. */
    this.sending = false;
    this.onChange = () => {};
  }

  changed() {
    this.onChange(this);
  }

  /** Whether text can be sent to the calculator now. */
  get canType() {
    return Boolean(this.store.state.booted) && this.noTyping === null;
  }

  setIndex(index) {
    this.index = index;
    this.loadError = null;
    this.refresh();
  }

  setLoadError(err) {
    this.loadError = message(err);
    this.refresh();
  }

  setActions(actions) {
    this.actions = actions;
    this.refresh();
  }

  /**
   * The variables of the calculator's current directory and of its
   * parents, nearest first, each name once (as the calculator resolves
   * a name), from `memoryTree`'s `{path, variables}`.
   */
  setVariables(tree) {
    const out = [];
    const seen = new Set();
    if (tree?.variables) {
      const dirs = [];
      let vars = tree.variables;
      dirs.push({ path: ["HOME"], vars });
      for (let i = 1; i < tree.path.length; i++) {
        const d = vars.find((v) => v.name === tree.path[i] && Array.isArray(v.variables));
        if (!d) break;
        vars = d.variables;
        dirs.push({ path: tree.path.slice(0, i + 1), vars });
      }
      for (const d of dirs.reverse()) {
        for (const v of d.vars) {
          if (seen.has(v.name)) continue;
          seen.add(v.name);
          out.push({ name: v.name, path: d.path, type: v.type, directory: Array.isArray(v.variables) });
        }
      }
    }
    this.variables = out;
    this.refresh();
  }

  /** Read the command line (the Enter rule follows it); says why typing is impossible. */
  async readCommandLine() {
    if (!this.store.state.booted) {
      this.commandLine = null;
      this.noTyping = "no calculator is running";
      return;
    }
    try {
      this.commandLine = await this.backend.commandLine();
      this.noTyping = null;
    } catch (err) {
      this.commandLine = null;
      this.noTyping = message(err);
    }
  }

  /** The 49G's entry mode (flag -95): the examples and command names are RPN text. */
  async readMode() {
    this.algebraic = false;
    if (this.store.state.booted !== "49g") return;
    try {
      const flags = await this.backend.flags();
      this.algebraic = flags.set.includes(-95);
    } catch {
      // Memory not set up yet: unknown, assumed RPN.
    }
  }

  /** Opening: a fresh query, the command line and the mode read. */
  async open() {
    this.query = "";
    this.selected = 0;
    this.notice = null;
    await Promise.all([this.readCommandLine(), this.readMode()]);
    this.refresh();
  }

  setQuery(q) {
    this.query = q;
    this.selected = 0;
    this.notice = null;
    this.refresh();
  }

  refresh() {
    const t0 = performance.now();
    this.rows = this.index
      ? search(this.index, this.query, { variables: this.variables, actions: this.actions, typing: this.canType })
      : search({ commands: [], menus: [] }, this.query, { actions: this.actions, typing: false });
    this.lastSearchMs = performance.now() - t0;
    if (this.selected >= this.rows.length) this.selected = Math.max(0, this.rows.length - 1);
    this.changed();
  }

  get row() {
    return this.rows[this.selected] ?? null;
  }

  select(i) {
    if (i < 0 || i >= this.rows.length) return;
    this.selected = i;
    this.changed();
  }

  move(delta) {
    if (!this.rows.length) return;
    this.selected = Math.min(this.rows.length - 1, Math.max(0, this.selected + delta));
    this.changed();
  }

  /** The verb Enter would use on the selected row. */
  verbFor(row = this.row, opposite = false) {
    if (!row || row.kind === "action" || row.kind === "menu") return null;
    return enterVerb(row, this.commandLine, opposite);
  }

  /** Choose row `n` (1-9) as its number shortcut does. */
  chooseNumber(n) {
    const row = this.rows[n - 1];
    return row ? this.choose(row) : Promise.resolve({ close: false });
  }

  /**
   * Act on `row`: an action runs, a menu is handed back to browse, a
   * command, variable or text is sent. Resolves to `{close}` (whether
   * the palette should close) and, for a menu, `{menu}`.
   */
  async choose(row, { opposite = false } = {}) {
    const plan = enterPlan(row, this.commandLine, opposite);
    if (!plan) return { close: false };
    if (plan.action) {
      // An action that fails says so here, as a refused send does.
      try {
        await plan.action.run?.();
      } catch (err) {
        this.notice = { text: `${plan.action.title}: ${message(err)}`, error: true };
        this.changed();
        return { close: false };
      }
      return { close: true };
    }
    if (plan.menu) return { close: true, menu: plan.menu };
    return this.send(plan.verb, plan.text, { closeAfter: true });
  }

  /** "Try it": the example's setup, input and command, run. The palette stays open to show the next one. */
  tryExample(example) {
    return this.send("run", exampleText(example), { closeAfter: false });
  }

  async send(verb, text, { closeAfter }) {
    if (!this.canType) {
      this.notice = { text: this.noTyping ?? "Nothing can be sent now.", error: true };
      this.changed();
      return { close: false };
    }
    if (this.sending) return { close: false };
    this.sending = true;
    this.notice = { text: verb === "run" ? `Running ${text.trim()}…` : `Inserting ${text.trim()}…`, error: false };
    this.changed();
    let result;
    try {
      result = await this.backend[verb](text);
    } catch (err) {
      this.sending = false;
      this.notice = { text: message(err), error: true };
      await this.readCommandLine();
      this.changed();
      return { close: false };
    }
    this.sending = false;
    this.commandLine = result.commandLine ?? this.commandLine;
    if (verb === "run" && this.algebraic) await this.readMode();
    if (verb === "run" && result.error) {
      // The calculator kept the line and showed a message: say it here.
      this.notice = { text: `The calculator says: ${result.error}`, error: true, calculator: true };
      this.changed();
      return { close: false };
    }
    if (verb === "run" && result.running) {
      this.notice = { text: "Sent. The calculator is still working on it.", error: false };
      this.changed();
      return { close: closeAfter };
    }
    this.notice = closeAfter ? null : { text: verb === "run" ? `Ran ${text.trim()}.` : `Inserted ${text.trim()}.`, error: false };
    this.changed();
    return { close: closeAfter };
  }
}
