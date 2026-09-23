# MultiSignal (Rust) — Implementation Plan

Detailed, task-by-task plan with code: [docs/plans/2026-09-23-multisignal-rust-rewrite.md](docs/plans/2026-09-23-multisignal-rust-rewrite.md).
Replaces `~/dev/MultiSignal.sh` with a native GTK4 + libadwaita app with an Apple-style (iOS/macOS) look.

## Stage 0–1: Toolchain and skeleton
**Goal**: Rust installed (+ Inter font); `cargo run` opens an empty libadwaita window.
**Success Criteria**: `cargo build` succeeds against GTK 4.14 / libadwaita 1.5.
**Tests**: none yet (build only).
**Status**: Complete — Rust 1.98.1, gtk4 0.11.5, libadwaita 0.9.2; skeleton committed. Open: Inter font not installed yet (`sudo apt install fonts-inter`, needed before the Stage 4 visual review)

## Stage 2: Core library (no GTK)
**Goal**: name rules, paths, launchers, running detection, profile loading, and the `Store` (create/delete/repair/launch), same on-disk format as the Bash script.
**Success Criteria**: every behaviour in `MultiSignal.test.sh` has a passing Rust test (parity table in the detailed plan).
**Tests**: unit tests per module; `tests/store.rs` against a temp HOME with a fake `/proc`.
**Status**: Complete — 24 unit + 11 integration tests. Store-level rows of the parity table are covered; UI-level rows (install, create-then-launch, delete cancel) land in Stage 4. Bash tests not in the parity table: `menu_cancel_exits_cleanly` and `create_prompt_has_no_underscore` are zenity-only; `delete_skips_running_profile_when_asked` and `delete_lists_names_with_commas` are multi-select, which is out of scope.

## Stage 3: System adapters
**Goal**: real gio Trash, detached `setsid -f` launching, snap install via pkexec.
**Success Criteria**: the real adapters trash into `$XDG_DATA_HOME/Trash` and launch a stub binary with the right profile.
**Tests**: `tests/system.rs`.
**Status**: Not Started

## Stage 4: Apple-style desktop UI
**Goal**: the approved mockup (https://claude.ai/artifact/A4wNfeEKb6wwDBmqeM7SWr): 1100×720 split view (sidebar + detail pane) folding to list → detail below 720 px, desktop dialogs, context menus, live running status, install page, light/dark tokens.
**Success Criteria**: `tests/ui.rs` passes; `scripts/ui-smoke.sh` screenshots (light, dark, folded, maximised) match the mockup boards.
**Tests**: `tests/ui.rs` (widget tree against fixtures + fakes), smoke screenshots reviewed side by side with the mockup.
**Status**: Not Started

## Stage 5: Install and switch over
**Goal**: `make install` to `~/.local`, app appears as "Signal Profiles"; Bash script moved to `legacy/`.
**Success Criteria**: parity table complete; both tools agree on the real `~/Signal`.
**Tests**: `desktop-file-validate`, manual run against real profiles.
**Status**: Not Started
