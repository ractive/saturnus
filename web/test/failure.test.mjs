// The page's start failure (web/failure.js). `node --test web/test/`
// (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { startFailure } from "../failure.js";

test("a start failure says what to do: reload in a browser, restart in the app; one full stop", () => {
  assert.equal(startFailure(new Error("no wasm"), false),
    "saturnus could not start: no wasm. Reload the page; if it keeps failing, your browser may be too old for WebAssembly.");
  assert.equal(startFailure(new Error("the worker stopped."), true), "saturnus could not start: the worker stopped. Restart the app.");
  assert.equal(startFailure("plain text", true), "saturnus could not start: plain text. Restart the app.");
});
