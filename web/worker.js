// saturnus's Worker: the calculators' hub (hub.js) for the one document
// (iteration 50d). Each view of the document is one connection: the
// document sends `{connect: true}` with a MessagePort it transferred
// (web/shared/calc-hub.js `hubPort`), and the hub speaks the protocol
// of web/protocol.md over it. Any other message is from the Worker's
// own page, one connection of its own (the tests, the protocol script).

import { Hub } from "./hub.js";

// Web Locks for the model's auto-save lock (hub.js `lockAutoSave`): two tabs
// that each run their own Worker do not both write a model's auto slot.
const hub = new Hub({ locks: self.navigator?.locks ?? null });
let page = null;
self.onmessage = (e) => {
  if (e.data?.connect === true && e.ports?.[0]) {
    const port = e.ports[0];
    const conn = hub.connect(port);
    port.onmessage = (m) => hub.receive(conn, m.data ?? {});
    return;
  }
  page ??= hub.connect(self);
  hub.receive(page, e.data ?? {});
};
