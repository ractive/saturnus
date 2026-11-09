// The transfer page's queue (kb/docs/saturnus-tx.md, "List, get, put"):
// what the user asked for, one job at a time per place. A job names the
// places it uses (a copy from a calculator to a folder uses both); it
// starts once none of them runs another job and no job asked for earlier
// waits for one of them, so each place does its jobs in the order asked.
// Jobs that have not started can be cancelled; a running one reports its
// progress in bytes, and one given a `stop` (a palmtop's) can be stopped.
// Each place counts the jobs of its run (from its first job to the moment
// it has none), so a pane can say "Copying B… 2 of 3". A job that failed
// is kept as a failure (its sentence) until dismissed. The page shows the
// running job in the head of its pane (pane.js) and the list under the
// panes (app.js); pure, no DOM: web/test/tx-queue.test.mjs.
//
// Events: `change` (the jobs, a job's state or progress, the failures changed).

/** A job dropped before it started: its promise rejects with this. */
export class Cancelled extends Error {
  constructor(label) {
    super(`${label} was cancelled before it started.`);
    this.kind = "cancelled";
  }
}

export class JobQueue extends EventTarget {
  constructor() {
    super();
    /** Queued and running jobs, in the order asked: `{id, label, doing, places, state, done, total, note}`. */
    this.jobs = [];
    this.nextId = 1;
    /** The jobs each place has ended in its current run, by place id. */
    this.ended = new Map();
    /** Jobs that failed, oldest first, until dismissed: `{id, text}`. */
    this.failures = [];
  }

  changed() {
    this.dispatchEvent(new CustomEvent("change"));
  }

  /**
   * Queue `run(job)` on `places` (ids; a place is busy while one of its
   * jobs runs). `label` says what it is ("Copy PRG to hp-files"), `doing`
   * what is happening while it runs ("Copying PRG…"; `label` and "…" when
   * not given), `total`
   * the bytes expected (null: unknown), `stop(job)` ends it while it runs
   * (null: it cannot be stopped) and `stoppable(job)` says whether that
   * would act now (null: whenever it runs). Resolves or rejects as `run` does;
   * rejects with `Cancelled` when cancelled before it starts.
   */
  add({ label, doing = null, places, run, total = null, stop = null, stoppable = null }) {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    const job = { id: this.nextId++, label, doing: doing ?? `${label}…`, places: [...new Set(places)], state: "queued", done: 0, total, note: "", run, stop, stoppable, resolve, reject };
    this.jobs.push(job);
    this.changed();
    this.pump();
    return Object.assign(promise, { id: job.id });
  }

  /** The jobs running now. */
  get running() { return this.jobs.filter((j) => j.state === "running"); }

  /** The jobs waiting. */
  get waiting() { return this.jobs.filter((j) => j.state === "queued"); }

  /** Whether `place` has a job running or waiting. */
  uses(place) { return this.jobs.some((j) => j.places.includes(place)); }

  /** Whether running job `j` offers Stop now. */
  canStop(j) {
    return j.state === "running" && Boolean(j.stop) && (!j.stoppable || j.stoppable(j));
  }

  /** Whether `place` runs a job now. */
  busy(place) { return this.running.some((j) => j.places.includes(place)); }

  /**
   * The job `place` runs now and where it is in the place's run: `{job,
   * index, count}` ("2 of 3": two ended or running, one waiting), or null.
   */
  progressOf(place) {
    const job = this.running.find((j) => j.places.includes(place));
    if (!job) return null;
    const ended = this.ended.get(place) ?? 0;
    return { job, index: ended + 1, count: ended + this.jobs.filter((j) => j.places.includes(place)).length };
  }

  /** A run ends with the place's last job: the next one counts from one. */
  settle(places) {
    for (const p of places) if (!this.uses(p)) this.ended.delete(p);
  }

  /** Keep the failure `text` ("PRG was not copied: …") until dismissed; its id. */
  fail(text) {
    const id = this.nextId++;
    this.failures.push({ id, text });
    this.changed();
    return id;
  }

  /** Dismiss failure `id` (null: every one). */
  dismiss(id = null) {
    const before = this.failures.length;
    this.failures = id === null ? [] : this.failures.filter((f) => f.id !== id);
    if (this.failures.length !== before) this.changed();
  }

  /** Start every job that may start. */
  pump() {
    const taken = new Set();
    for (const j of this.running) for (const p of j.places) taken.add(p);
    let started = false;
    for (const job of this.jobs) {
      if (job.state === "queued" && job.places.every((p) => !taken.has(p))) {
        this.start(job);
        started = true;
      }
      // Running or not, a job holds its places for those asked after it.
      for (const p of job.places) taken.add(p);
    }
    if (started) this.changed();
  }

  start(job) {
    job.state = "running";
    const report = (done, total = job.total, note = job.note) => {
      if (job.state !== "running") return;
      job.done = done;
      job.total = total;
      job.note = note;
      this.changed();
    };
    Promise.resolve()
      .then(() => job.run({ report, job }))
      .then(job.resolve, job.reject)
      .finally(() => {
        this.jobs = this.jobs.filter((j) => j !== job);
        job.state = "done";
        for (const p of job.places) this.ended.set(p, (this.ended.get(p) ?? 0) + 1);
        this.settle(job.places);
        this.changed();
        this.pump();
      });
  }

  /** Cancel job `id` if it has not started; whether it was. */
  cancel(id) {
    const job = this.jobs.find((j) => j.id === id && j.state === "queued");
    if (!job) return false;
    this.jobs = this.jobs.filter((j) => j !== job);
    job.state = "cancelled";
    this.settle(job.places);
    job.reject(new Cancelled(job.label));
    this.changed();
    this.pump();
    return true;
  }

  /** Cancel every job not started (of `place` only, when given); how many. */
  cancelWaiting(place = null) {
    const drop = this.waiting.filter((j) => place === null || j.places.includes(place));
    for (const j of drop) this.cancel(j.id);
    return drop.length;
  }
}
