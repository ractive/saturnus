// The command palette's model, without the DOM: the query and its ranked
// rows, the selection, the command line's state (read from the host when
// the palette opens and after every send) and what choosing a row does:
// `insert` or `run` through the protocol's typing verbs (web/protocol.md,
// "Typing"), an app action, or a menu to browse. On a key-driven model
// (the 42S, 27S, 17BII, 17B, 19BII; the 38G, 39G and 40G) a function is
// pressed instead: its keys go to the key queue (`typeKeys`), after
// leaving the menu the screen shows, if any, and (38G, 39G, 40G) going
// to HOME. On the 28S, whose command line cannot be read, the text is
// typed on its keys (keys28.js). `<sat-palette>` renders it; `web/test/`
// drives it with a fake backend.

import { send28, settle } from "./keys28.js";
import { buildIndex, cancelKeys, enterPlan, enterVerb, exampleSteps, exampleText, keyNames, keysText, menuShown, search } from "./reference.js";

const message = (err) => String(err?.message ?? err);

/** What the palette says when the calculator was not in a state to type into. */
export const NOT_READY = "The calculator is not ready for typing. Answer what it shows on its display, then try again.";

/**
 * The palette's words for a failed send: the host's own sentence where
 * it is the user's to act on (a character the model cannot type, a
 * text too long, no command line to replace), else, for the typing
 * engine's internal checks (the keyboard state it could not reach,
 * the screen not showing what it typed), `NOT_READY`; those details
 * go to the console.
 */
export function sendFailure(err) {
  const text = message(err);
  if (/could not bring the keyboard|typing stopped at character|did not open for|stayed busy|not plausible|not set up|not idle/i.test(text)) {
    console.warn("saturnus: typing failed:", text);
    return NOT_READY;
  }
  return text;
}

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
   * `[{id, title, description?, keywords?, off?, searchOnly?, run()}]`:
   * `off` is why it cannot run now (listed greyed, Enter says why);
   * `searchOnly` keeps it out of the list shown before anything is typed.
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
    /** How long `settle` (keys28.js) waits before and between its looks, in ms (tests set 0); also for the 28S's typing. */
    this.pace = { first: 40, poll: 40, limit: 10_000 };
    this.onChange = () => {};
  }

  /** Whether the shown model is key-driven: its functions are pressed. */
  get keyed() {
    return Boolean(this.index?.keyed);
  }

  /** Whether a function of the shown key-driven model can be pressed now. */
  get canPress() {
    return this.keyed && this.store.state.booted === this.index.model;
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
    // The 28S's line cannot be read: its commands are typed on its keys
    // (keys28.js), into the open line if there is one.
    if (this.store.state.booted === "28s") {
      this.commandLine = null;
      this.noTyping = null;
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
    if (plan.press) return this.press(plan.press.steps, { closeAfter: true });
    if (plan.action) {
      // An action that is off says why and stays (`off`, its reason).
      if (plan.action.off) {
        this.notice = { text: stop(plan.action.off), error: true };
        this.changed();
        return { close: false };
      }
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

  /** "Try it": the example's setup, input and command, run (pressed, on a key-driven model). The palette stays open to show the next one. */
  tryExample(example, command = this.row?.command) {
    if (this.keyed && command) return this.press(exampleSteps(command, example), { closeAfter: false, fresh: true, label: example.then?.length ? `the example with ${command.keysText}` : null });
    return this.send("run", exampleText(example), { closeAfter: false });
  }

  /**
   * Press `steps` on the running key-driven calculator: first out of
   * the menu the screen shows (its `leave` keys, again while one
   * shows), then the steps through the key queue, waiting until the
   * calculator has played them. `fresh` (an example) also ends the
   * entry in progress, so its numbers start new ones.
   */
  async press(steps, { closeAfter, fresh = false, label = null }) {
    if (!this.canPress) {
      this.notice = { text: this.store.state.booted ? `The calculator running is not the ${this.index?.model}.` : "Start the calculator to press this.", error: true };
      this.changed();
      return { close: false };
    }
    if (this.sending) return { close: false };
    const text = label ?? keysText(steps);
    this.sending = true;
    this.notice = { text: `Pressing ${text}…`, error: false };
    this.changed();
    try {
      await pressFunction(this.backend, this.store, this.index.km, steps, { fresh, pace: this.pace });
    } catch (err) {
      this.sending = false;
      this.notice = { text: message(err), error: true };
      this.changed();
      return { close: false };
    }
    this.sending = false;
    this.notice = closeAfter ? null : { text: `Pressed ${text}.`, error: false };
    this.changed();
    return { close: closeAfter };
  }

  /** Wait until the calculator has played the queued keys and sleeps (`settle` in keys28.js, at `pace`). */
  settle() {
    return settle(this.backend, this.store, this.pace);
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
      result = this.store.state.booted === "28s"
        ? await send28(this.backend, this.store, verb, text, this.pace)
        : await this.backend[verb](text);
    } catch (err) {
      this.sending = false;
      this.notice = { text: sendFailure(err), error: true };
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

/**
 * Press `steps` on the running key-driven calculator (`km`, its key
 * model from the reference): first a pending shift or ALPHA cancelled,
 * out of the menu the screen shows (its `leave` keys, again while one
 * shows), on the 38G-40G to HOME (`home`), then the steps through the key
 * queue, waiting until the calculator has played them. `fresh` (an
 * example) also ends the entry in progress, so its numbers start new
 * ones. Search's Enter and Try it, and an AI app's `run_function`
 * (web/ai-tools.js).
 */
export async function pressFunction(backend, store, km, steps, { fresh = false, pace = undefined } = {}) {
  const wait = () => settle(backend, store, pace);
  // The 38G, 39G and 40G go to HOME first (`home`), and an example
  // then clears the edit line with the leave keys (CANCEL).
  const home = km?.home;
  // A shift or ALPHA pending would change the first key: on the 42S
  // a shifted EXIT is OFF, on the 38G-40G a shifted ON too.
  await cancelPending(backend, store, wait);
  await leaveMenus(backend, store, km, wait, fresh && !home);
  if (home?.length) {
    backend.typeKeys(keyNames(home));
    await wait();
    if (fresh) await leaveMenus(backend, store, km, wait, true);
  }
  backend.typeKeys(keyNames(steps));
  await wait();
}

/**
 * Leave the menus the screen shows (at most six levels); with `always`, press the leave keys once even with none.
 * A model without `leave.menuBox` always shows a menu (the 17BII's MAIN): its leave keys go back there from anywhere, once.
 */
async function leaveMenus(backend, store, km, wait, always = false) {
  const leave = km?.leave;
  if (!leave) return;
  if (!leave.menuBox) {
    backend.typeKeys(keyNames(leave.keys));
    await wait();
    return;
  }
  let pressed = 0;
  while (pressed < 6 && menuShown(km, store.state.frame)) {
    backend.typeKeys(keyNames(leave.keys));
    pressed++;
    await wait();
  }
  if (always && !pressed) {
    backend.typeKeys(keyNames(leave.keys));
    await wait();
  }
}

/**
 * Cancel a pending shift or ALPHA (their annunciators on the display)
 * by pressing that key again, reading the annunciators after each
 * press; an error if one stays, so no key runs shifted.
 */
async function cancelPending(backend, store, wait) {
  for (let i = 0; i < 3; i++) {
    const keys = cancelKeys(store.state.frame?.annunciators);
    if (!keys.length) return;
    backend.typeKeys(keys);
    await wait();
  }
  if (cancelKeys(store.state.frame?.annunciators).length) {
    throw new Error("A shift or ALPHA stays on. Press the shift key on the calculator to turn it off, then try again.");
  }
}

/** `text` ending in a full stop. */
export function stop(text) {
  return /[.…!?]$/.test(text) ? text : `${text}.`;
}
