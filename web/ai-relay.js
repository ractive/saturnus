// Where AI access (web/ai.js) connects: tab-mcp's hosted relay, or on a
// page served from this computer a development relay. Small on purpose:
// web/main.js reads it on load and loads AI access only when there is a
// relay.

import { KEYS } from "./shared/storage-names.js";

/**
 * The hosted relay (ractive.ch's, since 2026-10-20; privacy notice at
 * https://ractive.ch/tab-mcp/). Null would turn AI access off on the
 * published site.
 */
export const HOSTED_RELAY = "https://tab-mcp.ractive.ch";

const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]"]);

/**
 * The relay AI access uses, or null for none: on a page served from this
 * computer (localhost) only a development relay named in localStorage
 * (`saturnus.aiRelay`, e.g. `http://127.0.0.1:8787`), never the hosted one,
 * so development pages put no traffic on it; elsewhere the hosted one.
 */
export function relayUrl({ location = globalThis.location, storage = globalThis.localStorage } = {}) {
  if (LOOPBACK.has(location?.hostname)) {
    try {
      const dev = storage?.getItem(KEYS.aiRelay);
      if (dev) return dev;
    } catch {
      // Storage blocked: no development relay.
    }
    return null;
  }
  return HOSTED_RELAY;
}

