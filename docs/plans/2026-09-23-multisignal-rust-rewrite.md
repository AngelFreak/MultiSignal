# MultiSignal (Rust + GTK4 + libadwaita) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace `~/dev/MultiSignal.sh` (Bash + zenity) with a native, modern, polished GNOME app that manages multiple Signal Desktop profiles in one window.

**Architecture:** One crate with a GTK-free core library (`names`, `paths`, `launcher`, `procs`, `profiles`, `store`) that owns every rule and file operation, a thin `system` module with the real adapters (gio trash, launching, snap install), and a `ui` module that only renders state and forwards clicks. Every location and side effect is injected (`Paths`, `Trash`, `Launch`, `Installer` traits), so the whole behaviour can be tested against a temp directory, and the UI can be tested with fakes.

**Tech Stack:** Rust (stable, via rustup) · `gtk4` crate (feature `v4_14`) · `libadwaita` crate (feature `v1_5`) · `thiserror` · `tempfile` (tests). System libraries already installed: GTK 4.14.5, libadwaita 1.5.0.

**Compatibility contract:** Same on-disk format as the Bash script, so both tools can run side by side during the switch-over:
- Profile data: `~/Signal/<name>/`
- Launcher: `~/.local/share/applications/Signal-<name>.desktop` (respects `$XDG_DATA_HOME`), `X-MultiSignal-Profile=<name>`, `Exec=env BAMF_DESKTOP_FILE_HINT=… /snap/bin/signal-desktop "--user-data-dir=…" %U`, mode 0644, no `MimeType`
- Name rule: `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`

---

## UI design

**Approved mockup:** https://claude.ai/artifact/A4wNfeEKb6wwDBmqeM7SWr (clickable; dark, light, narrow, empty, install and both dialogs). When this section and the mockup disagree, the mockup wins.

A desktop app for laptop and desktop screens, styled like a first-party macOS app (System Settings, Mail, Contacts). libadwaita provides the widgets (split view, dialogs, toasts, breakpoints, dark mode), with an Apple-style skin on top from `style*.css`.

| Apple pattern | Built with |
|---|---|
| Sidebar + detail window (macOS System Settings) | `AdwNavigationSplitView`: sidebar and content are each an `AdwNavigationPage` with its own `AdwToolbarView` + `AdwHeaderBar` |
| Folds to list → detail on narrow windows | `AdwBreakpoint` at `max-width: 720sp` setting `collapsed = true`; selecting a row then pushes the detail page (`show_content`) with a "‹ Profiles" back button |
| Sidebar list with a solid blue selection | `GtkListBox` with `.navigation-sidebar`, `SelectionMode::Single`, restyled |
| Grouped cards in the detail pane | `GtkListBox` `.boxed-list` of `AdwActionRow`s (title + value suffix), restyled: 12 px radius, hairlines |
| Desktop dialogs (icon, title, text, right-aligned push buttons) | Custom `AdwDialog` (`content_width` 460), not `AdwAlertDialog`, so buttons sit right-aligned like macOS |
| Right-click menus | `GtkPopoverMenu` from a `gio::Menu`, `has_arrow(false)` |
| Notifications | `AdwToast`, restyled as a small rounded panel |

**Honest limits:** GTK can't blur what's behind a window, so there's no frosted-glass sidebar (it's a solid, slightly lighter surface, like macOS with "reduce transparency"). Window controls stay native GNOME. SF Pro can't be bundled; the font stack is Inter (`fonts-inter`), then Ubuntu Sans.

### Window

- Default size **1100 × 720**, minimum 360 × 480. Size and maximised state are remembered between runs (a small `~/.config/multisignal/window.ini` via `glib::KeyFile`; YAGNI GSettings schemas).
- **Split (≥ 720 px wide):** sidebar 26 % of the width, clamped to 250–320 px; detail content centred with a 680 px maximum width.
- **Collapsed (< 720 px):** the sidebar becomes a full-width list page with a large title (the design approved earlier); tapping a profile opens its detail page.

### Sidebar

```
┌──────────────────────────┐
│      Signal Profiles  [+]│  sidebar header bar (title, blue "+")
│ ┌──────────────────────┐ │
│ │(D) Damon             │ │  selected: solid blue, white text
│ │    110 MB            │ │
│ └──────────────────────┘ │
│  (P) Personal            │  30 px gradient avatar, 14 px name
│      No app menu entry · 20 KB   ← orange warning
│  (U)● UKR                │  green dot on the avatar
│      Running · 77 MB     │  green "Running"
│                          │
├──────────────────────────┤
│ 3 profiles · 1 running · 208 MB │  footer, 12 px secondary
└──────────────────────────┘
```

- Click selects; double-click or Enter opens the profile; right-click → Open Signal / Show Signal · Show in Files · Move to Trash… (disabled while running).
- After a delete, the neighbouring profile is selected. The first profile is selected on start.

### Detail pane

```
                                                           [⋯] (×)
   ╭────╮
   │ D  │  Damon                         ← 96 px avatar, 28 px bold name
   ╰────╯  ○ Not running                  ← or ● Running (green)
           [ Open Signal ] [ Show in Files ]   ← 32 px rounded push buttons

   Storage
   ╭───────────────────────────────────────────────╮
   │ Data folder                    ~/Signal/Damon │
   │ Size                                   110 MB │
   ╰───────────────────────────────────────────────╯
   App Menu
   ╭───────────────────────────────────────────────╮
   │ Menu entry                    Signal (Damon) ✓│   or: Missing  [Repair]
   │ Launcher file   signal-desktop-damon.desktop · made by hand │
   ╰───────────────────────────────────────────────╯
   ╭───────────────────────────────────────────────╮
   │ Move to Trash…                                │   red; grey + footnote while running
   ╰───────────────────────────────────────────────╯
```

- "Open Signal" becomes "Show Signal" while running (launching again brings the window forward).
- The content bar shows the profile name only once the hero has scrolled away; "⋯" holds Repair App Menu Entries and About.
- Live status: running state refreshes every 2 s from `/proc`; sizes on start, after changes and when the window regains focus.

### Whole-window states

- **Empty:** 80 px symbolic icon, "No Profiles Yet" (28 px bold), one line of explanation, blue **Create Profile** button (36 px, 8 px radius).
- **Signal not installed:** same layout, **Install Signal**; while installing, a spinner + "Installing… you may be asked for your password"; cancelled → toast; failed → last lines of `snap` output and **Try Again**.

### Dialogs

**New Profile** (`AdwDialog`, 460 px): gradient avatar showing the first letter as you type · "New Profile" (15 px semibold) · "A separate Signal account with its own messages. It appears in your app menu as “Signal (name)”." · **Name** label + text field (32 px, 7 px radius, blue focus ring; red ring when invalid) · one status line under the field: hint "Letters, digits, dot, underscore and dash." / red error / blue **Use “My-Work”** fix-it / "“Work” already exists." · right-aligned **Cancel** `esc` and blue **Create Profile** `↵` (dimmed until valid). After creating, the new profile is selected and a toast offers **Open**.

**Move to Trash** (`AdwDialog`, 440 px): the profile's avatar with a red trash badge · "Move “Damon” to the Trash?" · "Its messages (110 MB) and app menu entry move to the Trash. You can restore them from the Trash until it's emptied." · **Cancel** (default, focused) and red **Move to Trash**.

Deliberate change from the Bash version: deletion is per profile (detail pane, context menu, or Delete key), not a multi-select checklist.

### Visual tokens

| Token | Light | Dark |
|---|---|---|
| Content background | `#F5F5F7` | `#000000` |
| Sidebar | `#E9E9EE` | `#1C1C1E` |
| Cards | `#FFFFFF` | `#1C1C1E` |
| Dialogs, menus | `#FFFFFF` | `#2C2C2E` |
| Accent (fills, icons) | `#007AFF` | `#0A84FF` |
| Link text | `#0066D6` | `#0A84FF` |
| Sidebar selection | `#0064D2` | `#0B6BDB` |
| Success (dot / text) | `#34C759` / `#248A3D` | `#30D158` |
| Warning text | `#C93400` | `#FF9F0A` |
| Destructive (text / button) | `#D70015` | `#FF453A` / `#E5352B` |
| Secondary text | `rgba(60,60,67,0.75)` | `rgba(235,235,245,0.6)` |
| Hairline | `rgba(60,60,67,0.2)` | `rgba(84,84,88,0.65)` |
| Push button fill | `rgba(0,0,0,0.07)` | `rgba(255,255,255,0.12)` |

Light-mode text colours are Apple's darker "accessible" variants so 12–13 px text keeps 4.5:1 contrast on white. Type: large title 28/700, dialog title 15/600, sidebar name 14/500, body 13–14/400, secondary 12–13. Radii: cards and dialogs 12 px, push buttons 7–8 px, menus 10 px. Sizes are shown Finder-style ("110 MB", "20 KB", "Empty"), see Task 6b.

### Styling (`src/ui/style.css`, `style-light.css`, `style-dark.css`)

libadwaita only auto-loads `style-dark.css` from GResources, which this project doesn't use, so `ui::load_css` loads `style.css` once plus a tokens provider that it swaps on `adw::StyleManager::default().connect_dark_notify`.

`style-dark.css` (light: the other column of the table):

```css
@define-color window_bg_color #000000;
@define-color view_bg_color #000000;
@define-color sidebar_bg_color #1C1C1E;
@define-color card_bg_color #1C1C1E;
@define-color dialog_bg_color #2C2C2E;
@define-color popover_bg_color #2C2C2E;
@define-color headerbar_bg_color alpha(#000000, 0.72);
@define-color accent_bg_color #0A84FF;
@define-color accent_color #0A84FF;
@define-color success_color #30D158;
@define-color warning_color #FF9F0A;
@define-color error_color #FF453A;
@define-color destructive_bg_color #E5352B;
@define-color destructive_color #FF453A;
@define-color secondary_fg rgba(235, 235, 245, 0.6);
@define-color hairline rgba(84, 84, 88, 0.65);
@define-color selection_bg #0B6BDB;
@define-color push_bg rgba(255, 255, 255, 0.12);
```

`style.css` (shared):

```css
window, dialog { font-family: "Inter", "Ubuntu Sans", sans-serif; }

/* Bars: flat; content bar shows a hairline + title only after scrolling */
headerbar { box-shadow: none; }
headerbar .bar-title { opacity: 0; transition: opacity 150ms ease-out; }
headerbar.scrolled .bar-title { opacity: 1; }
headerbar.scrolled { box-shadow: inset 0 -1px @hairline; }
headerbar button.accent-icon { color: @accent_color; }

/* Sidebar */
.navigation-sidebar { background: @sidebar_bg_color; padding: 4px 10px; }
.navigation-sidebar > row { border-radius: 8px; padding: 7px 10px; margin: 1px 0; }
.navigation-sidebar > row:selected { background: @selection_bg; color: white; }
.navigation-sidebar > row:selected .row-secondary,
.navigation-sidebar > row:selected .row-warning,
.navigation-sidebar > row:selected .status-running { color: alpha(white, 0.85); }
.sidebar-footer { padding: 10px 16px 12px; box-shadow: inset 0 1px @hairline; font-size: 12px; color: @secondary_fg; }

/* Detail */
.hero-title { font-size: 28px; font-weight: 700; letter-spacing: -0.02em; }
.hero-avatar { box-shadow: 0 6px 20px alpha(black, 0.25); }
.section-title { font-size: 13px; font-weight: 600; color: @secondary_fg; margin-left: 16px; }
.boxed-list { background: @card_bg_color; border-radius: 12px; box-shadow: none; }
.boxed-list > row { min-height: 44px; }
.boxed-list > row:not(:last-child) { border-bottom: 1px solid @hairline; }
.row-value { color: @secondary_fg; font-feature-settings: "tnum"; }
.row-secondary { font-size: 12px; color: @secondary_fg; font-feature-settings: "tnum"; }
.row-warning { font-size: 12px; color: @warning_color; }
.status-running { font-size: 12px; font-weight: 500; color: @success_color; }
.destructive-row { color: @destructive_color; }
.footnote { font-size: 12px; color: @secondary_fg; margin: 0 16px; }

/* Desktop push buttons */
button.push { min-height: 32px; padding: 0 16px; border-radius: 7px; background: @push_bg; font-weight: 500; }
button.push.suggested-action { background: @accent_bg_color; color: white; font-weight: 600; }
button.push.destructive-action { background: @destructive_bg_color; color: white; font-weight: 600; }
button.push.large { min-height: 36px; padding: 0 22px; border-radius: 8px; font-size: 15px; }
.key-hint { font-size: 11px; opacity: 0.7; margin-left: 6px; }

/* Avatars: soft vertical gradients */
avatar { color: white; font-weight: 600; }
avatar.color1 { background-image: linear-gradient(#5AC8FA, #007AFF); }
avatar.color2 { background-image: linear-gradient(#63E6BE, #30D158); }
avatar.color3 { background-image: linear-gradient(#FFD60A, #FF9F0A); }
avatar.color4 { background-image: linear-gradient(#FF8A80, #FF453A); }
avatar.color5 { background-image: linear-gradient(#DA8FFF, #BF5AF2); }
avatar.color6 { background-image: linear-gradient(#A5ABB8, #858994); }
/* … repeat the six gradients for color7–color14 */
.status-dot { min-width: 9px; min-height: 9px; border-radius: 999px; background: @success_color; box-shadow: 0 0 0 2px @sidebar_bg_color; }
.navigation-sidebar > row:selected .status-dot { box-shadow: 0 0 0 2px @selection_bg; }

/* Dialogs, fields, toasts */
dialog.desktop-dialog > .dialog-contents { border-radius: 12px; padding: 22px 22px 18px; }
.dialog-title { font-size: 15px; font-weight: 600; }
.dialog-body { font-size: 13px; color: @secondary_fg; }
entry.field { min-height: 32px; border-radius: 7px; }
entry.field.error { box-shadow: 0 0 0 3px alpha(@error_color, 0.25); border-color: @error_color; }
.field-status { font-size: 12px; min-height: 18px; }
.field-status.error { color: @error_color; }
toast { border-radius: 10px; }
```

`avatar`, `color1`–`color14`, and the dialog's internal node names are libadwaita internals: confirm them with the GTK Inspector (`GTK_DEBUG=interactive cargo run`) during Task 11/12 and adjust selectors. If the avatar classes differ, fall back to a small custom avatar (round `gtk::Label`, class chosen by hashing the name). The Inspector is also the fastest way to tune every value live against the mockup.

---

## Stage 0: Toolchain

### Task 0: Install Rust and verify GTK development files

**Step 1: Install rustup (needs network; the user runs this)**

Run: `! curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y`
then: `! source ~/.cargo/env`

**Step 2: Install the UI font** (optional but part of the look; the user runs this)

Run: `! sudo apt install fonts-inter`

**Step 3: Verify**

Run: `cargo --version && rustc --version && pkg-config --modversion gtk4 libadwaita-1 && fc-list | grep -c Inter`
Expected: cargo/rustc versions, `4.14.5`, `1.5.0`, and a non-zero Inter count.

---

## Stage 1: Project skeleton

### Task 1: Create the crate and an empty libadwaita window

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `src/lib.rs`, `.gitignore`

**Step 1: Initialise**

```bash
cd ~/dev/multisignal-rs
git init
cargo init --name multisignal
cargo add gtk4 --rename gtk --features v4_14
cargo add libadwaita --rename adw --features v1_5
cargo add thiserror
cargo add --dev tempfile
```

Before continuing, look up the resolved crate versions' API with Context7 (`/gtk-rs/gtk4-rs`) if any call below fails to compile; the docs win over this plan.

**Step 2: Minimal `src/lib.rs`**

```rust
//! MultiSignal: manage several Signal Desktop profiles.
//!
//! Everything except `ui` and `system` is free of GTK so it can be tested
//! against a temporary directory.
```

**Step 3: Minimal `src/main.rs`**

```rust
use adw::prelude::*;
use gtk::glib;

const APP_ID: &str = "io.github.multisignal.MultiSignal";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        adw::ApplicationWindow::builder()
            .application(app)
            .title("Signal Profiles")
            .default_width(1100)
            .default_height(720)
            .build()
            .present();
    });
    app.run()
}
```

**Step 4: Build and run**

Run: `cargo run`
Expected: an empty Adwaita window titled "Signal Profiles" opens; close it.

**Step 5: Commit**

```bash
git add -A
git commit -m "chore: cargo project with gtk4 + libadwaita skeleton"
```

---

## Stage 2: Core library (no GTK)

Each task: write the tests, run them and watch them fail, implement, run them and watch them pass, commit.

### Task 2: Profile name rules (`names`)

**Files:**
- Create: `src/names.rs`
- Modify: `src/lib.rs` (add `pub mod names;`)

**Step 1: Failing tests** (bottom of `src/names.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_names() {
        for n in ["Work", "UKR", "Family_2", "a.b-c", "9lives"] {
            assert!(is_valid(n), "{n}");
        }
    }

    #[test]
    fn rejects_unsafe_names() {
        let long = "a".repeat(65);
        for n in ["", ".", "..", "../x", "a/b", "My Profile", "it's", "a|b", "-rf", ".hidden", " ", long.as_str()] {
            assert!(!is_valid(n), "{n:?}");
        }
    }

    #[test]
    fn suggests_fixed_names() {
        assert_eq!(suggest("My Work").as_deref(), Some("My-Work"));
        assert_eq!(suggest("  My   Work! ").as_deref(), Some("My-Work"));
        assert_eq!(suggest("../evil").as_deref(), Some("evil"));
        assert_eq!(suggest("!!!"), None);
    }

    #[test]
    fn joins_names_for_messages() {
        assert_eq!(join(&["A"]), "A");
        assert_eq!(join(&["A", "B", "C"]), "A, B, C");
    }
}
```

**Step 2:** Run `cargo test names` → FAIL (functions missing).

**Step 3: Implement** (top of `src/names.rs`)

```rust
//! Profile-name rules. Identical to MultiSignal.sh, so both tools agree.

pub const MAX_LEN: usize = 64;

/// Letters, digits, `.`, `_`, `-`, starting with a letter or digit. This rules
/// out empty names, `.`/`..`, `/`, spaces, quotes and leading dashes.
pub fn is_valid(name: &str) -> bool {
    let mut chars = name.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    first_ok
        && name.len() <= MAX_LEN
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Turns a rejected name such as "My Work!" into "My-Work", or `None` if
/// nothing usable is left.
pub fn suggest(input: &str) -> Option<String> {
    let mut out = String::new();
    for c in input.trim().chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_') {
            out.push(c);
        } else if (c.is_whitespace() || c == '-') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let start = out.find(|c: char| c.is_ascii_alphanumeric())?;
    let fixed: String = out[start..].chars().take(MAX_LEN).collect();
    let fixed = fixed.trim_end_matches('-').to_string();
    is_valid(&fixed).then_some(fixed)
}

/// "A, B, C" for messages.
pub fn join<S: AsRef<str>>(names: &[S]) -> String {
    names.iter().map(AsRef::as_ref).collect::<Vec<_>>().join(", ")
}
```

**Step 4:** `cargo test names` → PASS. **Step 5:** `git commit -am "feat: profile name rules"` (add the new file first).

### Task 3: Injectable locations (`paths`)

**Files:** Create `src/paths.rs`; modify `src/lib.rs`.

**Step 1: Failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_locations_from_home() {
        let p = Paths::for_home(Path::new("/h"), None);
        assert_eq!(p.profile_dir("Work"), Path::new("/h/Signal/Work"));
        assert_eq!(p.own_launcher("Work"), Path::new("/h/.local/share/applications/Signal-Work.desktop"));
    }

    #[test]
    fn respects_xdg_data_home() {
        let p = Paths::for_home(Path::new("/h"), Some(Path::new("/data")));
        assert_eq!(p.applications, Path::new("/data/applications"));
    }
}
```

**Step 3: Implement**

```rust
//! Every location the app touches, so tests can point it at a temp dir.

use std::path::{Path, PathBuf};

pub const SNAP_DESKTOP_HINT: &str =
    "/var/lib/snapd/desktop/applications/signal-desktop_signal-desktop.desktop";
pub const SIGNAL_ICON: &str = "/snap/signal-desktop/current/meta/gui/signal-desktop.png";

#[derive(Clone, Debug)]
pub struct Paths {
    /// `~/Signal`: one sub-directory per profile.
    pub signal_base: PathBuf,
    /// `$XDG_DATA_HOME/applications` (default `~/.local/share/applications`).
    pub applications: PathBuf,
    /// Signal binary. `MULTISIGNAL_SIGNAL_BIN` overrides it for smoke tests.
    pub signal_bin: PathBuf,
    /// `/proc`, replaced by a fake tree in tests.
    pub proc_root: PathBuf,
}

impl Paths {
    pub fn for_home(home: &Path, xdg_data_home: Option<&Path>) -> Self {
        let data = xdg_data_home.map_or_else(|| home.join(".local/share"), Path::to_path_buf);
        Self {
            signal_base: home.join("Signal"),
            applications: data.join("applications"),
            signal_bin: PathBuf::from("/snap/bin/signal-desktop"),
            proc_root: PathBuf::from("/proc"),
        }
    }

    /// Reads `HOME`, `XDG_DATA_HOME` and `MULTISIGNAL_SIGNAL_BIN`.
    pub fn from_env() -> Result<Self, String> {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        let xdg = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty());
        let mut paths = Self::for_home(Path::new(&home), xdg.as_deref().map(Path::new));
        if let Some(bin) = std::env::var_os("MULTISIGNAL_SIGNAL_BIN") {
            paths.signal_bin = bin.into();
        }
        Ok(paths)
    }

    pub fn profile_dir(&self, name: &str) -> PathBuf {
        self.signal_base.join(name)
    }

    pub fn own_launcher(&self, name: &str) -> PathBuf {
        self.applications.join(format!("Signal-{name}.desktop"))
    }
}
```

Run, pass, commit: `feat: injectable paths`.

### Task 4: Launchers (`launcher`)

**Files:** Create `src/launcher.rs`; modify `src/lib.rs`.

**Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn paths(dir: &Path) -> Paths {
        Paths::for_home(dir, None)
    }

    #[test]
    fn written_launcher_passes_desktop_file_validate() {
        let tmp = tempfile::tempdir().unwrap();
        let p = paths(tmp.path());
        write(&p, "Work").unwrap();
        let status = std::process::Command::new("desktop-file-validate")
            .arg(p.own_launcher("Work"))
            .status()
            .expect("desktop-file-validate is installed");
        assert!(status.success());
    }

    #[test]
    fn exec_quotes_the_profile_dir_and_has_no_mimetype() {
        let tmp = tempfile::tempdir().unwrap();
        let p = paths(tmp.path());
        let text = render(&p, "Work").unwrap();
        let dir = p.profile_dir("Work");
        assert!(text.contains(&format!("\"--user-data-dir={}\" %U", dir.display())));
        assert!(!text.contains("MimeType"));
        assert!(text.contains("X-MultiSignal-Profile=Work"));
    }

    #[test]
    fn launcher_is_mode_644() {
        let tmp = tempfile::tempdir().unwrap();
        let p = paths(tmp.path());
        write(&p, "Work").unwrap();
        let mode = std::fs::metadata(p.own_launcher("Work")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o644);
    }

    #[test]
    fn finds_own_legacy_and_hand_made_launchers_only_for_that_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let p = paths(tmp.path());
        std::fs::create_dir_all(&p.applications).unwrap();
        let a = p.profile_dir("A");
        let b = p.profile_dir("AB"); // prefix of nothing, but "A" is a prefix of it
        std::fs::write(p.applications.join("Signal-A.desktop"),
            format!("[Desktop Entry]\nExec=sh -c 'x --user-data-dir={} %U'\n", a.display())).unwrap();
        std::fs::write(p.applications.join("custom.desktop"),
            format!("[Desktop Entry]\nExec=env X=1 x --user-data-dir={}\n", a.display())).unwrap();
        std::fs::write(p.applications.join("other.desktop"),
            format!("[Desktop Entry]\nExec=x --user-data-dir={} %U\n", b.display())).unwrap();
        let mut found = find(&p, "A").unwrap();
        found.sort();
        assert_eq!(found, vec![p.applications.join("Signal-A.desktop"), p.applications.join("custom.desktop")]);
    }

    #[test]
    fn refuses_paths_that_cannot_be_quoted_safely() {
        let p = Paths::for_home(Path::new("/home/we$ird"), None);
        assert!(render(&p, "Work").is_err());
    }
}
```

**Step 3: Implement**

```rust
//! Desktop launchers (`.desktop` files) for profiles.

use crate::paths::{Paths, SIGNAL_ICON, SNAP_DESKTOP_HINT};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Launcher text. Fails if the profile path contains characters that the
/// desktop-entry spec would require escaping (`"` `` ` `` `$` `\` `%`, newline);
/// real home directories never do, so refusing is simpler than escaping.
pub fn render(paths: &Paths, name: &str) -> io::Result<String> {
    let dir = paths.profile_dir(name);
    let dir = dir.to_str().ok_or_else(|| invalid("profile path is not UTF-8"))?;
    if dir.contains(['"', '`', '$', '\\', '%', '\n']) {
        return Err(invalid(&format!("cannot write a launcher for the path {dir:?}")));
    }
    Ok(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Signal ({name})\n\
         Comment=Private messaging from your desktop (profile: {name})\n\
         Exec=env BAMF_DESKTOP_FILE_HINT={SNAP_DESKTOP_HINT} {bin} \"--user-data-dir={dir}\" %U\n\
         Icon={SIGNAL_ICON}\n\
         Terminal=false\n\
         StartupWMClass=Signal\n\
         Categories=Network;InstantMessaging;Chat;\n\
         X-SnapInstanceName=signal-desktop\n\
         X-SnapAppName=signal-desktop\n\
         X-MultiSignal-Profile={name}\n",
        bin = paths.signal_bin.display(),
    ))
}

/// Writes `Signal-<name>.desktop` atomically (temp file + rename), mode 0644.
pub fn write(paths: &Paths, name: &str) -> io::Result<PathBuf> {
    let text = render(paths, name)?;
    fs::create_dir_all(&paths.applications)?;
    let target = paths.own_launcher(name);
    let mut tmp = tempfile_in(&paths.applications, name)?;
    tmp.1.write_all(text.as_bytes())?;
    tmp.1.set_permissions(fs::Permissions::from_mode(0o644))?;
    fs::rename(&tmp.0, &target)?;
    Ok(target)
}

/// Every `.desktop` file (ours, legacy or hand-made) whose Exec line starts
/// Signal with this profile's data directory.
pub fn find(paths: &Paths, name: &str) -> io::Result<Vec<PathBuf>> {
    let needle = format!("--user-data-dir={}", paths.profile_dir(name).display());
    let entries = match fs::read_dir(&paths.applications) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut found = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "desktop") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let matches = text.lines().filter(|l| l.starts_with("Exec=")).any(|line| {
            line.match_indices(&needle).any(|(i, _)| {
                // The path must end here, so "A" doesn't match "AB".
                matches!(line[i + needle.len()..].chars().next(), None | Some('"' | '\'' | ' '))
            })
        });
        if matches {
            found.push(path);
        }
    }
    Ok(found)
}

fn tempfile_in(dir: &Path, name: &str) -> io::Result<(PathBuf, fs::File)> {
    let path = dir.join(format!(".Signal-{name}.{}.tmp", std::process::id()));
    let file = fs::File::create(&path)?;
    Ok((path, file))
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.to_string())
}
```

Note: `Option::is_none_or` needs Rust 1.82+; rustup stable is newer.

Run, pass, commit: `feat: render, write and find launchers`.

### Task 5: Running-profile detection (`procs`)

Reads `/proc/<pid>/cmdline` directly instead of `pgrep`, so tests can use a fake `/proc` tree and the check can't match the manager's own processes.

**Files:** Create `src/procs.rs`; modify `src/lib.rs`.

**Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn fake_proc(root: &Path, pid: u32, argv: &[&str]) {
        let dir = root.join(pid.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cmdline"), argv.join("\0") + "\0").unwrap();
    }

    #[test]
    fn finds_signal_processes_by_user_data_dir() {
        let tmp = tempfile::tempdir().unwrap();
        fake_proc(tmp.path(), 10, &["/snap/signal-desktop/945/opt/Signal/signal-desktop", "--user-data-dir=/h/Signal/UKR"]);
        fake_proc(tmp.path(), 11, &["/opt/google/chrome/chrome", "--user-data-dir=/h/Signal/Fake"]);
        fake_proc(tmp.path(), 12, &["bash", "-c", "echo signal-desktop --user-data-dir=/h/Signal/Shell"]);
        std::fs::create_dir_all(tmp.path().join("self")).unwrap(); // non-numeric entries are skipped
        let running = running_data_dirs(tmp.path());
        assert!(running.contains(Path::new("/h/Signal/UKR")));
        assert!(!running.contains(Path::new("/h/Signal/Fake")), "not a Signal process");
        assert!(!running.contains(Path::new("/h/Signal/Shell")), "text inside another argument");
    }

    #[test]
    fn real_proc_does_not_panic() {
        let _ = running_data_dirs(Path::new("/proc"));
    }
}
```

**Step 3: Implement**

```rust
//! Which profiles have a running Signal instance.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Data directories of running Signal processes: any process whose argv[0]
/// is a `signal-desktop` binary and that has a `--user-data-dir=` argument.
pub fn running_data_dirs(proc_root: &Path) -> HashSet<PathBuf> {
    let Ok(entries) = fs::read_dir(proc_root) else { return HashSet::new() };
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|e| fs::read(e.path().join("cmdline")).ok()) // processes vanish; ignore
        .filter_map(|raw| {
            let args: Vec<String> = raw
                .split(|b| *b == 0)
                .map(|a| String::from_utf8_lossy(a).into_owned())
                .collect();
            let argv0 = Path::new(args.first()?);
            if argv0.file_name()? != "signal-desktop" {
                return None;
            }
            args.iter()
                .find_map(|a| a.strip_prefix("--user-data-dir="))
                .map(PathBuf::from)
        })
        .collect()
}
```

Run, pass, commit: `feat: detect running profiles from /proc`.

**Manual check (once):** with a real profile open, `cargo test` won't cover the snap's actual argv. Run
`for p in /proc/[0-9]*; do tr '\0' ' ' < $p/cmdline 2>/dev/null | grep -q -- '--user-data-dir=' && tr '\0' ' ' < $p/cmdline | cut -c1-150; echo; done | grep -i signal`
and confirm argv[0] ends in `/signal-desktop`. If it doesn't, loosen the argv[0] check to "contains `signal-desktop`" and add that case to the test.

**Finding (2026-09-23):** that check hides the real format, because `tr` turns NULs into spaces. Signal (Chromium) rewrites its `/proc/<pid>/cmdline` into one space-joined string with no NULs between arguments, so argv[0] is the whole command line. `procs` parses both layouts (`finds_signal_with_a_rewritten_command_line`, and `store::delete_refuses_profile_running_as_the_real_snap_does`). To inspect the raw format, use `od -c /proc/<pid>/cmdline | head`.

### Task 6: Profiles (`profiles`)

**Files:** Create `src/profiles.rs`; modify `src/lib.rs`.

**Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Paths) {
        let tmp = tempfile::tempdir().unwrap();
        let mut p = Paths::for_home(tmp.path(), None);
        p.proc_root = tmp.path().join("proc");
        (tmp, p)
    }

    #[test]
    fn lists_valid_profile_directories_sorted_case_insensitively() {
        let (_t, p) = setup();
        for d in ["work", "Damon", "UKR", "has space", ".hidden"] {
            std::fs::create_dir_all(p.signal_base.join(d)).unwrap();
        }
        std::fs::write(p.signal_base.join("file.txt"), "x").unwrap();
        assert_eq!(list_names(&p).unwrap(), ["Damon", "UKR", "work"]);
    }

    #[test]
    fn missing_signal_dir_means_no_profiles() {
        let (_t, p) = setup();
        assert!(list_names(&p).unwrap().is_empty());
    }

    #[test]
    fn finds_existing_name_ignoring_case() {
        let (_t, p) = setup();
        std::fs::create_dir_all(p.signal_base.join("Work")).unwrap();
        assert_eq!(existing_ignoring_case(&p, "work").as_deref(), Some("Work"));
        assert_eq!(existing_ignoring_case(&p, "Travel"), None);
    }

    #[test]
    fn load_reports_size_launchers_and_running() {
        let (_t, p) = setup();
        let dir = p.profile_dir("UKR");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/db"), vec![0u8; 5000]).unwrap();
        crate::launcher::write(&p, "UKR").unwrap();
        let pid = p.proc_root.join("42");
        std::fs::create_dir_all(&pid).unwrap();
        std::fs::write(pid.join("cmdline"), format!("/x/signal-desktop\0--user-data-dir={}\0", dir.display())).unwrap();

        let profiles = load_all(&p).unwrap();
        assert_eq!(profiles.len(), 1);
        let ukr = &profiles[0];
        assert!(ukr.size_bytes >= 5000);
        assert!(ukr.running);
        assert_eq!(ukr.launchers, vec![p.own_launcher("UKR")]);
    }
}
```

**Step 3: Implement**

```rust
//! Profiles are the directories in `~/Signal`, not the launchers, so profiles
//! with a missing or hand-made launcher are still found.

use crate::{launcher, names, paths::Paths, procs};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub name: String,
    pub dir: PathBuf,
    pub size_bytes: u64,
    pub running: bool,
    pub launchers: Vec<PathBuf>,
}

pub fn list_names(paths: &Paths) -> io::Result<Vec<String>> {
    let entries = match fs::read_dir(&paths.signal_base) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() && names::is_valid(&name) {
            out.push(name);
        }
    }
    out.sort_by_key(|n| n.to_lowercase());
    Ok(out)
}

/// The existing profile whose name equals `name` ignoring case, so "work" and
/// "Work" can't both exist as near-identical menu entries.
pub fn existing_ignoring_case(paths: &Paths, name: &str) -> Option<String> {
    list_names(paths).ok()?.into_iter().find(|n| n.eq_ignore_ascii_case(name))
}

pub fn load_all(paths: &Paths) -> io::Result<Vec<Profile>> {
    let running = procs::running_data_dirs(&paths.proc_root);
    list_names(paths)?
        .into_iter()
        .map(|name| {
            let dir = paths.profile_dir(&name);
            Ok(Profile {
                size_bytes: dir_size(&dir),
                running: running.contains(&dir),
                launchers: launcher::find(paths, &name)?,
                dir,
                name,
            })
        })
        .collect()
}

/// Total size of regular files, not following symlinks. Unreadable entries
/// count as 0 rather than failing the whole list.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}
```

Run, pass, commit: `feat: list and load profiles`.

### Task 6b: Finder-style sizes (`units`)

`glib::format_size` prints "110.0 MB"; the design shows "110 MB", "20 KB", "1.5 MB" and "Empty".

**Files:** Create `src/units.rs`; modify `src/lib.rs`.

**Step 1: Failing test**

```rust
#[cfg(test)]
mod tests {
    use super::size;

    #[test]
    fn formats_like_finder() {
        assert_eq!(size(0), "Empty");
        assert_eq!(size(512), "512 bytes");
        assert_eq!(size(20_000), "20 KB");
        assert_eq!(size(1_500_000), "1.5 MB");
        assert_eq!(size(110_000_000), "110 MB");
        assert_eq!(size(2_340_000_000), "2.3 GB");
    }
}
```

**Step 3: Implement**

```rust
//! Human-readable sizes, decimal units like Finder and GNOME Files.

pub fn size(bytes: u64) -> String {
    if bytes == 0 {
        return "Empty".into();
    }
    const UNITS: [&str; 4] = ["bytes", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    match unit {
        0 => format!("{bytes} bytes"),
        _ if value < 10.0 => format!("{value:.1} {}", UNITS[unit]),
        _ => format!("{} {}", value.round(), UNITS[unit]),
    }
}
```

Run, pass, commit: `feat: finder-style size formatting`.

### Task 7: The profile store (actions)

All user actions live here, behind injected side effects. The UI calls only this.

**Files:** Create `src/store.rs`, `tests/store.rs`; modify `src/lib.rs`.

**Step 1: Implement the types first** (so the integration tests compile), `src/store.rs`:

```rust
//! Create, delete, repair and launch profiles.

use crate::{launcher, names, paths::Paths, profiles, procs};
use std::io;
use std::path::Path;

/// Moves a path to the Trash. Real: gio. Tests: move into a temp dir.
pub trait Trash {
    fn trash(&self, path: &Path) -> Result<(), String>;
}

/// Starts Signal for a profile. Real: `setsid -f`. Tests: record the call.
pub trait Launch {
    fn launch(&self, paths: &Paths, name: &str) -> io::Result<()>;
}

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("“{input}” isn't a valid profile name")]
    Invalid { input: String, suggestion: Option<String> },
    #[error("A profile named “{0}” already exists")]
    Exists(String),
    #[error("Could not create the profile: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error("There is no profile named “{0}”")]
    NotFound(String),
    #[error("Quit Signal ({0}) first. Closing its window can leave it running in the tray.")]
    Running(String),
    #[error("Could not move “{name}” to the Trash: {reason}")]
    Trash { name: String, reason: String },
}

#[derive(Debug, Default, PartialEq)]
pub struct RepairReport {
    pub updated: Vec<String>,
    pub created: Vec<String>,
    /// (profile, launcher file name) left alone because it's hand-made.
    pub skipped: Vec<(String, String)>,
}

pub struct Store {
    pub paths: Paths,
    trash: Box<dyn Trash>,
    launcher: Box<dyn Launch>,
}

impl Store {
    pub fn new(paths: Paths, trash: Box<dyn Trash>, launcher: Box<dyn Launch>) -> Self {
        Self { paths, trash, launcher }
    }

    pub fn load(&self) -> io::Result<Vec<profiles::Profile>> {
        profiles::load_all(&self.paths)
    }

    /// Validates (trimmed input), refuses duplicates ignoring case, creates
    /// the data directory and launcher. Returns the final name.
    pub fn create(&self, input: &str) -> Result<String, CreateError> {
        let name = input.trim();
        if !names::is_valid(name) {
            return Err(CreateError::Invalid { input: name.to_string(), suggestion: names::suggest(name) });
        }
        if let Some(existing) = profiles::existing_ignoring_case(&self.paths, name) {
            return Err(CreateError::Exists(existing));
        }
        let dir = self.paths.profile_dir(name);
        if dir.exists() {
            return Err(CreateError::Exists(name.to_string()));
        }
        std::fs::create_dir_all(&dir)?;
        launcher::write(&self.paths, name)?;
        Ok(name.to_string())
    }

    /// Trashes launchers first, then data: if the second step fails, what's
    /// left is data without a launcher (fixable with Repair), never a launcher
    /// that would start Signal on a missing directory.
    pub fn delete(&self, name: &str) -> Result<(), DeleteError> {
        let dir = self.paths.profile_dir(name);
        if !names::is_valid(name) || !dir.is_dir() {
            return Err(DeleteError::NotFound(name.to_string()));
        }
        if procs::running_data_dirs(&self.paths.proc_root).contains(&dir) {
            return Err(DeleteError::Running(name.to_string()));
        }
        let trash_err = |reason| DeleteError::Trash { name: name.to_string(), reason };
        let launchers = launcher::find(&self.paths, name).map_err(|e| trash_err(e.to_string()))?;
        for path in launchers.iter().map(AsRef::as_ref).chain([dir.as_path()]) {
            self.trash.trash(path).map_err(trash_err)?;
        }
        Ok(())
    }

    /// Rewrites this tool's launchers in the current format and creates missing
    /// ones. Hand-made launchers are left alone.
    pub fn repair(&self) -> io::Result<RepairReport> {
        let mut report = RepairReport::default();
        for name in profiles::list_names(&self.paths)? {
            let found = launcher::find(&self.paths, &name)?;
            if self.paths.own_launcher(&name).exists() {
                launcher::write(&self.paths, &name)?;
                report.updated.push(name);
            } else if let Some(other) = found.first() {
                let file = other.file_name().unwrap_or_default().to_string_lossy().into_owned();
                report.skipped.push((name, file));
            } else {
                launcher::write(&self.paths, &name)?;
                report.created.push(name);
            }
        }
        Ok(report)
    }

    pub fn launch(&self, name: &str) -> io::Result<()> {
        if !names::is_valid(name) || !self.paths.profile_dir(name).is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("no profile named {name:?}")));
        }
        self.launcher.launch(&self.paths, name)
    }
}
```

**Step 2: Failing integration tests** `tests/store.rs`: one test per behaviour of `MultiSignal.test.sh` (see the parity table at the end).

```rust
use multisignal::paths::Paths;
use multisignal::store::{CreateError, DeleteError, Launch, Store, Trash};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

struct DirTrash(PathBuf);
impl Trash for DirTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        let dest = self.0.join(path.file_name().unwrap());
        std::fs::rename(path, dest).map_err(|e| e.to_string())
    }
}

#[derive(Clone, Default)]
struct Recorder(Rc<RefCell<Vec<String>>>);
impl Launch for Recorder {
    fn launch(&self, _: &Paths, name: &str) -> std::io::Result<()> {
        self.0.borrow_mut().push(name.to_string());
        Ok(())
    }
}

struct Fixture {
    _tmp: tempfile::TempDir,
    store: Store,
    trash: PathBuf,
    launched: Recorder,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let mut paths = Paths::for_home(&tmp.path().join("home"), None);
    paths.proc_root = tmp.path().join("proc");
    let trash = tmp.path().join("trash");
    for d in [&paths.signal_base, &paths.applications, &paths.proc_root, &trash] {
        std::fs::create_dir_all(d).unwrap();
    }
    let launched = Recorder::default();
    let store = Store::new(paths, Box::new(DirTrash(trash.clone())), Box::new(launched.clone()));
    Fixture { _tmp: tmp, store, trash, launched }
}

fn mark_running(f: &Fixture, name: &str) {
    let pid = f.store.paths.proc_root.join("4242");
    std::fs::create_dir_all(&pid).unwrap();
    let dir = f.store.paths.profile_dir(name);
    std::fs::write(pid.join("cmdline"), format!("/x/signal-desktop\0--user-data-dir={}\0", dir.display())).unwrap();
}

#[test]
fn create_trims_and_writes_dir_and_launcher() {
    let f = fixture();
    assert_eq!(f.store.create("  Work  ").unwrap(), "Work");
    assert!(f.store.paths.profile_dir("Work").is_dir());
    assert!(f.store.paths.own_launcher("Work").is_file());
}

#[test]
fn create_rejects_bad_names_without_touching_disk() {
    let f = fixture();
    for bad in ["../evil", "My Profile", "a|b", "."] {
        assert!(matches!(f.store.create(bad), Err(CreateError::Invalid { .. })), "{bad}");
    }
    assert_eq!(std::fs::read_dir(&f.store.paths.signal_base).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(&f.store.paths.applications).unwrap().count(), 0);
}

#[test]
fn create_suggests_a_fixed_name() {
    let f = fixture();
    match f.store.create("My Work") {
        Err(CreateError::Invalid { suggestion, .. }) => assert_eq!(suggestion.as_deref(), Some("My-Work")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn create_rejects_duplicates_ignoring_case() {
    let f = fixture();
    f.store.create("Work").unwrap();
    assert!(matches!(f.store.create("work"), Err(CreateError::Exists(n)) if n == "Work"));
}

#[test]
fn delete_trashes_launchers_and_data_only_for_that_profile() {
    let f = fixture();
    f.store.create("A").unwrap();
    f.store.create("B").unwrap();
    f.store.delete("A").unwrap();
    assert!(f.trash.join("A").is_dir());
    assert!(f.trash.join("Signal-A.desktop").is_file());
    assert!(f.store.paths.profile_dir("B").is_dir());
    assert!(f.store.paths.own_launcher("B").is_file());
}

#[test]
fn delete_never_touches_the_signal_base_dir() {
    let f = fixture();
    f.store.create("B").unwrap();
    std::fs::write(f.store.paths.applications.join("Signal-.desktop"), "").unwrap();
    for bad in ["", ".", "..", "../B"] {
        assert!(matches!(f.store.delete(bad), Err(DeleteError::NotFound(_))), "{bad:?}");
    }
    assert!(f.store.paths.profile_dir("B").is_dir());
}

#[test]
fn delete_refuses_running_profile() {
    let f = fixture();
    f.store.create("A").unwrap();
    mark_running(&f, "A");
    assert!(matches!(f.store.delete("A"), Err(DeleteError::Running(_))));
    assert!(f.store.paths.profile_dir("A").is_dir());
}

#[test]
fn repair_updates_ours_creates_missing_and_skips_hand_made() {
    let f = fixture();
    let p = &f.store.paths;
    for d in ["UKR", "Personal", "Damon"] {
        std::fs::create_dir_all(p.profile_dir(d)).unwrap();
    }
    std::fs::write(p.own_launcher("UKR"),
        format!("[Desktop Entry]\nExec=sh -c 'x --user-data-dir={} %U'\n", p.profile_dir("UKR").display())).unwrap();
    let damon = p.applications.join("signal-desktop-damon.desktop");
    let damon_text = format!("[Desktop Entry]\nExec=env X=1 x --user-data-dir={} %U\n", p.profile_dir("Damon").display());
    std::fs::write(&damon, &damon_text).unwrap();

    let report = f.store.repair().unwrap();
    assert_eq!(report.updated, ["UKR"]);
    assert_eq!(report.created, ["Personal"]);
    assert_eq!(report.skipped, [("Damon".to_string(), "signal-desktop-damon.desktop".to_string())]);
    assert_eq!(std::fs::read_to_string(&damon).unwrap(), damon_text);
    assert!(!p.own_launcher("Damon").exists());
}

#[test]
fn launch_passes_the_profile_to_the_launcher() {
    let f = fixture();
    f.store.create("Work").unwrap();
    f.store.launch("Work").unwrap();
    assert_eq!(*f.launched.0.borrow(), ["Work"]);
    assert!(f.store.launch("Nope").is_err());
}
```

**Step 3:** `cargo test --test store` → the tests that exercise logic pass, and any mistakes fail; fix until green. To prove the tests bite, temporarily break one rule (e.g. comment out the `running_data_dirs` check in `delete`) and confirm `delete_refuses_running_profile` fails, then restore it.

**Step 4:** commit: `feat: profile store with create/delete/repair/launch`.

---

## Stage 3: System adapters (real side effects)

### Task 8: gio Trash, launching, snap install

**Files:** Create `src/system.rs`, `tests/system.rs`; modify `src/lib.rs`.

**Step 1: Implement** `src/system.rs`

```rust
//! Real side effects: gio trash, `setsid -f` launching, snap via pkexec.

use crate::paths::{Paths, SNAP_DESKTOP_HINT};
use crate::store::{Launch, Trash};
use gtk::gio;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct GioTrash;

impl Trash for GioTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        gio::File::for_path(path)
            .trash(gio::Cancellable::NONE)
            .map_err(|e| e.to_string())
    }
}

pub struct SetsidLauncher;

impl Launch for SetsidLauncher {
    /// `setsid -f` detaches Signal so it outlives the manager and never
    /// becomes a zombie child of it.
    fn launch(&self, paths: &Paths, name: &str) -> io::Result<()> {
        let status = Command::new("setsid")
            .arg("-f")
            .arg(&paths.signal_bin)
            .arg(format!("--user-data-dir={}", paths.profile_dir(name).display()))
            .env("BAMF_DESKTOP_FILE_HINT", SNAP_DESKTOP_HINT)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!("setsid exited with {status}")))
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum InstallOutcome {
    Installed,
    Cancelled,
    Failed(String),
}

pub trait Installer {
    fn is_installed(&self) -> bool;
    /// Blocking; call from `gio::spawn_blocking`.
    fn install(&self) -> InstallOutcome;
    /// "apt" / "Flatpak" if Signal is installed some other way.
    fn other_install(&self) -> Option<&'static str>;
}

pub struct SnapInstaller;

impl Installer for SnapInstaller {
    fn is_installed(&self) -> bool {
        Command::new("snap")
            .args(["list", "signal-desktop"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    fn install(&self) -> InstallOutcome {
        let out = match Command::new("pkexec").args(["snap", "install", "signal-desktop"]).output() {
            Ok(o) => o,
            Err(e) => return InstallOutcome::Failed(format!("could not run pkexec: {e}")),
        };
        match out.status.code() {
            _ if self.is_installed() => InstallOutcome::Installed,
            Some(126 | 127) => InstallOutcome::Cancelled, // auth dialog dismissed
            _ => {
                let err = String::from_utf8_lossy(&out.stderr);
                let tail: Vec<&str> = err.lines().rev().take(5).collect();
                InstallOutcome::Failed(tail.into_iter().rev().collect::<Vec<_>>().join("\n"))
            }
        }
    }

    fn other_install(&self) -> Option<&'static str> {
        let flatpak = Command::new("flatpak")
            .args(["info", "org.signal.Signal"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if flatpak {
            Some("Flatpak")
        } else if Path::new("/opt/Signal/signal-desktop").exists() {
            Some("apt")
        } else {
            None
        }
    }
}
```

**Step 2: Integration tests on the real adapters** `tests/system.rs`

```rust
//! Exercises the real adapters. gio reads XDG_DATA_HOME once per process, so
//! this file is its own test binary and sets it before any gio call.

use multisignal::paths::Paths;
use multisignal::store::{Launch, Store, Trash};
use multisignal::system::{GioTrash, SetsidLauncher};
use std::os::unix::fs::PermissionsExt;

#[test]
fn real_adapters_trash_and_launch() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let data = tmp.path().join("data");
    std::env::set_var("XDG_DATA_HOME", &data);

    // gio trash moves into $XDG_DATA_HOME/Trash/files.
    let victim = home.join("Signal/Old");
    std::fs::create_dir_all(&victim).unwrap();
    GioTrash.trash(&victim).unwrap();
    assert!(!victim.exists());
    assert!(data.join("Trash/files/Old").is_dir());

    // setsid launcher runs the configured binary with the profile dir.
    let log = tmp.path().join("launched.log");
    let stub = tmp.path().join("signal-desktop");
    std::fs::write(&stub, format!("#!/bin/sh\necho \"$@\" > {}\n", log.display())).unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut paths = Paths::for_home(&home, Some(&data));
    paths.signal_bin = stub;
    let store = Store::new(paths.clone(), Box::new(GioTrash), Box::new(SetsidLauncher));
    store.create("Work").unwrap();
    store.launch("Work").unwrap();
    let line = (0..40)
        .find_map(|_| { std::thread::sleep(std::time::Duration::from_millis(50)); std::fs::read_to_string(&log).ok() })
        .expect("stub ran");
    assert_eq!(line.trim(), format!("--user-data-dir={}", paths.profile_dir("Work").display()));

    // And delete goes through gio for real.
    store.delete("Work").unwrap();
    assert!(data.join("Trash/files/Work").is_dir());
}
```

(`SnapInstaller` isn't run in tests: it would install software. Its logic is covered through `FakeInstaller` in the UI tests, and manually in Task 14.)

**Step 3:** `cargo test --test system` → PASS. Commit: `feat: gio trash, setsid launcher, snap installer`.

---

## Stage 4: The UI

Structure: plain structs + `Rc`, no GObject subclassing (less boilerplate, easy to read). All widgets are built in code; CSS is embedded with `include_str!`.

```
src/ui/
  mod.rs            Deps (store + installer), build_window(), CSS loading, actions/shortcuts
  window.rs         MainWindow: toast overlay, stack(install | empty | app), split view + breakpoint, reload()
  sidebar_row.rs    one sidebar row per profile (gradient avatar, status dot, secondary line, context menu)
  detail.rs         detail pane: hero, Storage / App Menu cards, Move to Trash card
  create_dialog.rs  AdwDialog with live name validation
  delete_dialog.rs  AdwDialog with destructive confirm
  install_page.rs   whole-window status page + spinner for the snap install
  style*.css        Apple-style skin: shared rules + light/dark tokens
```

Before each UI task, confirm exact method names for the resolved crate versions with Context7 (`/gtk-rs/gtk4-rs`, `/websites/gnome_pages_gitlab_gnome_libadwaita_doc_1-latest`), especially `adw::Dialog::present` (its parent argument became `Option<&impl IsA<Widget>>` in newer bindings), `adw::Breakpoint`/`BreakpointCondition::parse`, `NavigationSplitView`, and the `glib::clone!` syntax.

### Task 9: UI test harness (write this first)

GTK must run on the main thread, so UI tests are one binary with its own `main` (`harness = false`) that runs checks in order.

**Files:** Create `tests/ui.rs`; modify `Cargo.toml`:

```toml
[[test]]
name = "ui"
harness = false
```

`tests/ui.rs` (grows with each UI task):

```rust
//! Builds the real window against fixture profiles and fake side effects,
//! then inspects the widget tree. Needs a display (runs fine in the desktop
//! session; set GDK_BACKEND=x11 under XWayland if Wayland is unavailable).

use multisignal::paths::Paths;
use multisignal::store::{Launch, Store, Trash};
use multisignal::system::{InstallOutcome, Installer};
use multisignal::ui::{self, Deps};

struct NoTrash;
impl Trash for NoTrash { fn trash(&self, _: &std::path::Path) -> Result<(), String> { Ok(()) } }
struct NoLaunch;
impl Launch for NoLaunch { fn launch(&self, _: &Paths, _: &str) -> std::io::Result<()> { Ok(()) } }
struct FakeInstaller(bool);
impl Installer for FakeInstaller {
    fn is_installed(&self) -> bool { self.0 }
    fn install(&self) -> InstallOutcome { InstallOutcome::Installed }
    fn other_install(&self) -> Option<&'static str> { None }
}

fn deps(home: &std::path::Path, installed: bool, profiles: &[&str], running: &[&str]) -> Deps {
    let mut paths = Paths::for_home(home, None);
    paths.proc_root = home.join("proc");
    for p in profiles { std::fs::create_dir_all(paths.profile_dir(p)).unwrap(); }
    for (i, r) in running.iter().enumerate() {
        let pid = paths.proc_root.join((100 + i).to_string());
        std::fs::create_dir_all(&pid).unwrap();
        std::fs::write(pid.join("cmdline"),
            format!("/x/signal-desktop\0--user-data-dir={}\0", paths.profile_dir(r).display())).unwrap();
    }
    Deps {
        store: Store::new(paths, Box::new(NoTrash), Box::new(NoLaunch)),
        installer: Box::new(FakeInstaller(installed)),
    }
}

fn check(name: &str, ok: bool) {
    println!("{} {name}", if ok { "ok  " } else { "FAIL" });
    if !ok { std::process::exit(1); }
}

fn main() {
    adw::init().expect("a display is available");
    // Each UI task appends its checks here.
    println!("ui: all checks passed");
}
```

Also expose `pub mod ui;` and `pub mod system;` from `src/lib.rs` (the `ui` module starts as `pub struct Deps { pub store: Store, pub installer: Box<dyn Installer> }` plus a `build_window(deps: Deps) -> MainWindow` stub).

Run `cargo test --test ui` → prints the final line. Commit: `test: ui test harness`.

### Task 10: Window shell: split view, breakpoint, states, CSS

**Files:** Create `src/ui/mod.rs`, `src/ui/window.rs`, `src/ui/style.css`, `src/ui/style-light.css`, `src/ui/style-dark.css`; modify `src/main.rs`.

**Step 1: Failing checks** (add to `main()` in `tests/ui.rs`)

```rust
let tmp = tempfile::tempdir().unwrap();
let w = ui::build_window(deps(&tmp.path().join("a"), false, &[], &[]));
check("not installed → install page", w.visible_page() == "install");

let w = ui::build_window(deps(&tmp.path().join("b"), true, &[], &[]));
check("no profiles → empty page", w.visible_page() == "empty");

let w = ui::build_window(deps(&tmp.path().join("c"), true, &["UKR", "Damon", "Personal"], &["UKR"]));
check("profiles → app page", w.visible_page() == "app");
check("sidebar sorted by name", w.sidebar_names() == ["Damon", "Personal", "UKR"]);
check("first profile selected on start", w.selected().as_deref() == Some("Damon"));
check("footer summary", w.footer() == "3 profiles · 1 running · Empty");
check("default size is desktop-sized", w.window.default_width() == 1100 && w.window.default_height() == 720);

w.set_collapsed_for_test(true);
w.select("UKR");
check("collapsed: selecting shows the detail page", w.showing_content());
w.go_back_for_test();
check("collapsed: back returns to the list", !w.showing_content());
```

(The fixture profiles are empty directories, so the total is "Empty".) Plus unit tests in `src/ui/mod.rs`:

```rust
#[test]
fn summary_line_reads_naturally() {
    assert_eq!(summary_line(1, 0, 20_000), "1 profile · 20 KB");
    assert_eq!(summary_line(3, 1, 208_000_000), "3 profiles · 1 running · 208 MB");
}
```

**Step 2:** `cargo test` → FAIL.

**Step 3: Implement.** The widget tree (`src/ui/window.rs`):

```
AdwApplicationWindow (default 1100×720, min 360×480)
└─ AdwToastOverlay
   └─ GtkStack  "install" | "empty" | "app"
      └─ "app": AdwNavigationSplitView (min_sidebar_width 250, max 320, sidebar_width_fraction 0.26)
         ├─ sidebar: AdwNavigationPage "Profiles"
         │  └─ AdwToolbarView
         │     ├─ top: AdwHeaderBar (title "Signal Profiles", end: "+" .accent-icon, show_end_title_buttons false)
         │     ├─ content: GtkScrolledWindow → GtkListBox .navigation-sidebar (SelectionMode::Single)
         │     └─ bottom: GtkLabel .sidebar-footer (summary)
         └─ content: AdwNavigationPage (title = selected profile name)
            └─ AdwToolbarView (extend_content_to_top_edge)
               ├─ top: AdwHeaderBar (title widget: GtkLabel .bar-title; end: "⋯" menu)
               └─ content: GtkScrolledWindow → AdwClamp(680) → detail::build(…)  (Task 11)
```

Key code:

```rust
let split = adw::NavigationSplitView::builder()
    .min_sidebar_width(250.0)
    .max_sidebar_width(320.0)
    .sidebar_width_fraction(0.26)
    .sidebar(&sidebar_page)
    .content(&content_page)
    .build();

// Below 720 px the split folds into list → detail navigation.
let narrow = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 720sp").unwrap());
narrow.add_setter(&split, "collapsed", Some(&true.to_value()));
window.add_breakpoint(narrow);

list.connect_row_selected(clone!(@weak this => move |_, row| {
    if let Some(row) = row {
        this.show_profile(&row.widget_name());
        this.split.set_show_content(true); // only has an effect when collapsed
    }
}));
```

(`clone!` syntax changed in glib 0.20+: it's `#[weak] this` inside the macro now; check the resolved version with Context7.)

`MainWindow` fields: `window`, `toasts`, `stack`, `split`, `list`, `footer`, `content_page`, `content_bar`, `detail_holder` (the clamp whose child is replaced on selection), `deps`, `profiles: RefCell<Vec<Profile>>`, `selected: RefCell<Option<String>>`.

`reload()`: if not installed → "install"; load profiles; empty → "empty"; otherwise rebuild sidebar rows (`sidebar_row::build`, Task 11), keep the current selection if it still exists (else the neighbour at the same index, else the first), select it in the list, update the footer, show "app".

Scroll behaviour: the detail `ScrolledWindow`'s `vadjustment().connect_value_changed` toggles `.scrolled` on the content header bar when the value passes 110 (the hero has scrolled away), revealing the profile name in the bar.

Window size memory: on `close_request`, save `default_width`/`default_height`/`is_maximized` to `~/.config/multisignal/window.ini` with `glib::KeyFile`; restore them in `MainWindow::new` (ignore a missing or unreadable file).

`src/ui/mod.rs`: `Deps`, `build_window`, `summary_line`, `load_css` (two providers; tokens swapped on `StyleManager::connect_dark_notify`), `empty_page()` / `more_menu_button()` / `install_actions()` (`new-profile`, `repair`, `about`, `open-selected`, `trash-selected`). `app.set_accels_for_action`: `win.new-profile` ← `<Control>n`, `app.quit` ← `<Control>q`. Enter and Delete are **not** window-wide accels (they would fire while typing in a dialog's text field): Enter opens via the list's `row_activated`; Delete is an `EventControllerKey` on the sidebar list only → `win.trash-selected`.

`src/main.rs`: `connect_startup` → `ui::load_css()`; `connect_activate` → `Deps` from `Paths::from_env()` (missing HOME: print to stderr, exit 1), `GioTrash`, `SetsidLauncher`, `SnapInstaller`; `build_window`; `set_application`; present.

**Step 4:** `cargo test` → PASS; `cargo run` → sidebar + empty detail skeleton at 1100×720; drag the window narrower than 720 px and it folds. Commit: `feat(ui): split-view window with breakpoint and Apple-style skin`.

### Task 11: Sidebar rows, detail pane, live status, context menu

**Files:** Create `src/ui/sidebar_row.rs`, `src/ui/detail.rs`.

**Step 1: Failing checks**

```rust
// Fixture: launchers for Damon (hand-made file name) and UKR; none for Personal; UKR running.
check("running row says Running", w.sidebar_secondary("UKR").starts_with("Running"));
check("missing launcher row warns", w.sidebar_secondary("Personal").starts_with("No app menu entry"));
w.select("UKR");
check("detail title", w.detail_title() == "UKR");
check("running: primary button says Show Signal", w.detail_primary_label() == "Show Signal");
check("running: trash disabled", !w.action_enabled("win.trash-selected"));
w.select("Damon");
check("stopped: primary says Open Signal", w.detail_primary_label() == "Open Signal");
check("stopped: trash enabled", w.action_enabled("win.trash-selected"));
check("hand-made launcher is labelled", w.detail_value("Launcher file").ends_with("· made by hand"));
w.select("Personal");
check("missing launcher offers Repair", w.detail_has_repair());
```

**Step 3: Implement**

`sidebar_row::build(p: &Profile) -> gtk::ListBoxRow` (`widget_name` = profile name):

```
ListBoxRow
└─ Box h, spacing 10
   ├─ Overlay: adw::Avatar(30, name, true) + Box.status-dot (only if running; halign/valign End)
   └─ Box v (valign Center)
      ├─ Label name (14 px, ellipsize End)
      └─ Box h, spacing 4
         ├─ Label .row-warning "No app menu entry ·"   (only if no launcher)
         ├─ Label .status-running "Running ·"          (only if running)
         └─ Label .row-secondary  units::size(bytes)
```

Double-click: a `GtkGestureClick` on the row with `n_press == 2` → launch. Right-click (`set_button(3)`) and long-press → `GtkPopoverMenu` (Open Signal/Show Signal · Show in Files · — · Move to Trash…) pointing at the click position; select the row first so the menu acts on it. Call `popover.unparent()` in `row.connect_destroy`.

`detail::build(win, p) -> gtk::Box` (vertical, spacing 28, margins 72/40/48 when split, 64/16/40 when collapsed; hero switches from row to centred column when collapsed via a second breakpoint setter or by rebuilding on `split.connect_collapsed_notify`):

- Hero: `adw::Avatar(96)` (.hero-avatar) + status dot overlay; name `.hero-title`; status line (dot + "Running"/"Not running"); buttons `push suggested-action` "Open Signal"/"Show Signal" → `win.open-selected`, `push` "Show in Files" → `gio::AppInfo::launch_default_for_uri(&gio::File::for_path(&p.dir).uri(), None::<&gio::AppLaunchContext>)`.
- "Storage" `.section-title` + `.boxed-list`: `AdwActionRow` "Data folder" with suffix `.row-value` "~/Signal/{name}" (home shortened with `~`); "Size" with `units::size`.
- "App Menu" + `.boxed-list`: "Menu entry" → "Signal ({name})" + `object-select-symbolic` in success colour, or `.row-warning` "Missing" + `push` "Repair" (`win.repair`); "Launcher file" → file name (+ " · made by hand" when it isn't `Signal-{name}.desktop`) or "—".
- Danger card: `.boxed-list` with one activatable row, label "Move to Trash…" `.destructive-row` → `win.trash-selected`; insensitive while running, followed by `.footnote` "Quit Signal ({name}) before moving it to the Trash. Closing its window can leave it running in the tray."

Actions `open-selected` / `trash-selected` act on `selected`; `trash-selected` is disabled while that profile runs (updated on every refresh).

Live refresh: `glib::timeout_add_seconds_local(2, …)` compares `procs::running_data_dirs` with the running flags and calls `reload()` only on change (selection and scroll position survive). `window.connect_is_active_notify` → `reload()` on focus (refreshes sizes). After `open-selected`, a one-shot 1.5 s timeout reloads so "Running" appears promptly.

Test accessors on `MainWindow` read the sidebar row labels, the detail widgets (kept in a small `DetailWidgets` struct), and `window.lookup_action(…)`.

Commit: `feat(ui): sidebar rows, detail pane, live status and context menu`.

### Task 12: Desktop dialogs (New Profile, Move to Trash)

**Files:** Create `src/ui/create_dialog.rs`, `src/ui/delete_dialog.rs`.

**Step 1: Failing checks**: the validation logic is a pure function, testable without showing the dialog:

```rust
use multisignal::ui::create_dialog::{validate, Validation};
let tmp2 = tempfile::tempdir().unwrap();
let d = deps(tmp2.path(), true, &["Work"], &[]);
check("valid name", validate(&d.store, "Travel") == Validation::Ok);
check("space → suggestion", validate(&d.store, "My Work") == Validation::Invalid { suggestion: Some("My-Work".into()) });
check("case duplicate", validate(&d.store, "work") == Validation::Exists("Work".into()));
check("empty shows the hint, not an error", validate(&d.store, "") == Validation::Empty);
```

Plus a wiring check that creating through the dialog selects the new profile:

```rust
let w = ui::build_window(deps(&tmp.path().join("d"), true, &["Damon"], &[]));
w.create_via_dialog_for_test("Travel"); // fills the entry and activates the Create button
check("new profile selected", w.selected().as_deref() == Some("Travel"));
```

**Step 3: Implement**

Both dialogs share a small builder, `dialog_frame(icon: &gtk::Widget, title, body) -> (adw::Dialog, gtk::Box /*extra*/, gtk::Box /*buttons*/)`:

```
AdwDialog .desktop-dialog (content_width 460 / 440, follows_content_size)
└─ Box v, spacing 16, margins 22/22/18
   ├─ Box h, spacing 16:  icon (52 px avatar)  +  Box v (Label .dialog-title, Label .dialog-body, wrap)
   ├─ extra (indented 68 px to line up under the title)
   └─ Box h, halign End, spacing 8: push buttons
```

`create_dialog.rs`:
- `pub enum Validation { Empty, Ok, Invalid { suggestion: Option<String> }, Exists(String) }`, `pub fn validate(store, input) -> Validation` (names + `profiles::existing_ignoring_case`; no writes).
- Icon: `adw::Avatar(52, "", true)` whose text follows the entry (`avatar.set_text(Some(&text))`), so the initial appears while typing.
- Extra: Label "Name" (12 px semibold, secondary), `gtk::Entry` `.field` (placeholder "e.g. Work", `activates_default(true)`), a status line box: hint Label / `.field-status.error` Label / flat `accent-icon` Button "Use “…”".
- Buttons: `push` "Cancel" with `.key-hint` "esc" → `dialog.close()`; `push suggested-action` "Create Profile" with `.key-hint` "↵", `set_default_widget`, sensitive only when `Validation::Ok`.
- `entry.connect_changed` → validate → update sensitivity, the entry's `.error` class, the status line, and the body text's "Signal (name)" preview.
- Create: `store.create(text)` → `win.reload()` + select it + toast "“{name}” created" (`button_label("Open")`, `connect_button_clicked` → launch); `Err(e)` → show `e` in the status line and keep the dialog open.

`delete_dialog.rs`:
- Icon: the profile's 52 px avatar in an `Overlay` with a red circular badge (`user-trash-symbolic` on `@destructive_bg_color`).
- Title "Move “{name}” to the Trash?"; body "Its messages ({size}) and app menu entry move to the Trash. You can restore them from the Trash until it's emptied."
- Buttons: `push` "Cancel" (default widget + initial focus) and `push destructive-action` "Move to Trash".
- Confirm: `store.delete(name)`; success → reload (neighbour gets selected) + toast "“{name}” moved to Trash"; error → toast with the message (e.g. it started running meanwhile).

Escape closes both (AdwDialog default). On narrow windows AdwDialog shows as a bottom sheet automatically, which is fine.

**Step 4:** `cargo test --test ui` → PASS. Manual pass against the mockup's two dialog boards. Commit: `feat(ui): desktop create and trash dialogs`.

### Task 13: Install page

**Files:** Create `src/ui/install_page.rs`.

**Step 1: Failing check**

```rust
// FakeInstaller(false) with install() returning Installed: after the install
// future completes, the window reloads to the empty page.
let tmp3 = tempfile::tempdir().unwrap();
let w = ui::build_window(deps(tmp3.path(), false, &[], &[]));
check("install page shown", w.visible_page() == "install");
w.run_install_for_test(); // runs the same code as the button, awaiting on the main context
check("after install → empty page", w.visible_page() == "empty");
```

(Make `FakeInstaller` flip an `Rc<Cell<bool>>` to `true` inside `install()`.)

**Step 3: Implement**: `AdwStatusPage` (icon `system-software-install-symbolic`, title "Signal Desktop Isn't Installed", description including `other_install()` if any), a `push suggested-action large` button "Install Signal" (the whole-window layout from the design section). On click: replace the button with `gtk::Spinner` (spinning) + label "Installing… you may be asked for your password"; `glib::spawn_future_local(async move { let outcome = gio::spawn_blocking(move || installer.install()).await; … })`. (The installer must be `Send` for `spawn_blocking`; give `Installer` a `Send + Sync` bound, or build a fresh `SnapInstaller` inside the closure.) `Installed` → `reload()`; `Cancelled` → restore button + toast "Installation cancelled"; `Failed(msg)` → restore button labelled "Try Again" + show `msg` in a `caption` label.

Commit: `feat(ui): install page with async snap install`.

### Task 14: Visual smoke test and polish pass

**Files:** Create `scripts/ui-smoke.sh`.

This proves it looks right, not just that the widget tree is right. It uses the same technique used to review the Bash version: X11 backend + software rendering so `import` can screenshot the window.

```bash
#!/bin/bash
# Screenshots every app state in a sandbox HOME. Review target/ui-shots/*.png.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --quiet
out=target/ui-shots; rm -rf "$out"; mkdir -p "$out"
sandbox=$(mktemp -d); trap 'rm -rf "$sandbox"; pkill -f target/debug/multisignal || true' EXIT
export HOME=$sandbox/home XDG_DATA_HOME=$sandbox/home/.local/share
export MULTISIGNAL_SIGNAL_BIN=/bin/true GDK_BACKEND=x11 GSK_RENDERER=cairo
mkdir -p "$HOME/Signal"

shot() {  # name [color-scheme]
    ADW_DEBUG_COLOR_SCHEME=${2:-default} target/debug/multisignal &
    local id=""
    for _ in $(seq 50); do id=$(xdotool search --onlyvisible --name "Signal Profiles" | tail -1) && [[ -n $id ]] && break; sleep 0.1; done
    sleep 0.8; import -window "$id" "$out/$1.png"; kill %1; wait || true
}

shot 01-empty force-dark
mkdir -p "$HOME/Signal"/{Damon,Personal,UKR,Work}
head -c 30M /dev/zero > "$HOME/Signal/Work/db"
shot 02-desktop-light force-light
shot 03-desktop-dark force-dark
echo "Screenshots in $out"
```

**Steps:**
1. Run `scripts/ui-smoke.sh`; open every PNG (Read tool) and compare side by side with the mockup boards: spacing, alignment, avatar colours, selection colour, dialog layout, contrast in both themes. Also shoot the folded layout (`xdotool windowsize "$id" 400 720` before `import`) and a 1440×900 maximised window.
2. Manually exercise with the real system once: install flow (on a VM or after `snap remove signal-desktop` only if the user agrees), open a real profile, confirm the green dot appears within 2 s, trash a throwaway profile and restore it from the Files Trash.
3. Fix what looks off; re-run. Commit: `test: visual smoke script` and `style: polish from smoke review`.

---

## Stage 5: Install and switch over

### Task 15: Install for the user

**Files:** Create `data/io.github.multisignal.MultiSignal.desktop`, `Makefile`.

```ini
[Desktop Entry]
Type=Application
Name=Signal Profiles
Comment=Run several Signal accounts side by side
Exec=multisignal
Icon=system-users
Terminal=false
Categories=Network;InstantMessaging;Utility;
StartupNotify=true
```

```make
PREFIX ?= $(HOME)/.local
install:
	cargo build --release
	install -Dm755 target/release/multisignal $(PREFIX)/bin/multisignal
	install -Dm644 data/io.github.multisignal.MultiSignal.desktop \
		$(PREFIX)/share/applications/io.github.multisignal.MultiSignal.desktop
uninstall:
	rm -f $(PREFIX)/bin/multisignal \
		$(PREFIX)/share/applications/io.github.multisignal.MultiSignal.desktop
```

Verify: `desktop-file-validate data/*.desktop`, `make install`, open "Signal Profiles" from the app grid. Commit: `build: user install target`.

(App ID `io.github.multisignal.MultiSignal` is a placeholder; change it to a domain/GitHub account you own before publishing anywhere.)

### Task 16: Retire the Bash script

1. Walk the parity table below; every row must have a passing Rust test.
2. Run both tools against the real `~/Signal` (read-only actions: list, open) and confirm they agree.
3. Move `~/dev/MultiSignal.sh`, `.bak` and `.test.sh` into `~/dev/multisignal-rs/legacy/` (keep for reference). Commit.

---

## Parity table (Bash test → Rust test)

| MultiSignal.test.sh | Rust |
|---|---|
| test_profile_name_validation | `names::tests::{accepts_valid_names, rejects_unsafe_names}` |
| test_launcher_is_valid_desktop_entry | `launcher::tests::{written_launcher_passes_desktop_file_validate, exec_quotes_…}` |
| (permissions fix) | `launcher::tests::launcher_is_mode_644` |
| test_create_profile | `store::create_trims_and_writes_dir_and_launcher` |
| test_create_rejects_bad_names | `store::create_rejects_bad_names_without_touching_disk` |
| test_create_suggests_valid_name | `store::create_suggests_a_fixed_name` + ui `validate` checks |
| test_create_rejects_(case_insensitive_)duplicate | `store::create_rejects_duplicates_ignoring_case` |
| test_create_then_launch_now | ui: toast "Open" button (Task 12 manual) |
| test_delete_moves_profile_to_trash / finds_hand_made_launcher | `store::delete_trashes_…`, `launcher::tests::finds_…` |
| test_delete_ignores_empty_selection | `store::delete_never_touches_the_signal_base_dir` |
| test_delete_refuses_running_profile | `store::delete_refuses_running_profile` + ui trash-disabled check |
| test_delete_cancel_keeps_everything | delete dialog `close_response("cancel")` (manual, Task 12) |
| test_launch_profile | `store::launch_…` + `system::real_adapters_trash_and_launch` |
| test_repair_launchers | `store::repair_updates_ours_creates_missing_and_skips_hand_made` |
| test_install_* | ui install-page checks with `FakeInstaller` |
| test_lists_grow_with_profile_count / main_menu_shows_every_action | n/a: scrolling window, verified by the smoke screenshots |
| test_menu_cancel_exits_cleanly / test_create_prompt_has_no_underscore | n/a: zenity-only behaviour |
| test_delete_skips_running_profile_when_asked / test_delete_lists_names_with_commas | n/a: multi-select delete is out of scope (per-profile delete instead) |

Added during implementation (beyond the Bash suite): `store::create_failure_leaves_no_half_made_profile`, `store::delete_trashes_hand_made_launchers_too`, `store::delete_refuses_profile_running_as_the_real_snap_does`, `procs::tests::finds_signal_with_a_rewritten_command_line`, `launcher::tests::{write_leaves_no_temp_files_behind, reads_the_menu_name}`, and ui wiring checks (open launches, trash via dialog selects the neighbour, Repair writes the launcher, Delete key, context menu, CSS parses). Real-system check (2026-09-23): the Bash functions and the Rust core report the same profiles, running state and launchers for the real `~/Signal`.

## Out of scope (YAGNI until asked)

Rename profile · multi-select delete · restoring from Trash inside the app · Flatpak/apt Signal support · per-profile custom icons/window classes · translations.
