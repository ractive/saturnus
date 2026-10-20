// Which relay AI access uses (web/ai-relay.js): the hosted one on the
// published site, on localhost only a development relay named in
// localStorage.
import { test } from "node:test";
import assert from "node:assert/strict";
import { HOSTED_RELAY, relayUrl } from "../ai-relay.js";
import { KEYS } from "../shared/storage-names.js";

const storage = (value) => ({ getItem: (key) => (key === KEYS.aiRelay ? value : null) });

test("the published site uses the hosted relay", () => {
  assert.equal(HOSTED_RELAY, "https://tab-mcp.ractive.ch");
  assert.equal(relayUrl({ location: { hostname: "ractive.ch" }, storage: storage(null) }), HOSTED_RELAY);
  // A development relay in storage counts only on localhost.
  assert.equal(relayUrl({ location: { hostname: "ractive.ch" }, storage: storage("http://127.0.0.1:8787") }), HOSTED_RELAY);
});

test("localhost uses a development relay, never the hosted one", () => {
  for (const hostname of ["localhost", "127.0.0.1", "[::1]"]) {
    assert.equal(relayUrl({ location: { hostname }, storage: storage("http://127.0.0.1:8787") }), "http://127.0.0.1:8787");
    assert.equal(relayUrl({ location: { hostname }, storage: storage(null) }), null, hostname);
  }
  const blocked = { getItem: () => { throw new Error("blocked"); } };
  assert.equal(relayUrl({ location: { hostname: "localhost" }, storage: blocked }), null);
});
