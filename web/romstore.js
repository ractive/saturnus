// The ROM slots of the Worker host (web/protocol.md, "ROM slots"): the
// ROM of each model is kept in this browser's IndexedDB (database
// `saturnus-roms`), so it is chosen once. Identification and assignment
// follow the rules every host shares (the wasm core's `identify_rom` and
// `plan_roms`, crates/saturnus-web/src/romid.rs). Nothing leaves the
// browser. When the browser refuses to store (storage blocked, quota),
// the slots still work until the page is closed, with a note saying so.

export const ROM_DB = "saturnus-roms";
const SLOTS = "slots";
const IMAGES = "images";
/** The key of the settings record in the slots store. */
const SETTINGS = "@settings";

const NOT_KEPT = "This browser does not keep ROMs (storage is blocked or full): choose them again after a reload.";

function promised(req) {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/**
 * The store over IndexedDB: `slots` holds `{name, sha256, revision}` per
 * model (and the settings), `images` the bytes by SHA-256, so the 39G and
 * 40G share one copy. Each call opens and closes the database.
 */
export function indexedDbStore(getIdb = () => globalThis.indexedDB) {
  async function open() {
    // Read here, not before: a blocked store may throw on the access.
    const idb = getIdb();
    if (!idb) throw new Error("IndexedDB is not available");
    const req = idb.open(ROM_DB, 1);
    req.onupgradeneeded = () => {
      req.result.createObjectStore(SLOTS);
      req.result.createObjectStore(IMAGES);
    };
    return promised(req);
  }
  async function run(mode, fn) {
    const db = await open();
    try {
      return await new Promise((resolve, reject) => {
        const tx = db.transaction([SLOTS, IMAGES], mode);
        let result;
        // A failed step aborts the whole transaction: a slot is never kept without its image.
        Promise.resolve()
          .then(() => fn(tx.objectStore(SLOTS), tx.objectStore(IMAGES)))
          .then((r) => { result = r; }, (err) => {
            try { tx.abort(); } catch { /* already finished */ }
            reject(err);
          });
        tx.oncomplete = () => resolve(result);
        tx.onerror = () => reject(tx.error);
        tx.onabort = () => reject(tx.error ?? new Error("the store was aborted"));
      });
    } finally {
      db.close();
    }
  }
  return {
    /** `{settings, slots: {model: {name, sha256, revision}}}`. */
    load: () => run("readonly", async (slots) => {
      const [keys, values] = await Promise.all([promised(slots.getAllKeys()), promised(slots.getAll())]);
      const out = { settings: null, slots: {} };
      keys.forEach((k, i) => {
        if (k === SETTINGS) out.settings = values[i];
        else out.slots[k] = values[i];
      });
      return out;
    }),
    image: (sha256) => run("readonly", (_, images) => promised(images.get(sha256))),
    /** Put a slot and its image; drop images no slot uses any more. */
    put: (model, rec, bytes) => run("readwrite", async (slots, images) => {
      slots.put(rec, model);
      images.put(bytes, rec.sha256);
      await prune(slots, images);
    }),
    settings: (settings) => run("readwrite", (slots) => { slots.put(settings, SETTINGS); }),
    /** Remove one model's slot, or every slot and image (`null`). */
    remove: (model) => run("readwrite", async (slots, images) => {
      if (model === null) {
        const settings = await promised(slots.get(SETTINGS));
        slots.clear();
        images.clear();
        if (settings) slots.put({ ...settings, lastModel: null }, SETTINGS);
        return;
      }
      slots.delete(model);
      await prune(slots, images);
    }),
  };
}

/** Delete the images no slot names. */
async function prune(slots, images) {
  const [keys, values, shas] = await Promise.all([
    promised(slots.getAllKeys()), promised(slots.getAll()), promised(images.getAllKeys()),
  ]);
  const used = new Set(values.filter((_, i) => keys[i] !== SETTINGS).map((v) => v.sha256));
  for (const sha of shas) if (!used.has(sha)) images.delete(sha);
}

/**
 * The slots, in memory and in the store. `identify(bytes)` and
 * `plan(input)` are the wasm core's `identify_rom` and `plan_roms`;
 * `boot(model, bytes, name)` boots the machine and returns `{model,
 * romName}`; `models` are the host's models, in order.
 */
export class RomStore {
  constructor({ identify, plan, boot, models, store = indexedDbStore() }) {
    this.identify = identify;
    this.plan = plan;
    this.bootMachine = boot;
    this.models = models;
    this.store = store;
    /** model -> {name, sha256, revision, bytes (once read)} */
    this.slotMap = new Map();
    this.offers = [];
    this.nextOffer = 1;
    this.lastModel = null;
    this.bootLast = true;
    /** Whether the store keeps what is put into it (null: not yet known). */
    this.remembered = null;
    this.note = null;
    this.loaded = null;
  }

  refused(err) {
    this.remembered = false;
    this.note = `${NOT_KEPT} (${err?.name ?? "Error"})`;
  }

  async keep(fn) {
    if (this.remembered === false) return;
    try {
      await fn();
    } catch (err) {
      this.refused(err);
    }
  }

  load() {
    this.loaded ??= (async () => {
      try {
        const { settings, slots } = await this.store.load();
        for (const [model, rec] of Object.entries(slots)) {
          if (this.models.includes(model) && rec?.sha256) this.slotMap.set(model, { ...rec, bytes: null });
        }
        this.lastModel = this.models.includes(settings?.lastModel) ? settings.lastModel : null;
        this.bootLast = settings?.bootLast ?? true;
        this.remembered = true;
      } catch (err) {
        this.refused(err);
      }
    })();
    return this.loaded;
  }

  /** `romSlots`'s result. */
  async slots() {
    await this.load();
    return {
      slots: this.models.map((model) => {
        const s = this.slotMap.get(model);
        return s
          ? { model, fileName: s.name, revision: s.revision ?? null, state: s.changed ? "changed" : "ready" }
          : { model, fileName: null, revision: null, state: "empty" };
      }),
      offers: this.offers.map(({ id, models, name }) => ({ id, models, fileName: name })),
      lastModel: this.lastModel,
      bootLast: this.bootLast,
      remembered: this.remembered,
      note: this.note,
    };
  }

  async result(booted, notice = "") {
    return { ...(await this.slots()), booted, notice };
  }

  async boot(model, slot) {
    const booted = this.bootMachine(model, slot.bytes, slot.name);
    if (this.lastModel !== booted.model) {
      this.lastModel = booted.model;
      await this.keep(() => this.store.settings({ bootLast: this.bootLast, lastModel: this.lastModel }));
    }
    return booted;
  }

  /** `bootModel`: boot `model` from its kept ROM, checked against its hash. */
  async bootModel(model) {
    await this.load();
    const s = this.slotMap.get(model);
    if (!s) throw new Error(`no ROM is kept for the ${model.toUpperCase()}`);
    if (!s.bytes) {
      let bytes;
      try {
        bytes = await this.store.image(s.sha256);
      } catch (err) {
        this.refused(err);
      }
      if (!bytes) {
        this.slotMap.delete(model);
        throw new Error(`${s.name}, the ${model.toUpperCase()} ROM, is no longer in this browser; choose it again`);
      }
      s.bytes = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    }
    if (this.identify(s.bytes).sha256 !== s.sha256) {
      s.changed = true;
      throw new Error(`${s.name}, the ${model.toUpperCase()} ROM, has changed in this browser's store; choose it again`);
    }
    return this.result(await this.boot(model, s));
  }

  async assign(model, rec) {
    const slot = { name: rec.name, sha256: rec.id.sha256, revision: rec.id.revision ?? null, bytes: rec.bytes };
    this.slotMap.set(model, slot);
    await this.keep(() => this.store.put(model, { name: slot.name, sha256: slot.sha256, revision: slot.revision }, slot.bytes));
  }

  /**
   * `chooseRom`: `files` (`[{name, rom}]`, chosen by the user) are
   * identified and assigned for `model`, or an earlier `offer` is taken.
   */
  async chooseRom(model, files, offer) {
    await this.load();
    if (offer !== undefined && offer !== null) {
      const o = this.offers.find((x) => x.id === offer && x.models.includes(model));
      if (!o) throw new Error("that offer is no longer open");
      await this.assign(model, o);
      o.models = o.models.filter((m) => m !== model);
      this.offers = this.offers.filter((x) => x.models.length);
      return this.result(await this.boot(model, this.slotMap.get(model)));
    }
    if (!Array.isArray(files) || !files.length) throw new Error('"files" must hold at least one ROM file');
    const recs = files.map((f) => {
      const bytes = f.rom instanceof Uint8Array ? f.rom : new Uint8Array(f.rom);
      return { name: String(f.name ?? ""), bytes, id: this.identify(bytes) };
    });
    const filled = this.models.filter((m) => this.slotMap.has(m) && !this.slotMap.get(m).changed);
    const p = this.plan({
      selected: model,
      filled,
      files: recs.map((r) => ({ name: r.name, chosen: true, id: r.id })),
    });
    for (const a of p.assign) await this.assign(a.model, recs[a.file]);
    this.offers = p.offer.map((o) => ({ id: this.nextOffer++, models: o.models, ...recs[o.file] }));
    const booted = p.boot ? await this.boot(p.boot, this.slotMap.get(p.boot)) : null;
    return this.result(booted, p.notice);
  }

  /** `forgetRom`: one model's ROM, or every ROM (`null`), and the offers. */
  async forget(model) {
    await this.load();
    if (model === null) {
      this.slotMap.clear();
      this.lastModel = null;
    } else {
      this.slotMap.delete(model);
    }
    this.offers = [];
    if (this.remembered !== false) {
      try {
        await this.store.remove(model);
      } catch (err) {
        this.refused(err);
      }
    }
    return this.result(null);
  }

  /** `romSettings`: whether the last model boots when the page opens. */
  async settings(bootLast) {
    await this.load();
    this.bootLast = Boolean(bootLast);
    await this.keep(() => this.store.settings({ bootLast: this.bootLast, lastModel: this.lastModel }));
    return this.result(null);
  }
}
