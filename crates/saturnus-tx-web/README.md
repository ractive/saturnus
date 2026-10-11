# saturnus-tx-web

WebAssembly bindings of saturnus tx for the transfer page (`web/tx.html`,
`kb/docs/saturnus-tx.md`): `saturnus-tx`'s device over a serial link the
page holds, and the preview parts of `saturnus-objects` (binary transfer
files as typed objects, text files with their trigraphs as characters,
file names). `web/tx-build.sh` builds it with `wasm-pack` into
`web/tx/pkg/`. The page runs it in a dedicated Worker
(`web/tx/protocol-worker.js`), so a hidden tab's throttled timers do not
stretch the protocol's timeouts.

| Export | What |
|---|---|
| `new TxDevice(nowMs, seed, listenMs?)` | A device for one link; every call takes the page's clock in ms. |
| `start(nowMs, requestJson, data?)` | `connect`, `list`, `get`, `put` (the file in `data`), `disconnect`. |
| `input`, `timeout`, `linkError`, `abort` | Bytes read, the time, a failed link, drop the request. |
| `drain()` | `open` (reopen the port), `write`, events (`listening`, `found`, `progress`), the `result`. |
| `deadline()`, `isBusy()`, `connection()` | The timer to arm, whether a request runs, the calculator. |
| `file_kind`, `var_name`, `file_name`, `name_problem` | Files and names (`saturnus-tx`'s `names`). |
| `describe_file`, `text_of` | The preview. |

Not published (`publish = false`). MIT, see `AI_NOTICE` and `LICENSE` in
the repository.
