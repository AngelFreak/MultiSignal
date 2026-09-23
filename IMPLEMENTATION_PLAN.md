# Default Signal: move, lock, and link routing — Implementation Plan

Requested 2026-09-23: empty the default Signal profile, lock it from use, and
route re-authentication links (`signalcaptcha://`, `sgnl://`) to the right
profile. Decisions: "Empty" moves the default data into a named profile
(nothing is lost); "lock" hides the snap's Signal menu entry for this user;
links go to the running profile, else a dialog asks.

Contract check (docs/plans/…rust-rewrite.md, "Compatibility contract"): profile
launchers keep **no MimeType** (several launchers claiming the schemes means
the last one written wins, which is today's bug: every captcha goes to Damon).
Instead Signal Profiles' own desktop entry claims the two schemes and routes.

## Stage 1: Move the default Signal into a profile
**Goal**: `Store::adopt_default(name)` renames `~/snap/signal-desktop/current/.config/Signal` to `~/Signal/<name>` and writes its launcher; the default's detail pane gets "Move to Profile…" (dialog with the name, pre-filled from the default's display name).
**Success Criteria**: data moved in one rename, launcher written, default entry gone; refused while the default runs, for bad or taken names, and when there is no default data.
**Tests**: `tests/store.rs` adopt cases; `tests/ui.rs` adopt through the dialog selects the new profile and clears the default's display name.
**Status**: Complete

## Stage 2: Lock the default Signal
**Goal**: `lock` module writes/removes a per-user override `~/.local/share/applications/signal-desktop_signal-desktop.desktop` with `Hidden=true` (marked `X-MultiSignal-Lock=true`); UI shows Locked, disables Open, offers Lock/Unlock; the default row stays visible while locked even with no data.
**Success Criteria**: with the override, gio no longer finds the snap's entry; unlock removes only our own override.
**Tests**: `lock` unit tests; `tests/lock.rs` (own binary, sandboxed XDG dirs) proves GIO hides the entry; ui lock/unlock checks.
**Status**: Complete

## Stage 3: Route Signal links
**Goal**: the app's desktop entry claims `x-scheme-handler/sgnl` and `x-scheme-handler/signalcaptcha` (`Exec=multisignal %u`, `HANDLES_COMMAND_LINE`: raw arguments, because GIO rewrites `scheme://token` as `scheme://token/`); a GTK-free `route()` picks the single running profile, else the window asks; delivery runs `signal-desktop [--user-data-dir=…] <link>` so the running instance receives it. First run makes the app the default handler; "Handle Signal Links" in ⋯ toggles it.
**Success Criteria**: a link opened through the real `GApplication` path reaches the right profile's launch call.
**Tests**: `route` unit tests; store/system link delivery; ui chooser; an application-level test that calls `open` on the registered app.
**Status**: Complete

## Stage 4: Ship
**Goal**: README, design doc, screenshots, `.deb` (desktop entry with MimeType), install locally, push.
**Success Criteria**: full suite green locally and on the workflow; package installs; real link handler set.
**Status**: Not Started
