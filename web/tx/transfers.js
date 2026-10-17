// Copies between the two panes, and from the computer's file manager onto
// one (kb/docs/saturnus-tx.md, "Layout", "Two devices at once"): a
// calculator to a folder, a folder to a calculator, a calculator to
// another calculator (a get, then a put; the copy plan of saturnus-tx
// with its warnings), a folder to a folder, dropped files and folders to
// either. Each copy is first planned, and everything it would warn about
// or replace is asked in one place before anything runs; then each item
// is a job of the queue (queue.js) on the places it uses.
//
// A place is `{id, kind: "device" | "folder", title, device | folder,
// model}` (a pane's, app.js). Items to copy are a calculator's listing
// entries, or file-system items (`FsItem`: a folder's entries, dropped
// files and folders): `{name, directory, size, read(), children()}`.
// No DOM: web/test/tx-transfers.test.mjs runs it over fakes.

import { failureText, stringQuestion } from "./flows.js";

/** A file of a folder as an FsItem. */
export function folderItem(folder, path, entry) {
  return {
    name: entry.name,
    directory: entry.directory,
    size: entry.size ?? null,
    read: async () => (await folder.read(path, entry.name)).data,
    children: async () => (await folder.list([...path, entry.name])).entries.map((e) => folderItem(folder, [...path, entry.name], e)),
  };
}

/** A File (an upload, a drop) as an FsItem. */
export function fileItem(file) {
  return { name: file.name, directory: false, size: file.size, read: async () => new Uint8Array(await file.arrayBuffer()), children: async () => [] };
}

/**
 * Everything below FsItem `dir` as tree items, parents first: `{path:
 * [names below dir], data?}` (no data: a directory).
 */
export async function flatten(dir, prefix = []) {
  const out = [];
  for (const child of await dir.children()) {
    const path = [...prefix, child.name];
    if (child.directory) {
      out.push({ path });
      out.push(...(await flatten(child, path)));
    } else {
      out.push({ path, data: await child.read() });
    }
  }
  return out;
}

const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** A list of names in words: `A`, `A and B`, `A, B and 2 more`. */
export function namesText(names, max = 3) {
  if (names.length <= 1) return names[0] ?? "";
  if (names.length <= max) return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`;
  return `${names.slice(0, max - 1).join(", ")} and ${names.length - max + 1} more`;
}

/** The question before replacing `names` in `where` ("the calculator", "hp-files"). */
export function replaceManyQuestion(names, where, { files = false } = {}) {
  if (names.length === 1) {
    return {
      title: `Replace ${names[0]}?`,
      body: files
        ? `${where} already has a file named ${names[0]}. The copy takes its place; the old contents are lost.`
        : `${where} already has a variable named ${names[0]} here. The copy takes its place; the old contents are lost.`,
      action: "Replace",
      danger: true,
    };
  }
  return {
    title: `Replace ${plural(names.length, files ? "file" : "variable")}?`,
    body: `${where} already has these. The copies take their place; the old contents are lost.`,
    action: "Replace all",
    danger: true,
    sections: [{ text: "Replaced:", items: names }],
  };
}

/** The question before purging (a calculator) or deleting (a folder) `names`; `count` what directories hold. */
export function removeQuestion(names, { device = true, count = 0, where = "" } = {}) {
  const verb = device ? "Purge" : "Delete";
  const one = names.length === 1;
  const inside = count ? ` ${one ? "It holds" : "They hold"} ${plural(count, device ? "variable" : "item")}, which go too.` : "";
  return {
    title: one ? `${verb} ${names[0]}?` : `${verb} ${plural(names.length, device ? "variable" : "item")}?`,
    body: `${one ? `${names[0]} is` : "They are"} removed from ${where} for good.${inside}`,
    action: verb,
    danger: true,
    sections: one ? [] : [{ text: `${verb}d:`, items: names }],
  };
}

/** The question before a restore. */
export function restoreQuestion(fileName, title) {
  return {
    title: "Replace everything on the calculator?",
    body: `${title} gets the contents of ${fileName}: what HOME holds now is lost. The calculator restarts and leaves server mode.`,
    action: "Restore",
    danger: true,
  };
}

/** A device failure or a folder failure as a sentence. */
export const problemText = (err) => (err?.name === "FolderError" ? err.message : failureText(err));

export class Transfers {
  /**
   * `queue` a JobQueue; `helpers()` the wasm's file and name functions
   * (saturnus-web's tx module); `ask(question)` confirm.js's question, resolves to
   * true for the action; `askName({title, why, suggested, action,
   * check})` resolves to a name or null; `say(text, {error})` a toast;
   * `mark(place, path, name, value)` shows a row's progress where a pane
   * shows it (`<device-list>`'s `setBusy`: a fraction, null not known yet,
   * undefined done); `frame(fn)` the next animation frame (the marks are
   * painted at most once per frame).
   */
  constructor({ queue, helpers, ask, askName, say, mark = () => {}, frame = (fn) => fn() }) {
    this.queue = queue;
    this.helpers = helpers;
    this.ask = ask;
    this.askName = askName;
    this.say = say;
    this.mark = mark;
    this.frame = frame;
    /** Whether the files in this browser were said not to be on the disk (once a visit). */
    this.saidBrowser = false;
  }

  /**
   * Copy `items` from `from` (a place, or null for files from outside the
   * page) at `fromPath` to `to` at `toPath`. Plans, asks, then queues a
   * job per item; resolves once every job has ended to `{copied, failed,
   * skipped}` (names), or null when the user said no.
   */
  async copy({ from = null, fromPath = null, items, to, toPath, single = false }) {
    if (!items.length) return null;
    let plan;
    try {
      plan = await this.plan({ from, fromPath, items, to, toPath, single });
    } catch (err) {
      // A cable pulled, a permission taken back, a file gone while planning: said, nothing sent.
      if (!["cancelled", "aborted", "closed"].includes(err?.kind)) this.say(`Nothing was copied: ${problemText(err)}`, { error: true });
      return null;
    }
    if (!plan) return null;
    if (!plan.steps.length) {
      this.say(plan.nothing ?? "Nothing there can be copied.", { error: true });
      return null;
    }
    // One file that is no calculator object: the question names it.
    const lone = plan.steps.length === 1 && plan.sections.length === 1 && plan.stringsOnly;
    if (plan.sections.length && !(await this.ask(lone ? stringQuestion(plan.steps[0].name, plan.steps[0].target) : {
      title: `Copy ${plural(plan.steps.length, "item")} to ${to.title}?`,
      body: plan.lead ?? "Before anything is copied:",
      action: "Copy",
      danger: false,
      sections: plan.sections,
    }))) return null;
    if (plan.inUse.length) {
      const where = to.kind === "device" ? "The calculator" : to.title;
      if (!(await this.ask(replaceManyQuestion(plan.inUse, where, { files: to.kind === "folder" })))) return null;
      for (const s of plan.steps) if (plan.inUse.includes(s.target)) s.replace = true;
    }
    const places = [from?.id, to.id].filter((p) => p !== undefined && p !== null);
    const result = { copied: [], failed: [], skipped: [] };
    await Promise.all(plan.steps.map((step) => this.queue
      .add({ label: `Copy ${step.name} to ${to.title}`, places, total: step.size ?? null, run: ({ report }) => this.marked(step, { from, fromPath, to, toPath }, report) })
      .then((note) => {
        result.copied.push(step.name);
        if (note) this.say(note);
      }, (err) => {
        if (err?.kind === "cancelled") result.skipped.push(step.name);
        else {
          result.failed.push(step.name);
          if (err?.kind !== "declined") this.say(`${step.name} was not copied: ${problemText(err)}`, { error: true });
        }
      })));
    if (result.copied.length) {
      const done = result.copied.length === 1 ? `${result.copied[0]} copied to ${to.title}.` : `${plural(result.copied.length, "item")} copied to ${to.title}.`;
      // The files in this browser are not in Downloads (iteration 45): said with the first copy there.
      if (to.browser && !this.saidBrowser) {
        this.saidBrowser = true;
        this.say(`${done} ${result.copied.length === 1 ? "It is" : "They are"} not on your disk: use ⋯ → Download to save a file.`);
      } else this.say(done);
    }
    return result;
  }

  /**
   * Run `step` with its rows marked: the source row (a calculator's or a
   * folder's) and the target row, when the panes show them (a target that
   * is not there yet gets the mark if it shows up during the copy).
   */
  async marked(step, { from, fromPath, to, toPath }, report) {
    const rows = [];
    if (from && fromPath) rows.push([from, fromPath, step.name]);
    rows.push([to, toPath, step.target ?? step.name]);
    const set = (value) => {
      for (const [place, path, name] of rows) this.mark(place, path, name, value);
    };
    set(null);
    // The latest value, painted once per frame; none after the step ended.
    let latest = null;
    let pending = false;
    let ended = false;
    try {
      return await this.watch([from, to], (done, total, label) => {
        report(done, total, label);
        latest = total ? Math.min(done / total, 1) : null;
        if (pending) return;
        pending = true;
        this.frame(() => {
          pending = false;
          if (!ended) set(latest);
        });
      }, () => step.run(step));
    } finally {
      ended = true;
      set(undefined);
    }
  }

  /**
   * Forward the progress of the calculators among `places` to `report`
   * while `fn` runs, added up: a copy between two calculators gets from
   * one, then puts to the other, and its count only grows. Before the
   * second one reports, its total is taken to be the first one's, so the
   * bar does not run to full twice.
   */
  async watch(places, report, fn) {
    const devices = [...new Set(places.filter((p) => p?.kind === "device").map((p) => p.device))];
    const seen = new Map();
    const off = [];
    for (const d of devices) {
      const on = (e) => {
        seen.set(d, { done: e.detail.done ?? 0, total: e.detail.total ?? null });
        let done = 0;
        let total = 0;
        let last = null;
        for (const v of seen.values()) {
          done += v.done;
          if (v.total) {
            total += v.total;
            last = v.total;
          }
        }
        // The phases not begun yet: as large as the last one known.
        if (last) total += (devices.length - seen.size) * last;
        report(done, total ? Math.max(total, done) : null, e.detail.label ?? "");
      };
      d.addEventListener("progress", on);
      off.push(() => d.removeEventListener("progress", on));
    }
    try {
      return await fn();
    } finally {
      for (const f of off) f();
    }
  }

  /**
   * The steps of a copy and what to say before it: `{steps: [{name,
   * target, size, replace, run}], inUse: [target names], sections:
   * [{text, items}]}`, or null when a name the user was asked for was not
   * given.
   */
  async plan({ from, fromPath, items, to, toPath, single }) {
    const h = await this.helpers();
    const target = (await (to.kind === "device" ? to.device.list(toPath) : to.folder.list(toPath))).entries;
    const find = (name) => target.find((e) => e.name === name) ?? null;
    if (from?.kind === "device" && to.kind === "device") return this.planDevices(h, { from, fromPath, items, to, toPath, target });
    const steps = [];
    const inUse = [];
    const blocked = [];
    const strings = [];
    const families = [];
    let familyMessage = null;
    const sections = [];
    if (from?.kind === "device") {
      // A calculator to a folder: binary files, a directory as a folder (or one file).
      for (const e of items) {
        const asFile = !e.directory || single;
        const fileName = asFile ? h.file_name(e.name, "binary") : h.folder_name(e.name);
        const there = to.folder.existing ? await to.folder.existing(toPath, fileName) : null;
        if (there && (!there.exact || there.directory !== !asFile)) {
          blocked.push(there.exact ? `${fileName} (a ${there.directory ? "folder" : "file"} of that name is there)` : `${fileName} (${there.name} differs only in case)`);
          continue;
        }
        const step = { name: e.name, target: fileName, size: e.size ?? null, replace: false };
        if (there && asFile) inUse.push(fileName);
        if (asFile) {
          step.run = async (s) => {
            const got = await from.device.read(fromPath, e.name, { size: e.size });
            await to.folder.write(toPath, fileName, got.data, { replace: s.replace });
          };
        } else {
          step.run = async () => {
            const tree = await from.device.getTree([...fromPath, e.name], { size: e.size });
            const files = tree.items.map((i) => (i.directory ? { path: i.fsPath } : { path: i.fsPath, data: i.data }));
            await this.writeTree(to, toPath, fileName, files);
          };
        }
        steps.push(step);
      }
    } else {
      // Files: from a folder, or dropped from outside the page.
      for (const item of items) {
        if (to.kind === "folder") {
          // One storage in both panes (two panes on the page's files, one folder picked twice):
          // not onto itself, and a folder not into itself.
          if (from?.kind === "folder" && (await sameStorage(from.folder, to.folder))) {
            const below = fromPath.slice(1);
            const target = toPath.slice(1);
            const inside = [...below, item.name];
            if (below.join("/") === target.join("/")) {
              blocked.push(`${item.name} (it is already here)`);
              continue;
            }
            if (item.directory && inside.every((p, i) => target[i] === p)) {
              blocked.push(`${item.name} (a folder cannot go into itself)`);
              continue;
            }
          }
          const there = await to.folder.existing(toPath, item.name);
          if (there && (!there.exact || there.directory !== item.directory)) {
            blocked.push(there.exact ? `${item.name} (a ${there.directory ? "folder" : "file"} of that name is there)` : `${item.name} (${there.name} differs only in case)`);
            continue;
          }
          const step = { name: item.name, target: item.name, size: item.size, replace: false };
          if (item.directory) step.run = async () => this.writeTree(to, toPath, item.name, await flatten(item));
          else {
            if (there) inUse.push(item.name);
            step.run = async (s) => {
              await to.folder.write(toPath, item.name, await item.read(), { replace: s.replace });
            };
          }
          steps.push(step);
          continue;
        }
        // To a calculator.
        const model = to.model;
        if (item.directory) {
          let name;
          try {
            name = h.dir_name(item.name);
          } catch {
            name = null;
          }
          if (!name || h.name_problem(name)) {
            name = await this.askName({ title: "A name for the directory", why: `The calculator does not take “${name ?? item.name}” as a name. What should the folder ${item.name} be called there?`, suggested: suggestName(name ?? item.name), action: "Copy" });
            if (!name) return null;
          }
          const there = find(name);
          if (there && !there.directory) {
            blocked.push(`${name} (a variable of that name is there)`);
            continue;
          }
          const files = await flatten(item);
          for (const f of files.filter((f) => f.data)) {
            const kind = h.file_kind(f.data);
            if (kind === "other") strings.push([item.name, ...f.path].join("/"));
            else if (kind === "binary" && model) {
              const m = h.header_check(f.data, model);
              if (m) {
                families.push([item.name, ...f.path].join("/"));
                familyMessage ??= m.message;
              }
            }
          }
          const size = files.reduce((n, f) => n + (f.data?.length ?? 0), 0);
          steps.push({
            name: item.name,
            target: name,
            size,
            replace: false,
            run: async () => this.putTree(to, there ? [...toPath, name] : toPath, there ? null : name, files, { asString: strings.length > 0 }),
          });
          continue;
        }
        const data = await item.read();
        let name;
        try {
          name = h.var_name(item.name);
        } catch {
          name = null;
        }
        if (!name || h.name_problem(name)) {
          name = await this.askName({ title: "A name for it", why: `The calculator does not take “${name ?? item.name}” as a name. What should ${item.name} be called there?`, suggested: suggestName(name ?? item.name), action: "Put" });
          if (!name) return null;
        }
        const there = find(name);
        if (there?.directory) {
          blocked.push(`${name} (a directory of that name is there)`);
          continue;
        }
        const kind = h.file_kind(data);
        if (kind === "other") strings.push(item.name);
        else if (kind === "binary" && model) {
          const m = h.header_check(data, model);
          if (m) {
            families.push(item.name);
            familyMessage ??= m.message;
          }
        }
        if (there) inUse.push(name);
        steps.push({
          name: item.name,
          target: name,
          size: data.length,
          replace: false,
          run: async (s) => {
            await to.device.write(toPath, name, data, { replace: s.replace, asString: kind === "other" });
          },
        });
      }
    }
    if (families.length) {
      sections.push({ text: familyMessage ?? "These files were made for the other calculator family: the calculator stores them as strings, not as the objects.", items: families });
    }
    if (strings.length) sections.push({ text: "These are not calculator files. The calculator keeps their contents as strings:", items: strings });
    if (blocked.length) sections.push({ text: "Left out, because the name is taken by something a copy does not replace:", items: blocked });
    return { steps, inUse, sections, stringsOnly: strings.length > 0 && !families.length && !blocked.length, nothing: blocked.length ? `Nothing was copied: ${namesText(blocked)}.` : null };
  }

  /** A copy between two calculators: saturnus-tx's copy plan, its warnings, a get and a put per item. */
  planDevices(h, { from, fromPath, items, to, toPath, target }) {
    const strip = (e) => ({ name: e.name, kind: e.kind, size: e.size, checksum: e.checksum });
    const plan = h.copy_plan(JSON.stringify({ from: from.model, to: to.model, fromDir: fromPath, selection: items.map(strip), target: target.map(strip) }));
    const sections = [];
    // The warning names what it is about: every item that goes, a directory with all it holds.
    const going = plan.steps.filter((s) => !plan.blocked.includes(s.name)).map((s) => (s.directory ? `${s.name} (a directory, with all it holds)` : s.name));
    if (plan.warning && going.length) sections.push({ text: plan.warning, items: going });
    if (plan.untranslatable.length) sections.push({ text: "Left out: these have no text form the other calculator can read.", items: plan.untranslatable });
    if (plan.blocked.length) sections.push({ text: "Left out: a directory has the name on one side (purge it first to copy these).", items: plan.blocked });
    const sizes = new Map(items.map((e) => [e.name, e.size]));
    const steps = plan.steps.filter((s) => !plan.blocked.includes(s.name)).map((s) => ({
      name: s.name,
      target: s.name,
      size: sizes.get(s.name) ?? null,
      replace: false,
      run: async (step) => {
        if (s.directory) {
          const tree = await from.device.getTree(s.get.dir, { format: s.get.format, skip: s.get.skip ?? [], size: step.size });
          const files = tree.items.map((i) => (i.directory ? { path: i.fsPath } : { path: i.fsPath, data: i.data }));
          await to.device.putTree(toPath, s.name, files);
          if (tree.skipped?.length) return `${s.name}: left out ${namesText(tree.skipped.map((k) => k.path.join("/")))} (no text form the other calculator reads).`;
          return null;
        }
        const got = await from.device.read(s.get.dir, s.get.name, { text: s.get.format === "text", size: step.size });
        await to.device.write(toPath, s.name, got.data, { replace: step.replace });
        return null;
      },
    }));
    return { steps, inUse: plan.inUse.filter((n) => steps.some((s) => s.name === n)), sections, nothing: plan.untranslatable.length || plan.blocked.length ? `Nothing was copied: ${namesText([...plan.untranslatable, ...plan.blocked])} cannot go there.` : null };
  }

  /**
   * Put tree `files` into `path` of a calculator (as directory `name`,
   * or into `path` itself when null). Variables in use ask once, then go
   * again with `replace`.
   */
  async putTree(to, path, name, files, { asString = false } = {}) {
    try {
      await to.device.putTree(path, name, files, { asString });
    } catch (err) {
      if (err?.kind !== "namesInUse") throw err;
      if (!(await this.ask(replaceManyQuestion(err.names ?? [], "The calculator")))) throw Object.assign(new Error("not replaced"), { kind: "declined" });
      await to.device.putTree(path, name, files, { asString, replace: true });
    }
  }

  /**
   * Write tree `files` (`{path, data?}` below it) as folder `root` in
   * `path` of a folder place, made if missing, merged into if there; the
   * files already there are replaced only after asking.
   */
  async writeTree(to, path, root, files) {
    const folder = to.folder;
    const top = [...path, root];
    const there = await folder.existing(path, root);
    const conflicts = [];
    if (there?.exact && there.directory) {
      for (const f of files) {
        const at = [...top, ...f.path.slice(0, -1)];
        const found = await folder.existing(at, f.path.at(-1)).catch(() => null);
        if (found && (!found.exact || found.directory !== !f.data)) {
          throw Object.assign(new Error(`${[root, ...f.path].join("/")}: ${found.exact ? `a ${found.directory ? "folder" : "file"} of that name is there` : `${found.name} differs only in case`}.`), { name: "FolderError" });
        }
        if (found && f.data) conflicts.push([root, ...f.path].join("/"));
      }
      if (conflicts.length && !(await this.ask(replaceManyQuestion(conflicts, to.title, { files: true })))) {
        throw Object.assign(new Error("not replaced"), { kind: "declined" });
      }
    } else {
      await folder.mkdir(path, root);
    }
    for (const f of files) {
      const at = [...top, ...f.path.slice(0, -1)];
      const name = f.path.at(-1);
      if (!f.data) {
        const found = await folder.existing(at, name);
        if (!found) await folder.mkdir(at, name);
      } else {
        await folder.write(at, name, f.data, { replace: conflicts.includes([root, ...f.path].join("/")) });
      }
    }
  }
}

/** Whether folders `a` and `b` are one storage: the same Folder, or roots the browser says are the same entry. */
export async function sameStorage(a, b) {
  if (a === b) return true;
  if (typeof a?.root?.isSameEntry !== "function") return false;
  try {
    return Boolean(await a.root.isSameEntry(b.root));
  } catch {
    return false;
  }
}

/** A name the calculator may take, made from `name`: letters and digits, not starting with a digit. */
export function suggestName(name) {
  return String(name).replace(/\.[^.]*$/, "").replace(/[^A-Za-z0-9]/g, "").replace(/^[0-9]+/, "").slice(0, 16) || "NAME";
}
