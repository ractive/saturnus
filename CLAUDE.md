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
- Before committing or a PR, in order: `cargo fmt`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace -q`,
  `cargo deny check` (`just gates` runs all CI checks).
- No `.unwrap()`/`.expect()` outside tests; `anyhow::Context` with `?`.
- Test policy in `kb/docs/test-policy.md`: instruction unit tests, one e2e
  binary gated by `SATURNUS_ROM_DIR`, differential script against saturnng.

# PR discipline
One iteration = one branch (`iter-N/short-description`) = one PR. Self-review
the diff. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` (or the model that
wrote it). Never commit ROMs, state files or secrets.
