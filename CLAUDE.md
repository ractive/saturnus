# saturnus

A headless, clean-room emulator of the HP Saturn calculators (HP48 SX/GX,
HP49G, later 38G/39G/40G) in Rust. MIT, see `AI_NOTICE`.

# Agents
Delegate implementation work to Opus agents (`model: opus`) whenever possible;
brief them with the iteration file, the kb docs to read and the acceptance
criteria. The main session reviews.

# Documentation
All project knowledge lives in `./kb/` as markdown with YAML frontmatter.
Read first: `kb/docs/clean-room-rule.md` (non-negotiable),
`kb/docs/architecture.md`, `kb/decision-log.md`,
`kb/docs/knowledge-sources.md`, `kb/docs/test-policy.md`,
`kb/docs/open-hardware-questions.md`.
- Iteration plans: `kb/iterations/iteration-N-slug.md`, tasks as checkboxes,
  status `planned` -> `in-progress` -> `completed`.
- Decisions: `kb/decision-log.md`. Research: `kb/research/`. Backlog: `kb/backlog/`.

Always use `hyalo` for kb interactions, never Read/Grep/Edit on kb files
except for body prose: `hyalo summary`, `hyalo find`, `hyalo read <path>`,
`hyalo set`, `hyalo task toggle`, `hyalo lint`. `.hyalo.toml` sets `dir = "kb"`;
do not pass `--dir`. Follow the hints hyalo prints. `hyalo lint` must be clean
before a PR. Hardware facts live in the public calculator wiki
calculator-knowledgebase (<https://github.com/ractive/calculator-knowledgebase>,
checked out at `~/devel/calculator-knowledgebase/`, also hyalo-driven, its
own clean-room rules in its CLAUDE.md); cite them in code as
`wiki: hardware/timers` and write new findings back there, as commits or
PRs to that repository.

# Rust
- Edition 2024, stable. Windows, Linux, macOS; the core crate must build for
  `wasm32` (no I/O, no threads inside it).
- While working, run only the tests a change touches. Once before a PR, in
  order: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace -q`, `cargo deny check` (`just gates` runs all CI
  checks; `just web-test-full DIR` the web e2e). After review fixes, only the
  affected tests; see `kb/docs/test-policy.md`.
- No `.unwrap()`/`.expect()` outside tests; `anyhow::Context` with `?`.
- Test policy in `kb/docs/test-policy.md`: instruction unit tests, one e2e
  binary gated by `SATURNUS_ROM_DIR`, differential script against saturnng.

# satx
The transfer tool (Kermit and XModem to real calculators):
`crates/satx-core` (library), `crates/satx-cli` (the `satx` binary), and
the sans-IO protocol crates `crates/kermit-proto` and `crates/xmodem-proto`,
which must build for `wasm32`. Its kb is `kb/satx/`; its backlog items are
`kb/backlog/satx-*`.
- The oracle is saturnng in Docker (`tools/saturnng-oracle/`, the black box
  `kb/docs/clean-room-rule.md` allows) with the serial port on TCP. Ports
  4848-4852 belong to its containers (`just satx-oracle-up`); our own
  serial bridges use 4860-4869.
- `SATX_E2E_ADDR` gates satx-core's e2e suite: `tcp://localhost:4848` for
  the oracle, `saturnus://MODEL@ROM` for saturnus in process
  (`just satx-e2e-saturnus 49g` takes the ROMs from `SATURNUS_ROM_DIR`,
  else `roms/`). `scripts/satx-e2e-cli.sh` is the CLI's script.

# PR discipline
One iteration = one branch (`iter-N/short-description`) = one PR. Self-review
the diff. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` (or the model that
wrote it). Never commit ROMs, state files or secrets. Every commit's author
email is james@ractive.ch (CI's `commit-email` job; only dependabot is
exempt): check `git config user.email` in a new worktree.
