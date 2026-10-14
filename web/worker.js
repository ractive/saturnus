// saturnus's plain Web Worker: the calculators' hub (hub.js) with one page,
// where the browser has no SharedWorker (Samsung Internet) and in the
// tests. The page is its only connection; the protocol is the same
// (web/protocol.md).

import { Hub } from "./hub.js";

// Web Locks for the model's auto-save lock (hub.js `lockAutoSave`): two tabs
// that each run their own Worker do not both write a model's auto slot.
const hub = new Hub({ locks: self.navigator?.locks ?? null });
const page = hub.connect(self);
self.onmessage = (e) => hub.receive(page, e.data ?? {});
