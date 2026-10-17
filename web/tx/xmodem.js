// The fast transfer (XModem) of the 48GX and 49G: what the page says and
// the dialog that says it (kb/docs/saturnus-tx.md, "Fast transfer").
// XRECV and XSEND do not run inside the calculator's server, so the page
// ends the server, shows what to type on the calculator ('NAME' XRECV or
// 'NAME' XSEND, then ENTER), waits, and afterwards offers to reconnect
// once SERVER runs again. An emulated calculator can have the command
// typed for it, and its server started again by the reconnect.
//
// `fastWords` and `fastDone` are pure (web/test/tx-flows.test.mjs); the
// dialog (`XmodemDialog`) is made once, on first use, so tx.html is not
// touched.

import { el } from "../shared/dom.js";
import { MODEL_NAMES, START_SERVER, failureText } from "./flows.js";

/** The models with the fast transfer. */
export const XMODEM_MODELS = ["48gx", "49g"];

/** How long the calculator has to start the transfer, in seconds. */
export const XMODEM_WAIT_S = 180;

/** Whether `model` (the page's name: "48gx") has the fast transfer. */
export const hasXmodem = (model) => XMODEM_MODELS.includes(model);

/**
 * The words of the dialog while it waits: `title`, `lead` (before the
 * plan: what happens; with it: what to do), `keys` (what to type),
 * `notes` (sentences), for a `direction` ("put" or "get") of variable
 * `name` with `model`, `plan` the device's (`{keys, switchedToRpn}`)
 * once the server has ended.
 */
export function fastWords({ direction, name, model, plan = null, emulated = false, waitS = XMODEM_WAIT_S }) {
  const put = direction === "put";
  const title = put ? `Fast transfer of ${name} to the calculator` : `Fast transfer of ${name} from the calculator`;
  const who = emulated ? "the emulated calculator" : `the ${MODEL_NAMES[model] ?? "calculator"}`;
  if (!plan) {
    return {
      title,
      lead: `The page ends server mode on ${who}, then you start the transfer on it.`,
      keys: null,
      notes: [],
      status: "Ending server mode…",
    };
  }
  const notes = [];
  if (plan.switchedToRpn) notes.push("The calculator was in algebraic mode, so the page switched it to RPN for the command. It goes back when you reconnect.");
  if (put && model === "48gx") notes.push(`If the transfer fails, ${name} may be left on the calculator as an empty string.`);
  notes.push("To stop it on the calculator, press ON.");
  const minutes = Math.round(waitS / 60);
  return {
    title,
    lead: `Server mode has ended. On ${who}, type this and press ENTER:`,
    keys: plan.keys,
    notes,
    status: `Waiting for the calculator (up to ${minutes} ${minutes === 1 ? "minute" : "minutes"})…`,
  };
}

/**
 * The words once it is over: `lead` (what happened), `restart` (how the
 * server comes back; null when it never ended), `ok`. `error` the
 * failure (a TxFailure), null for success; `where` what a get became
 * ("PRG.hp in hp-files", "PRG.hp in your downloads"); `left` whether the
 * server ended.
 */
export function fastDone({ direction, name, model, error = null, where = null, left = true, emulated = false }) {
  const put = direction === "put";
  let lead;
  if (!error) lead = put ? `${name} is on the calculator.` : `${name} came over: ${where ?? `${name}.hp`}.`;
  else if (!left && (error.kind === "aborted" || error.kind === "cancelled")) lead = "Cancelled before it started: the calculator is still in server mode.";
  else if (error.kind === "aborted" || error.kind === "cancelled") lead = "Stopped. If the calculator still waits, press ON on it.";
  else if (error.kind === "xmodemTimeout") lead = `Nothing came from the calculator in time. Was ${put ? "XRECV" : "XSEND"} typed and ENTER pressed? If the calculator still waits, press ON on it.`;
  else lead = `Could not ${put ? "send" : "get"} ${name}: ${failureText(error)}`;
  if (!left) return { lead, restart: null, keys: null, ok: !error };
  const s = START_SERVER[model];
  const restart = emulated
    ? "The emulated calculator is out of server mode. Reconnect starts its server again."
    : `The calculator is out of server mode. ${s ? `${s.steps[0]} That starts its server again; then reconnect.` : "Start its server again, then reconnect."}`;
  return { lead, restart, keys: emulated ? null : s?.keys ?? null, ok: !error };
}

/**
 * The fast transfer's dialog: made once, shown per transfer. `start()`
 * shows it; `plan(plan)` once the server has ended; `progress(text)`;
 * `finish(words)` with `fastDone`'s words. Its buttons call the hooks
 * given to `start`: `cancel` (while it runs) and `type` (an emulated
 * calculator: type the command); `finish` resolves to the button that
 * closed it.
 */
export class XmodemDialog {
  constructor(doc = document) {
    this.doc = doc;
    this.dialog = null;
    this.hooks = {};
    this.state = null;
  }

  build() {
    if (this.dialog) return;
    const q = (cls) => this.dialog.querySelector(`.${cls}`);
    this.dialog = el("dialog", { id: "xmodem-dialog", class: "tx-dialog xmodem-dialog", "aria-labelledby": "xmodem-title" },
      el("div", { class: "tx-dialog-box" },
        el("h2", { id: "xmodem-title", class: "xmodem-title" }),
        el("p", { class: "xmodem-lead" }),
        el("div", { class: "connect-steps xmodem-steps", hidden: true },
          el("p", { class: "xmodem-keys" }, el("code", { class: "xmodem-command" })),
          el("ul", { class: "xmodem-notes" }),
        ),
        el("p", { class: "xmodem-status muted", role: "status" }),
        el("progress", { class: "xmodem-bar", hidden: true }),
        el("div", { class: "xmodem-restart", hidden: true },
          el("p", { class: "xmodem-restart-text" }),
          el("p", { class: "keys xmodem-restart-keys", "aria-hidden": "true" }),
        ),
        el("div", { class: "tx-dialog-actions" },
          el("button", { type: "button", "data-act": "cancel", text: "Cancel" }),
          el("button", { type: "button", "data-act": "close", text: "Close" }),
          el("button", { type: "button", "data-act": "later", text: "Later" }),
          el("button", { type: "button", class: "primary", "data-act": "type", text: "Type it for me" }),
          el("button", { type: "button", class: "primary", "data-act": "reconnect", text: "Reconnect" }),
        ),
      ),
    );
    this.ui = {
      title: q("xmodem-title"), lead: q("xmodem-lead"), steps: q("xmodem-steps"), command: q("xmodem-command"),
      notes: q("xmodem-notes"), status: q("xmodem-status"), bar: q("xmodem-bar"), restart: q("xmodem-restart"),
      restartText: q("xmodem-restart-text"), restartKeys: q("xmodem-restart-keys"),
    };
    this.dialog.addEventListener("click", (e) => {
      const act = e.target.closest?.("[data-act]")?.dataset.act;
      if (!act) return;
      if (act === "cancel") this.hooks.cancel?.();
      else if (act === "type") {
        e.target.closest("button").disabled = true;
        this.hooks.type?.();
      } else {
        this.answer = act;
        this.close();
      }
    });
    // Escape: Cancel while it runs (the dialog stays until the transfer stops), Close after.
    this.dialog.addEventListener("cancel", (e) => {
      e.preventDefault();
      if (this.state === "running") this.hooks.cancel?.();
      else this.close();
    });
    // Inside Files' root, where its styles are scoped (`:where(#files-view)`).
    (this.doc.getElementById("files-view") ?? this.doc.body).append(this.dialog);
  }

  buttons(shown) {
    for (const b of this.dialog.querySelectorAll("[data-act]")) {
      b.hidden = !shown.includes(b.dataset.act);
      b.disabled = false;
    }
  }

  /** Show the dialog for a transfer: `words` from `fastWords` (no plan yet); `hooks` as above; `emulated` offers "Type it for me" once the plan is in. */
  start(words, hooks = {}, { emulated = false } = {}) {
    this.build();
    this.hooks = hooks;
    this.emulated = emulated;
    this.state = "running";
    this.ui.title.textContent = words.title;
    this.ui.lead.textContent = words.lead;
    this.ui.steps.hidden = true;
    this.ui.status.textContent = words.status;
    this.ui.bar.hidden = true;
    this.ui.restart.hidden = true;
    this.buttons(["cancel"]);
    if (!this.dialog.open) this.dialog.showModal();
    this.dialog.querySelector('[data-act="cancel"]').focus();
  }

  /** The server has ended: what to type (`fastWords` with the plan). */
  plan(words) {
    if (this.state !== "running") return;
    this.ui.lead.textContent = words.lead;
    this.ui.command.textContent = words.keys;
    this.ui.notes.replaceChildren(...words.notes.map((n) => el("li", { text: n })));
    this.ui.steps.hidden = false;
    this.ui.status.textContent = words.status;
    this.buttons(this.emulated ? ["cancel", "type"] : ["cancel"]);
  }

  /** Bytes moving: the status line and the bar. */
  progress(text, done = 0, total = null) {
    if (this.state !== "running") return;
    this.ui.status.textContent = text;
    this.buttons(["cancel"]);
    if (total) {
      this.ui.bar.hidden = false;
      this.ui.bar.max = total;
      this.ui.bar.value = Math.min(done, total);
    }
  }

  /** It is over: `words` from `fastDone`. Resolves when the dialog closes, to the button pressed: "reconnect", "later", "close" (Escape: "close"). */
  finish(words) {
    this.build();
    this.state = "done";
    this.answer = "close";
    this.ui.lead.textContent = words.lead;
    this.ui.lead.classList.toggle("error", !words.ok);
    this.ui.steps.hidden = true;
    this.ui.bar.hidden = true;
    this.ui.status.textContent = "";
    this.ui.restart.hidden = !words.restart;
    this.ui.restartText.textContent = words.restart ?? "";
    this.ui.restartKeys.replaceChildren(...(words.keys ?? []).map((k) => el("kbd", { class: "calc-key", text: k })));
    this.buttons(words.restart ? ["later", "reconnect"] : ["close"]);
    if (!this.dialog.open) this.dialog.showModal();
    this.dialog.querySelector(words.restart ? '[data-act="reconnect"]' : '[data-act="close"]').focus();
    return new Promise((resolve) => this.dialog.addEventListener("close", () => resolve(this.answer), { once: true }));
  }

  close() {
    this.state = null;
    this.ui?.lead.classList.remove("error");
    if (this.dialog?.open) this.dialog.close();
  }
}
