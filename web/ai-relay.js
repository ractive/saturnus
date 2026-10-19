// Where AI access (web/ai.js) connects: tab-mcp's hosted relay, or on a
// page served from this computer a development relay. Small on purpose:
// web/main.js reads it on load and loads AI access only when there is a
// relay.

import { KEYS } from "./shared/storage-names.js";

/**
 * The hosted relay, once tab-mcp's runs (its iteration 6:
 * https://relay.ractive.ch, already in the CSP's connect-src). Null: AI
 * access is off on the published site.
 */
export const HOSTED_RELAY = null;

const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]"]);

/**
 * The relay AI access uses, or null for none: on a page served from this
 * computer (localhost) a development relay named in localStorage
 * (`saturnus.aiRelay`, e.g. `http://127.0.0.1:8787`), else the hosted one.
 */
export function relayUrl({ location = globalThis.location, storage = globalThis.localStorage } = {}) {
  if (LOOPBACK.has(location?.hostname)) {
    try {
      const dev = storage?.getItem(KEYS.aiRelay);
      if (dev) return dev;
    } catch {
      // Storage blocked: no development relay.
    }
  }
  return HOSTED_RELAY;
}

