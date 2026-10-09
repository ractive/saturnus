---
title: Releasing
type: docs
date: 2026-10-05
status: active
tags:
  - infrastructure
---

# Releasing

`.github/workflows/release.yml` calls the shared pipeline
`ractive/release-workflows/.github/workflows/release.yml@v0.2.3`. Publishing
a GitHub release `vX.Y.Z` runs it: the tag must match `saturnus-cli`'s
version, then `cargo audit` and `cargo deny check`, a build and test
matrix (Linux gnu/musl x86_64 and aarch64, macOS aarch64, Windows x86_64
and aarch64; the emulated aarch64 Linux targets and Windows on ARM build
only), archives with the `saturnus` binary, LICENSE and README, SBOMs and
build provenance, a `.deb` and an `.rpm` of the CLI (x86_64, see "Linux
packages"), and the upload to the release; after the upload, the
crates go to crates.io and the Homebrew formula (`ractive/homebrew-tap`)
and Scoop manifest (`ractive/scoop-bucket`) are written. Secrets, by name:
`CARGO_TOKEN`, `HOMEBREW_TAP_TOKEN`, `SCOOP_BUCKET_TOKEN`.

A manual run (`workflow_dispatch`, Actions tab or
`gh workflow run release.yml`) is a dry run: it builds, tests and packages,
and uploads the archives and the deb/rpm as workflow artifacts only (no
crates.io, no Homebrew or Scoop). A dispatch needs the workflow on the default branch,
so a change to `release.yml` is dry-run on `main` after its merge.

Deliberately off, although some of their secrets exist: winget (the
shared workflow's README: `winget-releaser` only updates a package that
already exists, the first submission is a manual PR to
`microsoft/winget-pkgs`), Cloudsmith (the repository first, see "Linux
packages"), AUR (an account and an SSH key). The shared workflow ships
one binary, `saturnus`.

A dry run needs no secrets; the caller grants `contents: write`,
`id-token: write` and `attestations: write`, which the shared workflow's
`build` job inherits for the provenance attestation.

To enable winget, AUR or Cloudsmith later, set the matching input
(`winget-identifier`, `aur-package`, `cloudsmith-repo`) and its secret,
as described in the shared workflow's README.

## Linux packages

`enable-linux-packages: true` (with `linux-package-crate: saturnus-cli`)
makes the shared workflow's `linux-packages` job build the workspace on
ubuntu-latest (x86_64 gnu; `cargo build --release --locked`, which leaves
out `saturnus-tauri` through `default-members`), then run `cargo deb -p
saturnus-cli --no-build --no-strip` and `cargo generate-rpm -p
crates/saturnus-cli`. They read `[package.metadata.deb]` and
`[package.metadata.generate-rpm]` in `crates/saturnus-cli/Cargo.toml`
and become the release assets `saturnus-vV-x86_64-linux.deb` and
`saturnus-vV-x86_64-linux.rpm` (also in `SHA256SUMS`). x86_64 only; the
aarch64 Linux users take the archives.

The binary in the packages is the static `x86_64-unknown-linux-musl`
build, so they depend on no glibc. The job itself builds the glibc
binary on `ubuntu-latest` (Ubuntu 24.04), and packaged as it was that
needed glibc 2.39: the first dry run (37911820913, on 27b22ae) had
`Depends: libc6 (>= 2.39)` in the .deb and `libc.so.6(GLIBC_2.39)` among
the .rpm's requirements, so Ubuntu 24.04+, Debian 13+, Fedora 40+ and
RHEL 10 only. v0.2.3 has no input for the package's target, so
`release.yml`'s `pre-package-command` builds the musl binary in the
`linux-packages` job and copies it over `target/release/saturnus` before
cargo-deb and cargo-generate-rpm run. It recognises that job by its id,
`GITHUB_JOB` `linux-packages`, and by `BIN_PATH`, `target/release/saturnus`;
the build matrix's job is `build`, its `BIN_PATH`
`target/<target>/release/saturnus`, and there the command does nothing.
If the two signs disagree it fails, and so it does when the copied binary
is not static (`file` must say so), rather than package the glibc binary.
**On every bump of the `release-workflows` pin**, re-read both jobs in
its `release.yml` (the job ids, the `BIN_PATH` each exports to
`pre-package-command`, and whether the packages are still built from
`target/release/`), and dry-run: the .deb's binary must be static. The CLI's dependencies
are pure Rust, so the musl target links with the Rust toolchain's own
musl and needs no `musl-tools`. Check on a dry run: the .deb's control
(`ar x`, then `tar -xOf control.tar.* ./control`) has no `libc6` in
`Depends`, and `file usr/bin/saturnus` from its data says `static-pie
linked`. A later release-workflows input for the package's target would
replace the command (proposed to that repository, not done).

Contents: `/usr/bin/saturnus` and `/usr/share/doc/saturnus-cli/` with
the root `README.md` and `LICENSE` (the .deb adds a generated
`copyright`). The package name is **`saturnus-cli`**: the desktop app's
.deb and .rpm (`saturnus_V_amd64.deb`, `saturnus-V-1.x86_64.rpm`, from
`desktop.yml`) are the package `saturnus`, and two packages of one name
replace each other. The two can be installed side by side: no file is
in both (checked against the 0.1.0 app packages: the app installs
`/usr/bin/saturnus-app`, `/usr/share/applications/saturnus.desktop` and
`/usr/share/icons/hicolor/*/apps/saturnus-app.png`; the CLI
`/usr/bin/saturnus` and `/usr/share/doc/saturnus-cli/`), and dpkg and rpm
refuse two packages that own the same path. The doc sources are written `../../README.md`: cargo-deb
resolves a source in the crate directory, cargo-generate-rpm in the
working directory first, so a bare `README.md` would be a different file
in each package. No shell completions yet: the CLI has no `completions`
subcommand; adding one (clap_complete) and a `pre-package-command` that
writes them, as hptx does, is a follow-up.

Locally (`cargo install cargo-deb cargo-generate-rpm` once):
`cargo build --release -p saturnus-cli`, then the two commands above;
the packages land in `target/debian/` and `target/generate-rpm/`
(the host's architecture). `ar x` the .deb and `tar -tvf data.tar.xz`
lists it; `bsdtar -tvf` lists an .rpm on macOS.

**Cloudsmith** (hosted apt and dnf repositories, a one-time bootstrap,
not done yet): the owner creates the repository `ractive/saturnus` on
cloudsmith.io, then `release.yml` gets `cloudsmith-repo: ractive/saturnus`
(`CLOUDSMITH_API_KEY` is already a repository secret). The shared
workflow's `cloudsmith` job then pushes each release's .deb and .rpm,
non-blocking. Releases made before that are pushed by hand
(`cloudsmith push deb ractive/saturnus/any-distro/any-version <file>`,
and `push rpm` likewise).

`release-workflows` stays at v0.2.3: v0.2.2's post-release jobs were
skipped whenever `linux-packages` was (decision log 2026-10-09). With
deb/rpm on, that job runs and the fix is no longer needed for this
caller, but it is when deb/rpm are turned off again.

## crates.io

Published, in this order (each needs the ones before it on the index):
`saturnus` (the core, no dependencies), `saturnus-objects`,
`saturnus-host` (the front ends' host-neutral code: emulator, key queue,
typing, skins, ROM identification), `saturnus-drive`, `saturnus-cli` (the
`saturnus` binary, so `cargo install saturnus-cli` works). The list is
`publish-crates` in `release.yml` and `publish-crates.yml`, and `just
package` uses the same crates; keep the three in sync. Everything else is
`publish = false`: `saturnus-web` (the wasm bindings, shipped as the web
page), `saturnus-tauri` (shipped as installers), `saturnus-kermit` (test
support) and `saturnus-refgen` (a tool).

One version for all of them, `workspace.package.version`; the internal
dependencies are `[workspace.dependencies]` with a path and that version,
so a version bump changes `workspace.package.version` **and** every
`version` in `[workspace.dependencies]`. A missed one shows in `just
package`: the packages then require the old version, which the
rehearsal's temporary registry does not hold.
The desktop app takes its version from its crate (no `version` in
`tauri.conf.json`).

Package contents (`include` in each manifest): sources, README and
LICENSE (a copy of the root `LICENSE`, not a link, which a checkout
without symlinks would package as a stub; `just package` and CI check
that the copies match); the core adds `examples/`, the CLI `build.rs` and the command
reference it embeds (`crates/saturnus-cli/data/commands/`). No `tests/`:
their goldens are screen dumps of HP's ROMs. Each published crate's
README is its crates.io page and compiles as a doctest
(`ReadmeDoctests` in its `lib.rs`).

The rehearsal, `just package`: `cargo package --locked` for the five
crates in one call. Cargo (1.90 and later) packages them in dependency
order and verifies each package by building it against the packages
before it, through a temporary local registry in
`target/rehearsal/package/tmp-registry` (CI: `target/package/`), so the
chain is checked without crates.io. `cargo publish --dry-run` cannot do that for a crate whose
dependencies are not on crates.io yet. Cargo treats that registry's
packages like crates.io's, as immutable per version: it reuses their
unpacked sources (`$CARGO_HOME/registry/src/-<hash>/`) and their build
artifacts, so without care a second local run verifies against the first
run's code. `just package` removes both first and builds in its own
target directory, `target/rehearsal` (a fresh CI runner has neither). `just doc` builds the docs as
docs.rs does, with warnings denied.

## Release checklist

From "secrets present" to "published", in order, on the owner's machine
in a clean clone of `main`. `V` is the version without the `v`; for the
first release `V=0.1.0`. Every step says what to check before the next.

1. **Preconditions.**
   `gh secret list --repo ractive/saturnus` lists `CARGO_TOKEN`,
   `HOMEBREW_TAP_TOKEN`, `SCOOP_BUCKET_TOKEN` and `FTP_PASSWORD` (the
   web page's upload to ractive.ch, step 9; names only, never values).
   First release only: each of `saturnus saturnus-objects
   saturnus-host saturnus-drive saturnus-cli` is still free:
   `curl -s -o /dev/null -w '%{http_code}\n' -A saturnus-release-check https://crates.io/api/v1/crates/NAME`
   prints `404` (later releases:
   `cargo owner --list NAME` names the owner).
2. **Version and notes on `main`** (a normal PR if anything changes):
   `workspace.package.version` in `Cargo.toml` is `V`, and so is every
   `version` in its `[workspace.dependencies]` (step 3's `just package`
   fails on a mismatch); `CHANGELOG.md` has
   `## V (YYYY-MM-DD)` with the release date instead of `(unreleased)`;
   `.github/release-notes/vV.md` exists. Then
   `git switch main && git pull --ff-only && git status --short` prints
   nothing.
3. **Local gates**: `just gates` (includes `just package` and `just doc`)
   passes.
4. **ROM-gated suites**, with the owner's ROMs:
   `just rom-tests ~/devel/saturnus/roms` (every crate's tests with the
   ROMs, `saturnus-tauri`'s `tests/runner.rs` included, and the full
   reference regeneration, minutes). All pass; `git status --short` still
   prints nothing.
5. **Pipeline dry run**: `gh workflow run release.yml --ref main`, then
   `gh run watch $(gh run list --workflow release.yml -L 1 --json databaseId -q '.[0].databaseId') --exit-status`.
   Check: `gh run download <id> -n dry-run-bundle -D /tmp/saturnus-dry`
   holds seven `saturnus-vV-<target>` archives,
   `saturnus-vV-x86_64-linux.deb`, `saturnus-vV-x86_64-linux.rpm` and
   `SHA256SUMS`.
6. **Desktop dry run**: `gh workflow run desktop.yml --ref main`, watch it
   the same way; `gh run download <id> -D /tmp/saturnus-app` holds the
   `.dmg`, `.msi`, `-setup.exe`, `.deb`, `.rpm` and `.AppImage`, each named
   with `V`.
7. **Release** (this publishes: crate versions on crates.io are
   permanent):
   `gh release create vV --target main --title "saturnus V" --notes-file .github/release-notes/vV.md`.
   This starts `release.yml`. Watch it as in step 5. Check:
   `gh release view vV --json assets -q '.assets[].name'` lists the seven
   archives, `saturnus-vV-x86_64-linux.deb` and `.rpm`, `SHA256SUMS`,
   SBOMs; each crate answers `200` at
   `https://crates.io/api/v1/crates/NAME/V`;
   `cargo install saturnus-cli --locked --root /tmp/saturnus-install && /tmp/saturnus-install/bin/saturnus --version`
   prints `saturnus V`; `gh api repos/ractive/homebrew-tap/contents/Formula/saturnus.rb -q .name`
   and `gh api repos/ractive/scoop-bucket/contents/bucket/saturnus.json -q .name`
   answer.
8. **Installers on the release**:
   `gh workflow run desktop.yml --ref main -f release-tag=vV`; watch it;
   `gh release view vV` now also lists the six installers.
9. **Web page**: `gh workflow run pages.yml --ref vV`; watch it as in
   step 5 (both jobs pass: `build`, then `ractive`, the FTPS upload to
   ractive.ch; there is no GitHub Pages deployment any more); open
   <https://ractive.ch/saturnus/>, load a ROM
   from a local file, and check the About panel: its first line reads
   `saturnus V, build <id>`, with V this release and `<id>` the 16 hex
   digits `web/site.sh` printed in the `build` job's log ("web/site.sh:
   build <id>, N files precached"). A different build is an old service
   worker: reload, or the update notice's Reload. The desktop app's
   About reads `saturnus V, desktop app`.
10. **docs.rs**: <https://docs.rs/crate/saturnus/V>,
    `.../saturnus-objects/V`, `.../saturnus-host/V`,
    `.../saturnus-drive/V` show a successful build (minutes after step 7).

Recovery:

- `release.yml` fails **before** the upload (version check, audit,
  build): nothing is published. `gh release delete vV --cleanup-tag --yes`,
  fix on `main`, start again at step 2.
- The **`crates-io` job** fails after the upload. For a transient
  cause (index lag beyond its retries, network):
  `gh workflow run publish-crates.yml -f ref=vV`, always the release's
  tag (the input is required), so crates.io gets exactly what the release
  is; it skips the crates already published. For a cause in the code (a
  package that does not build): fix it on `main` and release `V+1`;
  crates already published as `V` stay. A published version cannot be
  replaced: a broken one is yanked (`cargo yank --version V <crate>`).
- **Homebrew or Scoop** fails (token, network):
  `gh run rerun <id> --failed`.
- **`desktop.yml` with the tag** fails while attaching: it never
  overwrites, so delete what it attached
  (`gh release delete-asset vV <file> --yes`) and dispatch it again.

## Web page: ractive.ch

`.github/workflows/pages.yml` (manual dispatch) builds the wasm package,
assembles the site with `web/site.sh` (every page file in `web/`, the
components, the package, and `web/site.htaccess` as `.htaccess` for
Apache hosts), and publishes it by FTP to the owner's site
(<https://ractive.ch/saturnus/>, `httpdocs/saturnus/` on ractive.ch; the
`FTP_PASSWORD` repository secret, the same the site's own deploy uses).
The first copy on ractive.ch was committed into that site's repository
by hand (ractive.ch PR 8); from then on this workflow keeps it current.
GitHub Pages (enabled 2026-10-06) was disabled and its site deleted on
2026-10-08 at the owner's request: ractive.ch is the only web address.
Dispatch it after every merge to `main` that changes `web/` or the
bindings. The FTP step uses explicit TLS (`protocol: ftps`), adds and
overwrites only, and leaves its sync-state file in the directory: a file
renamed or removed in `web/` stays on ractive.ch until removed by hand
(or a run with the action's clean-slate option). The ractive.ch site's own
repository must not carry a copy of the directory any more, so that only
this workflow writes there (its PR 8 copy is to be removed with an
`exclude` of `httpdocs/saturnus/**` in that site's deploy).

`web/site.sh` also checks that every relative module import and `new
URL(...)` of the assembled page names a file in it: before iteration 21
`pages.yml` copied a fixed file list without `romstore.js`, which the
Worker imports since iteration 20, and the next deploy would have broken
the page. `web/site.sh --list` prints the selected page files; the
desktop app embeds the same set (see "Desktop app"). Built in CI, the
wasm names only the runner's paths (`/home/runner/...`) in its panic
messages; a local build would name the builder's home directory, so the
page is deployed only from CI.

## Desktop app

`.github/workflows/desktop.yml` builds the Tauri app's installers with
`tauri-apps/tauri-action` on macOS (Apple silicon), Windows and Linux. It
runs only when dispatched by hand (Actions tab or `gh workflow run
desktop.yml`), needs no secrets (the Apple ones are optional, see below)
and uploads the installers as workflow
artifacts; given a `release-tag` it builds that tag (not the dispatched
branch) and attaches the installers to that existing release (the
release must exist first, for example from `release.yml`); assets
already on the release are never replaced, so a rerun needs them deleted
by hand first.
It is not triggered by publishing a release: the checklist dispatches
it with the tag after `release.yml` has made the release (step 8).

The installers are **unsigned** for 0.1.0 (the owner's decision); the
release notes and the README say so and how to open them. The macOS app
is only ad-hoc signed by the linker, so a downloaded copy is reported as
"damaged" and needs `xattr -dr com.apple.quarantine` (a proper ad-hoc
signature, Tauri's `bundle.macOS.signingIdentity: "-"`, would turn that
into the "Open Anyway" path; not done). The app embeds the page's files only
(iteration 21; before, `frontendDist` was all of `web/`, with its README,
`protocol.md`, the build scripts and the tests): `build.rs` copies them
with `web/site.sh`'s rule (top-level HTML, CSS, JS, JSON, SVG and
`components/*.js`; no wasm package, no `.htaccess`) into
`$OUT_DIR/app-site` and hands that to Tauri's code generation through
`TAURI_CONFIG`, merged into what the Tauri CLI may have set (the dev
server's URL in `cargo tauri dev`). `frontendDist` in `tauri.conf.json`
stays `../../web`: the CLI checks that it exists before building, and
`cargo tauri dev` serves it. The test `tests/frontend.rs` compares the
embedded set with `web/site.sh --list`. What the owner has to provide for
signed installers:

- macOS: wired, waiting for the secrets. `desktop.yml` signs and
  notarises when the repository has the six Apple secrets that
  release-workflows' `docs/macos-signing.md` describes and its
  `scripts/set-apple-secrets.sh` sets (`APPLE_CERTIFICATE`,
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_API_KEY_ID`, `APPLE_API_ISSUER`, `APPLE_API_KEY`: notarisation
  with an App Store Connect API key, not an Apple ID password). Without
  `APPLE_CERTIFICATE` it builds unsigned exactly as before. With it, a
  second tauri-action step gets the secrets under Tauri's names (Tauri's
  `APPLE_API_KEY` is the key ID, and the .p8 is written to a temporary
  file passed as `APPLE_API_KEY_PATH`; Apple's "Developer ID - G2"
intermediate, which the .p12 lacks and Tauri does not import, goes into
a keychain of its own on the search list, checked against a pinned
SHA-256). Tauri signs with the hardened
  runtime, notarises and staples the `.app` and signs the `.dmg`; the
  workflow then notarises and staples the `.dmg` too and checks both with
  `codesign`, `stapler validate` and `spctl`. The unsigned build must not
  see these names at all: the Tauri bundler takes a variable that is set
  but empty as given. The same secrets sign the CLI's macOS binary once
  `release.yml` pins the release-workflows tag that brings signing.
  Unsigned, Gatekeeper refuses the `.dmg` until the user allows it in
  System Settings.
- Windows: a code-signing certificate (Tauri's `bundle.windows`
  `certificateThumbprint` or a `signCommand`), else SmartScreen warns.
- Linux: nothing; `.deb`, `.rpm` and the AppImage are unsigned as usual.
- The bundle identifier is `ch.ractive.saturnus`
  (`crates/saturnus-tauri/tauri.conf.json`); changing it later orphans
  the app's stored preferences, so decide it before the first release.
- The app's version is the workspace version (`tauri.conf.json` has
  none, so Tauri takes the crate's), the same as the release tag.

No updater is configured (it would need a signing key pair and an
endpoint).

### The Homebrew cask

`desktop.yml`'s `cask` job keeps the cask `saturnus-app` in
`ractive/homebrew-tap` (`Casks/saturnus-app.rb`; not `saturnus`, which
is the CLI formula there). It runs after `attach`, so only on a dispatch
with a `release-tag`, and only with the Apple secrets; otherwise its steps
skip. A draft or pre-release leaves the cask alone (a notice says so). It
downloads the release's `saturnus_<version>_aarch64.dmg` and refuses to
go on unless the `.dmg` and the `saturnus.app` in it pass `stapler
validate` and `spctl`, the app is signed by a Developer ID Application
certificate, and its bundle id and version are `ch.ractive.saturnus` and
the tag's. Then it fills `@VERSION@` and `@SHA256@` in
`packaging/homebrew/saturnus-app.rb` (read at the tag) and commits the
result straight to the tap with `HOMEBREW_TAP_TOKEN`, as release-workflows
does for the formula. The first such release creates `Casks/`. Change the
cask in the template, not in the tap: the next release overwrites it.

The cask: `url` the release asset, `name "saturnus"`, the
`shortDescription` as `desc`, `homepage` <https://ractive.ch/saturnus/>,
`depends_on arch: :arm64` and a bare `depends_on :macos` (the app needs
macOS 11, Tauri's 10.13 raised to 11.0 for arm64, which is below
Homebrew's own minimum, so `brew style` rejects a version), `app
"saturnus.app"`, and a `zap` of the folders named after the bundle id
(Application Support, which holds both Tauri's data and config dirs,
Caches, WebKit, Saved Application State and the Preferences plist).
Checked by `brew style` and `brew audit --cask --strict --online` in a
throwaway local tap against the 0.1.0 `.dmg`; `--new` also fails there
because 0.1.0 is unsigned (expected) and on "not notable enough", which
only applies to `homebrew/cask`. The `zap` paths are the conventional
ones for the bundle id, not yet checked on an installed app.

## Pinning policy

Third-party actions are pinned to a full commit SHA with a `# vX.Y.Z`
comment; Dependabot proposes the updates. First-party reusable workflows
and actions are referenced by tag, not SHA: `ractive/release-workflows`
by exact tag (`@v0.2.3`), `ractive/setup-hyalo@v1` floating on its major
tag, as in hyalo. Both repositories belong to the owner, so a tag is a
reviewed release; Dependabot ignores them.
