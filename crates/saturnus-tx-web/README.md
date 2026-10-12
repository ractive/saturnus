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
| `start(nowMs, requestJson, data?)` | `connect`, `list`, `get`, `put` (the file in `data`), `disconnect`, `mkdir`, `rename`, `purge` (`count` for a directory), `get-tree`, `put-tree` (`data`: `[{path, data?}]`, file-system names), `memory`, `backup`, `restore` (the backup in `data`), `pict`. |
| `input`, `timeout`, `linkError`, `abort` | Bytes read, the time, a failed link, drop the request. |
| `drain()` | `open` (reopen the port), `write`, events (`listening`, `found`, `progress`, `item`), the `result` (a tree's files as each item's `data`). |
| `deadline()`, `isBusy()`, `connection()`, `restarted()` | The timer to arm, whether a request runs, the calculator, whether a restore restarted it. |
| `copy_plan`, `header_check` | Copying between devices: the steps, `inUse` (replace asks), `blocked` (a directory on either side), `untranslatable` (no text form), the `warning`. |
| `file_kind`, `var_name`, `file_name`, `folder_name`, `dir_name`, `backup_file_name`, `name_problem` | Files and names (`saturnus-tx`'s `names`). |
| `describe_file`, `text_of` | The preview. |
| `file_model`, `FileNames` | A file's model by its header's ROM letter; a 48SX's or 48GX's ROM's command names, whose `describe_file` gives programs and algebraics their text. |

Errors that ask the user first carry what the question needs: `nameClash`
(`name`), `namesInUse` (`names`), `notEmpty` (`count`, sent back with the
purge), `noRoom` (`needed`, `free`), `partial` (a tree put that stopped:
`directories`, `variables`, `rootMade`, the `source` error); `restarted`
after a restore.
`web/test/tx-bindings.test.mjs` drives the bindings from Node over the
recorded ops traces.

Not published (`publish = false`). MIT, see `AI_NOTICE` and `LICENSE` in
the repository.
