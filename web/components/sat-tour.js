// <sat-tour>: the tour of the page (kb iteration 35). Rings one element at
// a time and puts a short bubble beside it (a sheet at the bottom on a
// phone), with Back, Next, Skip tour and a count; Esc ends it. The steps
// are data (web/tour.js); the page opens what a step needs through the
// hooks app.js hands over (`setup`, `snapshot`, `restore`), which only
// open and close views, never touch the calculator, and put everything
// back when the tour ends. The bubble is a non-modal dialog: the page
// stays usable, and the calculator leaves the keys typed in it alone.
// Also the quiet line that offers the tour on a first visit. Light DOM.

import { OFFER_KEY, TourRun, chapter, context, fillText, nextChapter, offerShown } from "../tour.js";
import { icon } from "./icons.js";

const TEMPLATE = `
  <div class="tour-ring" hidden></div>
  <dialog class="tour-bubble" aria-labelledby="tour-title" aria-describedby="tour-text">
    <p class="tour-count"><span class="tour-chapter"></span><span class="tour-step"></span></p>
    <h2 id="tour-title" class="tour-title"></h2>
    <p id="tour-text" class="tour-text"></p>
    <p class="tour-next-chapter" hidden><button type="button" class="link tour-go-on"></button></p>
    <div class="tour-actions">
      <button type="button" class="link tour-skip">Skip tour</button>
      <button type="button" class="tour-back">Back</button>
      <button type="button" class="primary tour-next">Next</button>
    </div>
  </dialog>
  <div class="tour-offer" role="region" aria-label="Tour" hidden>
    <span>New here?</span>
    <button type="button" class="link tour-offer-go">Take a 2-minute tour.</button>
    <button type="button" class="icon tour-offer-close" title="No thanks" aria-label="No thanks">${icon("close")}</button>
  </div>`;

/** Gap between the ring and the bubble, and the bubble and the window's edge. */
const GAP = 12;
const EDGE = 8;
/** How long the ring follows its anchor after a step opens (a sheet or a view sliding in). */
const FOLLOW_MS = 600;

const PHONE = "(max-width: 759px)";
const nextFrame = () => new Promise((r) => requestAnimationFrame(() => r()));
const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));

/**
 * Whether `el` is seen: rendered, not hidden, of a size, in the window
 * and not covered by something else at its middle (the memory view over
 * the calculator, say).
 */
export function seen(el) {
  if (!el?.isConnected || el.closest("[hidden]")) return false;
  if (el.checkVisibility && !el.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true })) return false;
  const r = el.getBoundingClientRect();
  if (r.width < 2 || r.height < 2) return false;
  const vw = document.documentElement.clientWidth;
  const vh = window.innerHeight;
  if (r.right <= 0 || r.bottom <= 0 || r.left >= vw || r.top >= vh) return false;
  const x = clamp(r.left + r.width / 2, 1, vw - 1);
  const y = clamp(r.top + r.height / 2, 1, vh - 1);
  const hit = document.elementFromPoint(x, y);
  return Boolean(hit && (el.contains(hit) || hit.contains(el)));
}

export class SatTour extends HTMLElement {
  connectedCallback() {
    if (this.bubble) return;
    this.innerHTML = TEMPLATE;
    const $ = (sel) => this.querySelector(sel);
    this.ring = $(".tour-ring");
    this.bubble = $(".tour-bubble");
    this.offerLine = $(".tour-offer");
    this.run = null;
    this.busy = Promise.resolve();
    $(".tour-next").addEventListener("click", () => this.step(1));
    $(".tour-back").addEventListener("click", () => this.step(-1));
    $(".tour-skip").addEventListener("click", () => this.end());
    $(".tour-go-on").addEventListener("click", () => {
      // The next chapter; the focus still goes back where the first began.
      const next = this.goOn;
      const back = this.returnTo;
      this.end({ keep: true }).then(() => next && this.start(next.id, { returnTo: back }));
    });
    $(".tour-offer-go").addEventListener("click", () => this.start("start"));
    $(".tour-offer-close").addEventListener("click", () => {
      this.prefs?.set("tour", "dismissed");
      this.offerLine.hidden = true;
    });
    this.onKey = (e) => {
      if (e.key !== "Escape" || !this.run) return;
      // A modal dialog opened over the tour (Search, say) keeps its Esc.
      if ([...document.querySelectorAll("dialog[open]")].some((d) => d.matches(":modal"))) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      this.end();
    };
    this.onMove = () => this.place();
    // The offer lives on the stage, over the top of the calculator's case.
    document.getElementById("stage")?.append(this.offerLine);
  }

  /**
   * `hooks`: `setup(kind)`, `snapshot()`, `restore(snap)` (app.js);
   * `store` (the running model), `bindings` (the keys named), `prefs`
   * (the offer's answer), `host` ("browser" or "app"), `mac`.
   */
  attach({ hooks, store, bindings, prefs, host, mac }) {
    this.hooks = hooks;
    this.store = store;
    this.bindings = bindings;
    this.prefs = prefs;
    this.host = host;
    this.mac = mac;
  }

  /** Where the tour runs now (web/tour.js `context`). */
  context() {
    return context({
      host: this.host,
      pointer: matchMedia("(pointer: coarse)").matches ? "coarse" : "fine",
      phone: matchMedia(PHONE).matches,
      mac: this.mac,
      booted: this.store?.state.booted ?? null,
      keys: (id) => this.bindings?.labelOf(id) ?? "",
    });
  }

  /** The quiet line offering the tour, unless it was dismissed or taken. */
  offer() {
    this.offerLine.hidden = !offerShown(this.prefs?.get("tour"));
  }

  /** Run `fn` after what is under way; a failure ends the tour and leaves the queue usable. */
  queue(fn) {
    return this.busy.then(fn).catch((err) => {
      console.warn("saturnus: the tour stopped:", err);
      return this.close().then(() => null, () => null);
    });
  }

  isOpen() {
    return Boolean(this.run);
  }

  /**
   * Start chapter `id` ("start", "calculator", "memory") from its first
   * step; the focus goes back to `returnTo` (else where it is now) after.
   */
  start(id = "start", { returnTo = null } = {}) {
    const c = chapter(id);
    if (!c) return Promise.resolve(false);
    this.busy = this.queue(async () => {
      if (this.run) await this.close({ keep: false });
      this.prefs?.set("tour", "taken");
      this.offerLine.hidden = true;
      const ctx = this.context();
      this.ctx = ctx;
      const active = document.activeElement;
      this.returnTo = returnTo ?? (active instanceof HTMLElement && active !== document.body && !this.contains(active) ? active : null);
      this.snap = this.hooks.snapshot();
      this.run = new TourRun(c, ctx);
      window.addEventListener("keydown", this.onKey, true);
      window.addEventListener("resize", this.onMove);
      document.addEventListener("scroll", this.onMove, true);
      const first = await this.run.go(1, (s) => this.show(s));
      if (!first) {
        await this.close({ keep: false });
        return false;
      }
      this.bubble.querySelector(".tour-next").focus({ preventScroll: true });
      return true;
    });
    return this.busy;
  }

  /** Next (1) or Back (-1); Next past the last step ends the chapter. */
  step(dir) {
    if (!this.run) return Promise.resolve(null);
    this.busy = this.queue(async () => {
      if (!this.run) return null;
      const s = await this.run.go(dir, (x) => this.show(x));
      if (!s) await this.close({ keep: false });
      else if (!this.bubble.contains(document.activeElement) || document.activeElement.offsetParent === null) {
        this.bubble.querySelector(".tour-next").focus({ preventScroll: true });
      }
      return s?.id ?? null;
    });
    return this.busy;
  }

  /** End the tour: the views as they were, the focus where it was. */
  end(opts) {
    this.busy = this.queue(() => this.close(opts));
    return this.busy;
  }

  async close({ keep = false } = {}) {
    if (!this.run) return;
    this.run = null;
    window.removeEventListener("keydown", this.onKey, true);
    window.removeEventListener("resize", this.onMove);
    document.removeEventListener("scroll", this.onMove, true);
    this.ring.hidden = true;
    this.bubble.close();
    document.body.classList.remove("tour-room");
    this.anchor = null;
    await this.hooks.restore(this.snap);
    const back = this.returnTo;
    this.returnTo = null;
    if (!keep && back?.isConnected && seen(back)) back.focus({ preventScroll: true });
  }

  /** Open what `step` needs and find its anchor; true when it is seen. */
  async reveal(step) {
    await this.hooks.setup(step.setup);
    await nextFrame();
    await nextFrame();
    const el = document.querySelector(step.anchor);
    if (!el) return null;
    el.scrollIntoView({ block: matchMedia(PHONE).matches ? "center" : "nearest", inline: "nearest" });
    await nextFrame();
    return seen(el) ? el : null;
  }

  /** Show `step` if its anchor can be seen; false (and nothing shown) if not. */
  async show(step) {
    // The tour's own parts out of the way of `seen`.
    this.bubble.classList.add("measuring");
    this.ring.hidden = true;
    const el = await this.reveal(step);
    this.bubble.classList.remove("measuring");
    if (!el) return false;
    this.anchor = el;
    const run = this.run;
    const index = run.steps.indexOf(step);
    const $ = (sel) => this.bubble.querySelector(sel);
    $(".tour-chapter").textContent = `${run.chapter.title} · `;
    $(".tour-step").textContent = `${index + 1} of ${run.total}`;
    $(".tour-title").textContent = step.title;
    $(".tour-text").textContent = fillText(step.text, this.ctx);
    $(".tour-back").hidden = index === 0;
    const last = index === run.total - 1;
    $(".tour-next").textContent = last ? "Done" : "Next";
    this.goOn = last ? nextChapter(run.chapter.id, this.context()) : null;
    $(".tour-next-chapter").hidden = !this.goOn;
    if (this.goOn) $(".tour-go-on").textContent = `Next: ${this.goOn.title}`;
    this.dataset.step = step.id;
    if (!this.bubble.open) this.bubble.show();
    this.ring.hidden = false;
    this.place();
    if (matchMedia(PHONE).matches) this.makeRoom(el);
    // Follow a view or a sheet still sliding in.
    const until = performance.now() + FOLLOW_MS;
    const follow = () => {
      if (this.anchor !== el || performance.now() > until) return;
      this.place();
      requestAnimationFrame(follow);
    };
    requestAnimationFrame(follow);
    return true;
  }

  /**
   * On a phone: `el` scrolled up out from under the sheet, the panel given
   * room at its end for the sheet so that its last element can be too.
   */
  makeRoom(el) {
    const top = this.bubble.getBoundingClientRect().top;
    if (el.getBoundingClientRect().bottom + GAP <= top) return;
    document.documentElement.style.setProperty("--tour-room", `${Math.ceil(this.bubble.offsetHeight)}px`);
    document.body.classList.add("tour-room");
    let box = el.parentElement;
    while (box && !(/(auto|scroll)/.test(getComputedStyle(box).overflowY) && box.scrollHeight > box.clientHeight)) box = box.parentElement;
    if (!box) return;
    const r = el.getBoundingClientRect();
    const room = Math.max(0, r.top - Math.max(0, box.getBoundingClientRect().top));
    box.scrollTop += Math.min(r.bottom + GAP - top, room);
    this.place();
  }

  /** The ring around the anchor and the bubble beside it (or the sheet at the bottom). */
  place() {
    const el = this.anchor;
    if (!el || !this.run) return;
    const vw = document.documentElement.clientWidth;
    const vh = window.innerHeight;
    const r = el.getBoundingClientRect();
    const ring = this.ring.style;
    const top = clamp(r.top - 4, 2, vh - 4);
    const left = clamp(r.left - 4, 2, vw - 4);
    ring.top = `${top}px`;
    ring.left = `${left}px`;
    ring.width = `${Math.max(0, clamp(r.right + 4, 2, vw - 2) - left)}px`;
    ring.height = `${Math.max(0, clamp(r.bottom + 4, 2, vh - 2) - top)}px`;
    const b = this.bubble;
    const phone = matchMedia(PHONE).matches;
    b.classList.toggle("sheet", phone);
    if (phone) {
      b.style.left = b.style.top = "";
      return;
    }
    const w = b.offsetWidth;
    const h = b.offsetHeight;
    const fits = {
      below: r.bottom + GAP + h <= vh - EDGE,
      above: r.top - GAP - h >= EDGE,
      right: r.right + GAP + w <= vw - EDGE,
      left: r.left - GAP - w >= EDGE,
    };
    let x;
    let y;
    // An anchor on the left (the side panel): the bubble beside it, not over the panel.
    const leftSide = r.right + GAP + w <= vw - EDGE && r.left + r.width / 2 < vw * 0.3;
    if (leftSide) {
      x = r.right + GAP;
      y = clamp(r.top + r.height / 2 - h / 2, EDGE, vh - h - EDGE);
    } else if (fits.below || fits.above) {
      x = clamp(r.left + r.width / 2 - w / 2, EDGE, vw - w - EDGE);
      y = fits.below ? r.bottom + GAP : r.top - GAP - h;
    } else if (fits.right || fits.left) {
      x = fits.right ? r.right + GAP : r.left - GAP - w;
      y = clamp(r.top + r.height / 2 - h / 2, EDGE, vh - h - EDGE);
    } else {
      // An anchor as large as the window (the calculator): inside it, at the bottom right.
      x = clamp(r.right - w - GAP, EDGE, vw - w - EDGE);
      y = clamp(r.bottom - h - GAP, EDGE, vh - h - EDGE);
    }
    b.style.left = `${Math.round(x)}px`;
    b.style.top = `${Math.round(y)}px`;
  }

  /**
   * For the tests: every step of chapter `id` that applies here, opened
   * as the tour would, with whether its anchor is seen; the views put
   * back after.
   */
  async probe(id) {
    const c = chapter(id);
    const ctx = this.context();
    const snap = this.hooks.snapshot();
    const out = [];
    try {
      for (const s of new TourRun(c, ctx).steps) out.push({ id: s.id, anchor: s.anchor, seen: Boolean(await this.reveal(s)) });
    } finally {
      await this.hooks.restore(snap);
    }
    return out;
  }
}

customElements.define("sat-tour", SatTour);
