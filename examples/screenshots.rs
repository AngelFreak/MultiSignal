//! Renders every app state to PNG for side-by-side review with the mockup:
//! `cargo run --example screenshots [out-dir]` (default `target/ui-shots`).
//!
//! GTK draws the window itself (`WidgetPaintable`), so the images have exact
//! logical sizes regardless of display scaling. The window is the real one,
//! built on fixture profiles with fake side effects; the stylesheet is the
//! bundled one.

use adw::prelude::*;
use gtk::{gio, glib, graphene};
use multisignal::paths::Paths;
use multisignal::store::{Launch, Store, Trash};
use multisignal::system::{InstallOutcome, Installer};
use multisignal::ui::{self, Deps, MainWindow};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

struct NoTrash;
impl Trash for NoTrash {
    fn trash(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
}
struct NoLaunch;
impl Launch for NoLaunch {
    fn launch(&self, _: &Paths, _: &str) -> std::io::Result<()> {
        Ok(())
    }
    fn launch_default(&self, _: &Paths) -> std::io::Result<()> {
        Ok(())
    }
}
struct FixedInstaller(bool);
impl Installer for FixedInstaller {
    fn is_installed(&self) -> bool {
        self.0
    }
    fn install(&self) -> InstallOutcome {
        InstallOutcome::Cancelled
    }
    fn other_install(&self) -> Option<&'static str> {
        None
    }
}

/// Profiles as in the mockup: Damon (hand-made launcher, 110 MB), Personal
/// (no launcher, 20 KB), UKR (running, 77 MB), Work (empty), plus the snap's
/// default Signal (running, 3.3 GB).
fn fixture_home(home: &Path) -> Paths {
    let mut paths = Paths::for_home(home, None);
    paths.proc_root = home.join("proc");
    for (name, bytes) in [
        ("Damon", 110_000_000),
        ("Personal", 20_000),
        ("UKR", 77_000_000),
        ("Work", 0),
    ] {
        let dir = paths.profile_dir(name);
        std::fs::create_dir_all(&dir).unwrap();
        if bytes > 0 {
            std::fs::File::create(dir.join("db"))
                .unwrap()
                .set_len(bytes)
                .unwrap();
        }
    }
    for name in ["UKR", "Work"] {
        multisignal::launcher::write(&paths, name).unwrap();
    }
    std::fs::write(
        paths.applications.join("signal-desktop-damon.desktop"),
        format!(
            "[Desktop Entry]\nName=Signal (Damon)\nExec=env X=1 /snap/bin/signal-desktop --user-data-dir={} %U\n",
            paths.profile_dir("Damon").display()
        ),
    )
    .unwrap();
    // The snap's own profile, running from the normal "Signal" entry.
    std::fs::create_dir_all(&paths.default_data_dir).unwrap();
    std::fs::File::create(paths.default_data_dir.join("db"))
        .unwrap()
        .set_len(3_300_000_000)
        .unwrap();
    let default_pid = paths.proc_root.join("99");
    std::fs::create_dir_all(&default_pid).unwrap();
    std::fs::write(
        default_pid.join("cmdline"),
        "/snap/signal-desktop/945/opt/Signal/signal-desktop --no-sandbox --disable-gpu\0",
    )
    .unwrap();
    let pid = paths.proc_root.join("100");
    std::fs::create_dir_all(&pid).unwrap();
    std::fs::write(
        pid.join("cmdline"),
        format!(
            "/snap/signal-desktop/1/signal-desktop\0--user-data-dir={}\0",
            paths.profile_dir("UKR").display()
        ),
    )
    .unwrap();
    paths
}

fn window(paths: &Paths, installed: bool, size: (i32, i32)) -> Rc<MainWindow> {
    let deps = Deps {
        store: Store::new(paths.clone(), Box::new(NoTrash), Box::new(NoLaunch)),
        installer: Arc::new(FixedInstaller(installed)),
    };
    let w = ui::build_window(deps);
    w.window.set_default_size(size.0, size.1);
    // Tiling window managers resize new windows; they leave fixed-size ones
    // floating, so the screenshot has the size asked for.
    w.window.set_resizable(false);
    w.window.present();
    settle(700);
    w
}

/// Runs the main loop for `ms` so layout, breakpoints and dialogs settle.
fn settle(ms: u64) {
    let done = Rc::new(Cell::new(false));
    let flag = done.clone();
    glib::timeout_add_local_once(Duration::from_millis(ms), move || flag.set(true));
    while !done.get() {
        glib::MainContext::default().iteration(true);
    }
}

fn save(w: &MainWindow, out: &Path, name: &str) {
    save_window(&w.window, out, name);
}

/// A dialog is drawn in the main window, or, when that window can't host it
/// (fixed-size windows can't), in a window of its own; shoot whichever it is.
fn save_dialog(w: &MainWindow, dialog: &adw::Dialog, out: &Path, name: &str) {
    match dialog.root().and_downcast::<gtk::Window>() {
        Some(own) if own != *w.window.upcast_ref::<gtk::Window>() => save_window(&own, out, name),
        _ => save(w, out, name),
    }
}

/// Draws any surface (a window or a popover) to PNG.
fn save_window(window: &impl IsA<gtk::Native>, out: &Path, name: &str) {
    let window = window.upcast_ref::<gtk::Native>();
    let (width, height) = (window.width() as f32, window.height() as f32);
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width as f64, height as f64);
    let node = snapshot.to_node().expect("the window drew something");
    let renderer = window.renderer().expect("a realized window has a renderer");
    let texture =
        renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, width, height)));
    let path = out.join(format!("{name}.png"));
    texture.save_to_png(&path).expect("PNG written");
    println!("  {name} ({width}×{height})");
}

fn scheme(dark: bool) {
    adw::StyleManager::default().set_color_scheme(if dark {
        adw::ColorScheme::ForceDark
    } else {
        adw::ColorScheme::ForceLight
    });
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/ui-shots"), PathBuf::from);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let app = adw::Application::builder()
        .application_id("io.github.multisignal.Screenshots")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| {
        let _hold = app.hold();
        ui::load_css();
        let tmp = tempfile::tempdir().unwrap();
        let empty = Paths::for_home(&tmp.path().join("empty"), None);
        let full = fixture_home(&tmp.path().join("full"));
        println!("Screenshots in {}:", out.display());

        for dark in [true, false] {
            scheme(dark);
            let theme = if dark { "dark" } else { "light" };

            let w = window(&full, true, (1100, 720));
            save(&w, &out, &format!("default-{theme}"));
            w.select("Damon");
            settle(300);
            save(&w, &out, &format!("desktop-{theme}"));
            if let Some(menu) = w.open_more_menu_for_test() {
                settle(400);
                save_window(&menu, &out, &format!("more-menu-{theme}"));
                menu.popdown();
            }
            w.select("UKR");
            settle(300);
            save(&w, &out, &format!("running-{theme}"));
            w.select("Damon");
            let dialog = w.open_create_dialog();
            dialog.entry.set_text("My Work");
            settle(400);
            save_dialog(&w, &dialog.dialog, &out, &format!("new-profile-{theme}"));
            dialog.dialog.close();
            let trash = w.open_trash_dialog().unwrap();
            settle(400);
            save_dialog(&w, &trash.dialog, &out, &format!("trash-{theme}"));
            trash.dialog.close();
            w.window.destroy();

            let w = window(&full, true, (400, 720));
            save(&w, &out, &format!("narrow-list-{theme}"));
            w.select("UKR");
            settle(500);
            save(&w, &out, &format!("narrow-detail-{theme}"));
            w.window.destroy();

            let w = window(&empty, true, (1100, 720));
            save(&w, &out, &format!("empty-{theme}"));
            w.window.destroy();

            let w = window(&empty, false, (1100, 720));
            save(&w, &out, &format!("install-{theme}"));
            w.window.destroy();
        }

        scheme(false);
        let w = window(&full, true, (1440, 900));
        save(&w, &out, "large-light");
        w.window.destroy();
        app.quit();
    });
    app.run_with_args::<&str>(&[]);
}
