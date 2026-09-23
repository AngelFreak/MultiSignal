# MultiSignal (Rust) — Implementation Plan

Detailed, task-by-task plan with code: [docs/plans/2026-09-23-multisignal-rust-rewrite.md](docs/plans/2026-09-23-multisignal-rust-rewrite.md).
Replaces `~/dev/MultiSignal.sh` with a native GTK4 + libadwaita app with an Apple-style (iOS/macOS) look.

## Stage 0–1: Toolchain and skeleton
**Goal**: Rust installed (+ Inter font); `cargo run` opens an empty libadwaita window.
**Success Criteria**: `cargo build` succeeds against GTK 4.14 / libadwaita 1.5.
**Tests**: none yet (build only).
**Status**: In Progress — Rust 1.98.1 installed, crate created (gtk4 0.11.5, libadwaita 0.9.2), skeleton builds; Inter font not installed yet; not committed

## Stage 2: Core library (no GTK)
**Goal**: name rules, paths, launchers, running detection, profile loading, and the `Store` (create/delete/repair/launch), same on-disk format as the Bash script.
**Success Criteria**: every behaviour in `MultiSignal.test.sh` has a passing Rust test (parity table in the detailed plan).
**Tests**: unit tests per module; `tests/store.rs` against a temp HOME with a fake `/proc`.
**Status**: Not Started

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
