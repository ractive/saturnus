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
`ractive/release-workflows/.github/workflows/release.yml@v0.3.1` twice, once
per command-line tool, because the shared pipeline packages one binary per
archive. One tag releases both:

- `release-saturnus`: `bin-name` and `version-package` `saturnus`,
  checksums in `saturnus-SHA256SUMS`, the .deb/.rpm of the package
  `saturnus` (with the musl swap, "Linux packages", as satx's), and the crates.io
  publish of all nine crates, satx's included ("crates.io");
- `release-satx`: `bin-name` and `version-package` `satx`, checksums in
  `satx-SHA256SUMS`, the shell completions in the archives and in the
  .deb/.rpm of the package `satx`, no crates (`publish-crates: ""`), and
  one target more, `x86_64-apple-darwin` on the Intel macOS runner.

Both version-packages take the workspace version, so the tag matches both.
v0.3.0 keeps the two apart where they would collide: each takes only its own
artifacts (`<bin-name>-v<version>-…`), uploads its own checksums file, has
its own dry-run bundle, pushes only its own packages to Cloudsmith, and the
Homebrew tap and Scoop bucket pushes rebase onto each other's and retry.

Publishing a GitHub release `vX.Y.Z` runs both: the tag must match the
crate's version, then `cargo audit` and `cargo deny check`, a build and test
matrix (Linux gnu/musl x86_64 and aarch64, macOS aarch64, Windows x86_64
and aarch64; the emulated aarch64 Linux targets and Windows on ARM build
only), archives with the binary, LICENSE and README (satx's also its
`completions/`), SBOMs and
build provenance (the macOS binary signed with a Developer ID
certificate and notarised before it is archived, see "macOS signing"),
a `.deb` and an `.rpm` of each CLI (x86_64, see "Linux
packages"), and the upload to the release; after the upload, the
crates go to crates.io and the Homebrew formulas `saturnus` and `satx`
(`ractive/homebrew-tap`) and Scoop manifests (`ractive/scoop-bucket`) are
written. The tests (`cargo test --workspace`) run in `release-saturnus` on
the native macOS and Windows targets, and in `release-satx` on the Intel
Mac only; the Linux targets build only, because the workspace's tests
take in `saturnus-tauri` and its webkit2gtk libraries, absent on the
runners (ci.yml tests everything else on every PR). Secrets, by name:
`CARGO_TOKEN`, `HOMEBREW_TAP_TOKEN`, `SCOOP_BUCKET_TOKEN`, and the six
`APPLE_*` of "macOS signing".

A manual run (`workflow_dispatch`, Actions tab or
`gh workflow run release.yml`) is a dry run: it builds, tests and packages,
and uploads the archives and the deb/rpm as workflow artifacts only (no
crates.io, no Homebrew or Scoop), one bundle per caller:
`dry-run-bundle-saturnus` and `dry-run-bundle-satx` (the artifact
`dry-run-bundle-<bin-name>`, `dry-run-bundle` before v0.3.0), each with
its own checksums file. A dispatch needs the workflow on the default
branch, but it runs the dispatched ref's version of it
(`gh workflow run release.yml --ref <branch>`), so a change to
`release.yml` can be dry-run on its branch.

Deliberately off, although some of their secrets exist: winget (the
shared workflow's README: `winget-releaser` only updates a package that
already exists, the first submission is a manual PR to
`microsoft/winget-pkgs`), Cloudsmith (the repository first, see "Linux
packages"), AUR (an account and an SSH key), on both callers.

A dry run needs no secrets; the caller grants `contents: write`,
`id-token: write` and `attestations: write`, which the shared workflow's
`build` job inherits for the provenance attestation.

To enable winget, AUR or Cloudsmith later, set the matching input
(`winget-identifier`, `aur-package`, `cloudsmith-repo`) and its secret,
as described in the shared workflow's README.

## macOS signing

Since v0.2.4 the shared workflow signs and notarises each macOS binary
when the repository has `APPLE_CERTIFICATE` (with
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
`APPLE_API_KEY_ID`, `APPLE_API_ISSUER` and `APPLE_API_KEY`; its
`docs/macos-signing.md`): hardened runtime and a secure timestamp, after
`pre-package-command` and before the archive, so the checksums file, the SBOM,
the attestation and the Homebrew checksum describe the signed binary.
`release.yml` passes them with `secrets: inherit`. A dry run stays
unsigned unless the caller sets `macos-sign-dry-run: true`, which this one
does not: a dispatch builds an unreviewed ref. Without the secrets nothing
changes. This covers both CLIs; the desktop installers are a separate
matter ("Desktop app").

## Linux packages

`enable-linux-packages: true` (with `linux-package-crate: saturnus`)
makes the shared workflow's `linux-packages` job build the workspace on
ubuntu-latest (x86_64 gnu; `cargo build --release --locked`, which leaves
out `saturnus-tauri` through `default-members`), then run `cargo deb -p
saturnus --no-build --no-strip` and `cargo generate-rpm -p
crates/saturnus-cli`. They read `[package.metadata.deb]` and
`[package.metadata.generate-rpm]` in `crates/saturnus-cli/Cargo.toml`
and become the release assets `saturnus-vV-x86_64-linux.deb` and
`saturnus-vV-x86_64-linux.rpm` (also in `saturnus-SHA256SUMS`). x86_64
only; the aarch64 Linux users take the archives. `release-satx` does the
same for `satx` (`linux-package-crate: satx`, `crates/satx-cli`):
`satx-vV-x86_64-linux.deb` and `.rpm` with `/usr/bin/satx`, its README and
LICENSE, and the bash, zsh and fish completions, which its
`pre-package-command` writes into `completions/` and
`crates/satx-cli/completions/` (cargo-deb resolves sources in the crate
directory, cargo-generate-rpm in the workspace root first). The same
command then does the musl swap below with `-p satx`, so satx's packages
hold the static binary too: its dependencies are pure Rust as well
(`serialport` without default features, so no libudev; checked with
`cargo build --target x86_64-unknown-linux-musl -p satx`, `static-pie
linked`).

The binary in the packages is the static `x86_64-unknown-linux-musl`
build, so they depend on no glibc. The job itself builds the glibc
binary on `ubuntu-latest` (Ubuntu 24.04), and packaged as it was that
needed glibc 2.39: the first dry run (37911820913, on 27b22ae) had
`Depends: libc6 (>= 2.39)` in the .deb and `libc.so.6(GLIBC_2.39)` among
the .rpm's requirements, so Ubuntu 24.04+, Debian 13+, Fedora 40+ and
RHEL 10 only. v0.3.1 has no input for the package's target, so
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

Contents: `/usr/bin/saturnus` and `/usr/share/doc/saturnus/` with
the root `README.md` and `LICENSE` (the .deb adds a generated
`copyright`). The package name is **`saturnus`**; the desktop app's
.deb and .rpm (from `desktop.yml`) are the package `saturnus-app`,
through the Linux-only `productName` in
`crates/saturnus-tauri/tauri.linux.conf.json`, because two packages of
one name replace each other. The same file gives the app's .desktop
entry a template of its own (`bundle.linux.deb.desktopTemplate` and
`rpm.desktopTemplate`, `crates/saturnus-tauri/linux/saturnus.desktop`,
Tauri's default with `Name=saturnus`), so the desktop menu says
"saturnus" while the package, the binary and the file are
`saturnus-app`; the AppImage takes the deb's entry. Checked on the 0.2.0
desktop run 38094261210 (b32e4e7, before the template): the .deb
(`Package: saturnus-app`) and the .rpm (`saturnus-app`) install
`/usr/bin/saturnus-app`, `/usr/share/applications/saturnus-app.desktop`
and `/usr/share/icons/hicolor/{32x32,128x128,256x256@2}/apps/saturnus-app.png`;
the CLI's `/usr/bin/saturnus` and `/usr/share/doc/saturnus/`, satx's
`/usr/bin/satx`, its completions and `/usr/share/doc/satx/`. The three
can be installed side by side: their file lists, compared pairwise as
.deb and as .rpm, share no file, and `/usr/bin/saturnus` is the CLI's
only (a file-list check, not an install); dpkg and rpm refuse two
packages that own the same path. The doc sources are written `../../README.md`: cargo-deb
resolves a source in the crate directory, cargo-generate-rpm in the
working directory first, so a bare `README.md` would be a different file
in each package. No shell completions yet: the CLI has no `completions`
subcommand; adding one (clap_complete) and a `pre-package-command` that
writes them, as satx does, is a follow-up.

Locally (`cargo install cargo-deb cargo-generate-rpm` once):
`cargo build --release -p saturnus`, then the two commands above;
the packages land in `target/debian/` and `target/generate-rpm/`
(the host's architecture). `ar x` the .deb and `tar -tvf data.tar.xz`
lists it; `bsdtar -tvf` lists an .rpm on macOS.

**Cloudsmith** (hosted apt and dnf repositories, a one-time bootstrap,
not done yet): the owner creates the repository `ractive/saturnus` on
cloudsmith.io, then both jobs in `release.yml` get `cloudsmith-repo:
ractive/saturnus` (`CLOUDSMITH_API_KEY` is already a repository secret).
Each caller's `cloudsmith` job then pushes its own .deb and .rpm,
non-blocking. Releases made before that are pushed by hand
(`cloudsmith push deb ractive/saturnus/any-distro/any-version <file>`,
and `push rpm` likewise).

`release-workflows` is at v0.3.1, which finds each SBOM by its package's
manifest directory (the package `saturnus` lives in `crates/saturnus-cli`,
`satx` in `crates/satx-cli`; v0.3.0 shipped no SBOM for them) and fails
the build when one is missing; v0.3.0 brought two callers in one run and
the 429 wait for new crates; v0.2.4 brought macOS signing, and v0.2.3 fixed v0.2.2's
post-release jobs, skipped whenever `linux-packages` was (decision log
2026-10-09). With deb/rpm on, that job runs and the fix is no longer
needed for this caller, but it is when deb/rpm are turned off again.

## crates.io

Nine crates are published, in this order (each needs the ones before it
on the index):

1. `saturnus-core` (the core, no dependencies);
2. `kermit-proto` (the Kermit protocol; `saturnus-host` uses it);
3. `saturnus-objects`;
4. `saturnus-host` (the front ends' host-neutral code: emulator, key queue,
   typing, skins, ROM identification);
5. `saturnus-drive`;
6. `saturnus` (the CLI in `crates/saturnus-cli`, so `cargo install
   saturnus` works);
7. `xmodem-proto` (the XModem protocol);
8. `satx-core` (satx's library);
9. `satx` (the transfer tool in `crates/satx-cli`, `cargo install satx`).

`release-saturnus` publishes all nine (`publish-crates`), and
`release-satx` none, so one job owns the order. The list is in
`release.yml` and as the default of `publish-crates.yml`'s `crates`
input, and `just package` uses the same crates in the same order; keep the
three in sync. crates.io lets an account create 5 new crates at once and
then one every 10 minutes; for a release with new names the shared
workflow waits 600 seconds on that 429 ("You have published too many new
crates") and tries again, at most 6 times per crate.

`publish-crates.yml` (dispatch only) publishes from a tag:
`gh workflow run publish-crates.yml -f ref=vV` publishes the whole list,
`-f crates=a,b` a subset in the order given. Both skip a crate already
on crates.io. Everything else is
`publish = false`: `saturnus-web` (the wasm bindings, shipped as the web
page), `saturnus-tauri` (shipped as installers), `saturnus-kermit` (test
support), `saturnus-refgen` (a tool), and `saturnus-tx` (Files' device
layer, used only in this repository; its wasm bindings are
saturnus-web's `tx` module).

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

The rehearsal, `just package`: `cargo package --locked` for the nine
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
   `HOMEBREW_TAP_TOKEN` and `SCOOP_BUCKET_TOKEN`, and
   `gh secret list --repo ractive/saturnus --env pages` lists
   `FTP_PASSWORD` (the web page's upload to ractive.ch, step 9; it lives
   only in the `pages` environment, not as a repository secret; names
   only, never values).
   For each crate of the publish list ("crates.io": `saturnus-core
   kermit-proto saturnus-objects saturnus-host saturnus-drive saturnus
   xmodem-proto satx-core satx`), `cargo owner --list NAME` names the
   owner; a name not on crates.io yet is free instead:
   `curl -s -o /dev/null -w '%{http_code}\n' -A saturnus-release-check https://crates.io/api/v1/crates/NAME`
   prints `404`.
2. **Version and notes on `main`** (a normal PR if anything changes):
   `workspace.package.version` in `Cargo.toml` is `V`, and so is every
   `version` in its `[workspace.dependencies]` (step 3's `just package`
   fails on a mismatch); `CHANGELOG.md` has
   `## V (YYYY-MM-DD)` with the release date instead of `(unreleased)`;
   `.github/release-notes/vV.md` exists. Then
   `git switch main && git pull --ff-only && git status --short` prints
   nothing.
3. **Local gates**: `just gates` (includes `just package` and `just doc`)
   passes. Then the links to other sites (needs the network, not in CI):
   `node scripts/check-links.mjs` (`--verbose` prints every address)
   checks the docs pages, the landing page, `web/about.json`, Search's
   manuals and the app's texts. It must print "no dead links"; fix a
   dead one at its source (the page, `scripts/about-json.py`, the
   reference). "Blocked, check by hand" lists the addresses of sites
   that turn scripts away (hpmuseum.org and columbia.edu behind
   Cloudflare, the Wayback Machine's rate limit): open them in a
   browser.
4. **ROM-gated suites**, with the owner's ROMs:
   `just rom-tests ~/devel/saturnus/roms` (every crate's tests with the
   ROMs, `saturnus-tauri`'s `tests/runner.rs` included, and the full
   reference regeneration, minutes). All pass; `git status --short` still
   prints nothing.
5. **Pipeline dry run**: `gh workflow run release.yml --ref main`, then
   `gh run watch $(gh run list --workflow release.yml -L 1 --json databaseId -q '.[0].databaseId') --exit-status`.
   Check: `gh run download <id> -n dry-run-bundle-saturnus -D /tmp/saturnus-dry`
   holds seven `saturnus-vV-<target>` archives,
   `saturnus-vV-x86_64-linux.deb`, `saturnus-vV-x86_64-linux.rpm` and
   `saturnus-SHA256SUMS`; `gh run download <id> -n dry-run-bundle-satx -D /tmp/satx-dry`
   holds eight `satx-vV-<target>` archives (with `x86_64-apple-darwin`),
   `satx-vV-x86_64-linux.deb`, `.rpm` and `satx-SHA256SUMS`.
6. **Desktop dry run**: `gh workflow run desktop.yml --ref main`, watch it
   the same way (five build jobs: macOS, and Windows and Linux on x86_64
   and ARM);
   `gh run download <id> -D /tmp/saturnus-app` holds the `.dmg`, the
   `.msi` and `-setup.exe` twice, the `.deb`, `.rpm` and `.AppImage`
   twice, each named with `V` and the architecture:
   `saturnus_V_…` (`saturnus-app_V_…` on Linux; see "Desktop app").
7. **Release** (this publishes: crate versions on crates.io are
   permanent):
   `gh release create vV --target main --title "saturnus V" --notes-file .github/release-notes/vV.md`.
   This starts `release.yml`, and `desktop.yml` once it succeeded (step 8). Watch it as in step 5. Check:
   `gh release view vV --json assets -q '.assets[].name'` lists both
   jobs' assets: the seven saturnus and eight satx archives, the .deb and
   .rpm of each, `saturnus-SHA256SUMS`, `satx-SHA256SUMS`, SBOMs; each crate answers `200` at
   `https://crates.io/api/v1/crates/NAME/V`;
   `cargo install saturnus --locked --root /tmp/saturnus-install && /tmp/saturnus-install/bin/saturnus --version`
   prints `saturnus V` (and likewise `cargo install satx`);
   `gh api repos/ractive/homebrew-tap/contents/Formula/saturnus.rb -q .name`,
   `.../Formula/satx.rb`, `gh api repos/ractive/scoop-bucket/contents/bucket/saturnus.json -q .name`
   and `.../bucket/satx.json` answer.
8. **Installers on the release**: `release.yml`'s success started
   `desktop.yml` with the tag (`gh run list -w desktop.yml -e workflow_run -L 1`); watch it;
   `gh release view vV` now also lists the six installers of the
   one app, and with the Apple secrets the `cask` job has updated its
   cask: `gh api repos/ractive/homebrew-tap/contents/Casks/saturnus-app.rb -q .name`
   answers.
9. **Web page**: `gh workflow run pages.yml --ref main`, with `main`
   still at the tag's commit (`git fetch --tags` and
   `git rev-parse vV^{commit} origin/main` print the same hash): the
   upload job and the `pages` environment, which holds `FTP_PASSWORD`,
   admit `main` only, so a run on the tag builds and skips the upload
   (0.5.0's first try). Watch it as in
   step 5 (both jobs pass: `build`, then `ractive`, the FTPS upload to
   ractive.ch; there is no GitHub Pages deployment any more); open
   <https://ractive.ch/saturnus/app/>, load a ROM
   from a local file, and check the About panel: its first line reads
   `saturnus V, build <id>`, with V this release and `<id>` the 16 hex
   digits `web/site.sh` printed in the `build` job's log ("web/site.sh:
   app/ build <id>, N files precached"). A different build is an
   old service worker: reload, or the update notice's Reload. The
   desktop app's About reads `saturnus V, desktop app`. Then open
   Files (the home page's link or `#/files`): "Add an emulated
   calculator" lists the ROM just loaded in Calculator and connects to
   it (its HOME listed).
10. **docs.rs**: `https://docs.rs/crate/NAME/V` shows a successful build
    for each of the nine crates of the publish list ("crates.io"), minutes
    after step 7.

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
  (`gh release delete-asset vV <file> --yes`) and dispatch it again
  (`gh workflow run desktop.yml --ref main -f release-tag=vV`).
  When a system's build failed, nothing was attached (the `attach` job
  needs every build): fix it on `main` if the cause is in the code (then
  `V+1`), else dispatch again.
- **A cask** fails (token, network, a check of the `.dmg`): rerun the
  failed `cask` job (`gh run rerun <id> --failed`); each cask is its own
  job and never moves a cask backwards.

## Web page: ractive.ch

`.github/workflows/pages.yml` (manual dispatch; only a run dispatched
on `main` publishes, another branch builds the site artifact and stops;
the upload job runs in the `pages` GitHub Environment, deployments from
`main` only, which must hold the `FTP_PASSWORD` secret)
builds the wasm
package (`web/build.sh`), assembles the site with
`web/site.sh` (the landing page `web/landing/` at the top with
`web/site.htaccess` as `.htaccess` and the kill switch `sw.js`; the
app under `app/`: the one document `index.html` with the views'
modules (`tx/` is Files'), the components, the shared code, the one
wasm package, the manifest, the icons and its own service worker with a
build hash; the docs under `docs/` with `llms.txt` at the top,
rendered from `web/docs/pages/` by `web/docs/build.mjs` with the
runner's Node (iteration 57), and `sitemap.xml` at the top, each
address with the date of the last commit touching its sources as
`lastmod` (the build job checks out the whole history, `fetch-depth: 0`,
for that); `emulator/` and `transfer/` hold only the kill-switch
`sw.js`; `web/README.md`, "The site"), and publishes
it by FTP to the owner's site (<https://ractive.ch/saturnus/>, the
app at <https://ractive.ch/saturnus/app/>, `httpdocs/saturnus/` on
ractive.ch; the `FTP_PASSWORD` secret of the `pages` environment, the same the site's
own deploy uses). The app's service worker keeps its own caches
(`saturnus:/saturnus/app/:<build>`); the build log prints one
"web/site.sh: app/ build <id>" line.
The first copy on ractive.ch was committed into that site's repository
by hand (ractive.ch PR 8); from then on this workflow keeps it current.
GitHub Pages (enabled 2026-10-06) was disabled and its site deleted on
2026-10-08 at the owner's request: ractive.ch is the only web address.
Dispatch it after every merge to `main` that changes `web/` or the
bindings. Around the upload the `ractive` job tells IndexNow
(<https://www.indexnow.org>) what changed: before it, it fetches the
live `sitemap.xml`; after it, `scripts/indexnow.mjs` (a sparse checkout
of that one file) POSTs the addresses that are new, have a new
`lastmod` (the date a change landed on `main`: `git log --first-parent`)
or left the sitemap to `api.indexnow.org` with the key whose file
`https://ractive.ch/3d159b02864628aba0cf93708a000930.txt` the
ractive.ch repository serves (saturnus publishes no key file). Nothing
to send, an error answer or no network only log a warning, and both
IndexNow steps `continue-on-error`: they never fail or stop the deploy.
The key file must be live before this job first runs with a new
sitemap, or that one submission is lost (it was, from ractive.ch PR 18). The FTP step uses explicit TLS (`protocol: ftps`). The action
(SamKirkland/FTP-Deploy-Action v4.4.0) keeps `.ftp-deploy-sync-state.json`
in `httpdocs/saturnus/` and syncs against it: a file it deployed earlier
that is no longer in `site/` is deleted on the next run (its log reads
"Uploading", "Deleting", "Replacing"), and its `exclude` patterns are
left out of publishing and deleting alike. A file that never went
through the action (a leftover of the first copy, say) is unknown to the
sync state and stays until removed by hand. The ractive.ch site's own
repository must not carry a copy of the directory any more, so that only
this workflow writes there (its PR 8 copy is to be removed with an
`exclude` of `httpdocs/saturnus/**` in that site's deploy).

`web/site.sh` also checks that every relative module import and `new
URL(...)` of the assembled page names a file in it: before iteration 21
`pages.yml` copied a fixed file list without `romstore.js`, which the
Worker imports since iteration 20, and the next deploy would have broken
the page. It checks every `href` and `src` of the app's
`index.html` the same way. `web/site.sh --list` prints the app's
page files; the desktop app embeds the same set (see "Desktop app").
Built in CI, the
wasm names only the runner's paths (`/home/runner/...`) in its panic
messages; a local build would name the builder's home directory, so the
page is deployed only from CI.

## Desktop app

`.github/workflows/desktop.yml` builds the Tauri app's installers with
`tauri-apps/tauri-action` on macOS (Apple silicon), Windows and Linux,
the last two on x86_64 and on ARM. The ARM builds run natively on
GitHub's ARM runners (`windows-11-arm`, `ubuntu-22.04-arm`; free for
public repositories), with the same steps as x86_64. The architecture is
in each name, as Tauri writes it, so no two collide:

| Platform | Installers |
|---|---|
| macOS Apple silicon | `saturnus_V_aarch64.dmg` |
| Windows x86_64 | `saturnus_V_x64_en-US.msi`, `saturnus_V_x64-setup.exe` |
| Windows ARM | `saturnus_V_arm64_en-US.msi`, `saturnus_V_arm64-setup.exe` |
| Linux x86_64 | `saturnus-app_V_amd64.deb`, `saturnus-app-V-1.x86_64.rpm`, `saturnus-app_V_amd64.AppImage` |
| Linux ARM | `saturnus-app_V_arm64.deb`, `saturnus-app-V-1.aarch64.rpm`, `saturnus-app_V_aarch64.AppImage` |

The ARM builds were first dry-run in run 39259881090 and needed no
step of their own: Linux ARM took 10 min, Windows ARM 13 min (x86_64:
12 and 17). Checked there: the .deb's control says `Architecture:
arm64`, and the app binary in the .deb, the .rpm and the AppImage's
squashfs is `ELF 64-bit ... ARM aarch64`; the .msi's template is
`Arm64` (WiX 3.14) and the app in it and in the setup `.exe` is `PE32+
... Aarch64`. The setup `.exe` itself is NSIS's 32-bit x86 stub on both
Windows builds, which Windows on ARM runs emulated. A
release starts it (`workflow_run`) once `release.yml`, run by the
published release, has succeeded: the tag is that run's `head_branch`,
and a failed release gets no installers and moves no cask; `release.yml`'s
dry runs start nothing. Otherwise it runs when dispatched
by hand (Actions tab or `gh workflow run desktop.yml`), needs no secrets (the Apple ones are optional, see below)
and uploads the installers as workflow
artifacts; given a `release-tag` it builds that tag (not the dispatched
branch) and attaches the installers to that existing release (the
release must exist first, for example from `release.yml`); assets
already on the release are never replaced, so a rerun needs them deleted
by hand first. With them goes one checksums file,
`saturnus-app-SHA256SUMS` (`sha256sum --check` format).
A dispatch with `release-tag` is the rebuild and recovery path
("Recovery"). A pre-release later promoted to a release gets no new run
(the casks skip pre-releases): delete its installers and dispatch.

From 0.2.0 the macOS app is signed with the Developer ID certificate and
notarised (the Apple secrets above); the Windows installers are still
unsigned, and the release notes and the README say how to get past
SmartScreen. (0.1.0's installers were all unsigned: its macOS app needed
`xattr -dr com.apple.quarantine`.) The app embeds the page's files only
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
  with an App Store Connect API key, not an Apple ID password), and
  only for a release tag or a run dispatched on `main`: another branch
  builds unsigned, so no unreviewed code gets the Developer ID. Without
  `APPLE_CERTIFICATE` it builds unsigned exactly as before. The six Apple
  secrets and `HOMEBREW_TAP_TOKEN` are meant for the `release` GitHub
  Environment (deployments from `main` and `v*` tags only): the build
  jobs that sign, and the `cask` job, run in that environment, a build
  that does not sign runs in `unsigned`, which holds no secret. For now
  they stay repository secrets, which every job can read: `release.yml`
  passes them to the shared release-workflows pipeline with
  `secrets: inherit` (the CLIs' macOS signing, the Homebrew formulas),
  and its jobs take no environment until that pipeline gets an
  `environment` input. Move them into `release` then. So a release tag builds from a run on `main` (the `workflow_run` event
and the checklist's dispatch both run there). Before
  building, the `verify` job checks that the tag is a published (not
  draft) release whose commit is on `main`, else the run fails. With it, a
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
  but empty as given. The same secrets sign the CLI's macOS binary
  (release-workflows v0.2.4, "macOS signing").
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

### Files in the app

Iteration 51 ([[iterations/iteration-51-one-desktop-app]]) folded saturnus
tx's app (the former `crates/saturnus-tx-tauri`, iteration 40) into
this one: one app, one identifier (`ch.ractive.saturnus`), one cask. What Files adds:

- **The page's wasm**: Files' protocol, aplet server and private
  calculators run in the page's wasm workers, so `build.rs` embeds the
  wasm package beside `web/site.sh --list`'s files and the CSP allows
  `'wasm-unsafe-eval'` and `worker-src 'self'`. `beforeBuildCommand`
  (run from the repository root) builds it with `web/build.sh`, so
  `desktop.yml` installs wasm-pack and the wasm32 target; a production
  build by the Tauri CLI without them fails, any other build warns.
- **Native parts**: serial ports (`serialport`, without libudev: on Linux
  the ports' USB names come from sysfs) and the computer's files for the
  right pane, as Tauri commands (`transfer.rs`); downloads through a save
  dialog; `folders.json` (the picked folders) in the app's config folder.
  The capability grants nothing more from core (dialogs open from Rust;
  the serial bytes come on a channel). The Linux packages' description
  names the `dialout` group.

### The Homebrew cask

`desktop.yml`'s `cask` job keeps the cask `saturnus-app` in
`ractive/homebrew-tap` (`Casks/saturnus-app.rb`, not `saturnus`, which is
the CLI formula there). The tap's `saturnus-tx` cask (the former separate app
until iteration 51) is removed in the tap by hand. It runs after `attach`, so only with a release tag (a release, or a
dispatch with `release-tag`), and only with the Apple secrets; otherwise its steps
skip. A draft or pre-release, a tag that is not the latest release, or a
version below the tap's leaves the cask alone (a notice says so), so
dispatching an old tag cannot roll it back. It
downloads the release's `saturnus_<version>_aarch64.dmg` and refuses to
go on unless the `.dmg` and the `saturnus.app` in it pass `stapler
validate` and `spctl`, the app is signed by a Developer ID Application
certificate, and its bundle id and version are `ch.ractive.saturnus` and
the tag's. Then it fills `@VERSION@` and `@SHA256@` in
`packaging/homebrew/saturnus-app.rb` (read at the tag) and commits the
result straight to the tap, as release-workflows does for the formula.
The tap is cloned without credentials; `HOMEBREW_TAP_TOKEN` reaches git
only through the environment for the push, and the clone is removed
afterwards. The first such release creates `Casks/`. Change the
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
by exact tag (`@v0.3.1`), `ractive/setup-hyalo@v1` floating on its major
tag, as in hyalo. Both repositories belong to the owner, so a tag is a
reviewed release; Dependabot ignores them.
