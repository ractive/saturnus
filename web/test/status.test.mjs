// The status line's message (web/status.js): an outcome goes after a
// while, a step in progress and an error stay; an error goes with the
// user's next action, not the one that caused it. `node --test web/test/`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { Store } from "../store.js";
import { ERROR_GRACE_MS, MESSAGE_MS, clearsItself, watchMessages } from "../status.js";

/** A store with the message clearing on a hand-moved clock. */
function setup() {
  const store = new Store();
  let t = 0;
  const timers = new Map();
  let next = 1;
  const listeners = {};
  watchMessages(store, {
    now: () => t,
    setTimer: (f, ms) => { timers.set(next, [t + ms, f]); return next++; },
    clearTimer: (id) => timers.delete(id),
    on: (type, fn) => { listeners[type] = fn; },
  });
  const advance = (ms) => {
    t += ms;
    for (const [id, [at, f]] of [...timers]) {
      if (at <= t) {
        timers.delete(id);
        f();
      }
    }
  };
  return { store, advance, act: (type = "pointerdown") => listeners[type]() };
}

test("which messages clear by themselves", () => {
  assert.equal(clearsItself("State saved in this browser.", false), true);
  assert.equal(clearsItself("Downloading the HP 48GX ROM from hpcalc.org…", false), false, "in progress");
  assert.equal(clearsItself("Could not save the state: quota", true), false, "an error");
  assert.equal(clearsItself("", false), false);
});

test("an outcome goes after a while; a newer message keeps its own time", () => {
  const { store, advance } = setup();
  store.set({ message: "State saved in this browser.", messageError: false });
  advance(MESSAGE_MS - 1);
  assert.equal(store.state.message, "State saved in this browser.");
  store.set({ message: "Screen copied as an image." });
  advance(10);
  assert.equal(store.state.message, "Screen copied as an image.", "the first one's timer is gone");
  advance(MESSAGE_MS);
  assert.equal(store.state.message, "");
});

test("a step in progress stays; an error stays until the next action", () => {
  const { store, advance, act } = setup();
  store.set({ message: "Downloading the HP 48GX ROM from hpcalc.org…", messageError: false });
  advance(10 * MESSAGE_MS);
  assert.match(store.state.message, /Downloading/);
  store.set({ message: "Could not save the state: quota", messageError: true });
  advance(10 * MESSAGE_MS);
  assert.match(store.state.message, /Could not save/);
  advance(0);
  act("keydown");
  assert.equal(store.state.message, "", "the next key clears it");
  store.set({ message: "Could not reset: halted", messageError: true });
  act("pointerdown");
  assert.match(store.state.message, /Could not reset/, "not the click that caused it");
  advance(ERROR_GRACE_MS);
  act("pointerdown");
  assert.equal(store.state.message, "");
  assert.equal(store.state.messageError, false);
});
