---
title: "The desktop app in the Mac App Store"
type: backlog
date: 2026-10-09
status: deferred
priority: low
tags:
  - backlog
  - saturnus
  - desktop
  - release
---

# The desktop app in the Mac App Store

Owner (2026-10-09), after Developer ID signing was set up: "when we have
a stable version, publish it to the app store". Not before a stable
release; plan it as its own iteration.

The notarised download and a Homebrew cask already give Mac users a clean
install. The store adds discovery and updates, at the cost of what follows.

## What it takes

- **Certificates and records.** An Apple Distribution certificate and a
  Mac Installer Distribution certificate (the Developer ID ones do not
  apply), a provisioning profile, an app record in App Store Connect,
  screenshots, a privacy policy and the privacy questionnaire, and a
  review of every update.
- **App Sandbox (required).**
  - ROMs are remembered by path and read again later. In the sandbox that
    needs security-scoped bookmarks kept per ROM, so the ROM store needs
    rework.
  - The serial bridge and the control API listen on 127.0.0.1, which
    needs the network server entitlement. Expect review questions.
  - The data folder moves into the app's container.
- **A separate build.** The sandboxed, store-signed build sits next to the
  Developer ID one, so the release workflow builds both.

## Review risks

- **Emulators** are allowed since Apple's 2024 change to guideline 4.7.
- **The ROM download** fetches HP's software from hpcalc.org. Review may ask
  for proof of rights; hpcalc.org's permission from HP covers use with
  emulators, which may not satisfy Apple. The store build should drop the
  Download button and let people choose their own file.
- **Trademarks** (guideline 5.2). "HP 48GX" in the app name or keywords,
  and skins that copy HP's design closely, can be rejected without HP's
  permission. Use a name such as "saturnus: Saturn calculator emulator"
  and name the models only in the description.

## iPhone and iPad

The web page already installs as an app (PWA) on iOS. A native iOS app
through Tauri's iOS target is a new project with the same review risks.

## First step

A sandboxed build of the desktop app with ROMs chosen by file only, to see
what breaks before any store work.
