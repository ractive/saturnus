// The memory view's tree and flags panel: finding variables in the tree
// and the rows of the flags panel. The objects' text forms and previews
// are web/shared/objects.js. Pure functions, no DOM: tested in web/test/
// under Node.

// ------------------------------------------------------------ variables

/** The variables of the directory at `path` (`["HOME", "A"]`) in `tree`, or null. */
export function directoryAt(tree, path) {
  let vars = tree;
  for (const name of path.slice(1)) {
    const dir = vars?.find((v) => v.name === name && Array.isArray(v.variables));
    if (!dir) return null;
    vars = dir.variables;
  }
  return vars ?? null;
}

/** Every variable under `vars` whose name contains `needle`, with its path. */
export function findVariables(vars, needle, path = ["HOME"], out = []) {
  const n = needle.toLowerCase();
  for (const v of vars ?? []) {
    if (v.name.toLowerCase().includes(n)) out.push({ variable: v, path });
    if (Array.isArray(v.variables)) findVariables(v.variables, needle, [...path, v.name], out);
  }
  return out;
}

// ------------------------------------------------------------ flags

/** The name of the setting in `values` whose flags are as in `on` (a Set), or "". */
function settingOf(values, on) {
  const v = values.find((x) => x.set.every((f) => on.has(f)) && !x.clear.some((f) => on.has(f)));
  return v?.name ?? "";
}

/** The flag numbers that are set, from a `flags` reply. */
export function setFlags(flags) {
  return new Set(flags?.set ?? []);
}

/**
 * The rows of the flags panel for one model's entry of flags.json and a
 * `flags` reply: `{topics: [{topic, rows}], undocumented: [{flag, set, status}]}`.
 * A row is `{first, last, label, name, status, bits: [bool], set, now, other, field, value}`:
 * `now` is the meaning of the current state, `other` that of the opposite;
 * `value`, for a field with named settings (flags.json `values`), the one
 * the flags hold now, "" for a combination none names, else null.
 */
export function flagRows(entry, topics, flags) {
  const on = setFlags(flags);
  const byTopic = new Map(topics.map((t) => [t, []]));
  const undocumented = [];
  for (const e of entry?.system ?? []) {
    const numbers = [];
    for (let n = e.first; n >= e.last; n--) numbers.push(n);
    const bits = numbers.map((n) => on.has(n));
    const documented = e.status !== "unknown" && e.status !== "unused" ? true : Boolean(e.clear || e.set || e.field);
    if (!documented) {
      for (const n of numbers) undocumented.push({ flag: n, set: on.has(n), status: e.status });
      continue;
    }
    const single = e.first === e.last;
    const set = single ? bits[0] : null;
    const row = {
      first: e.first,
      last: e.last,
      label: single ? String(e.first) : `${e.first} … ${e.last}`,
      name: e.name,
      status: e.status,
      bits,
      set,
      field: e.field ?? null,
      now: single ? ((set ? e.set : e.clear) ?? null) : null,
      other: single ? ((set ? e.clear : e.set) ?? null) : null,
      value: e.values ? settingOf(e.values, on) : null,
    };
    if (!byTopic.has(e.topic)) byTopic.set(e.topic, []);
    byTopic.get(e.topic).push(row);
  }
  return {
    topics: [...byTopic].filter(([, rows]) => rows.length).map(([topic, rows]) => ({ topic, rows })),
    undocumented,
  };
}
