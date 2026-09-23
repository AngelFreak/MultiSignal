//! Builds the real window against fixture profiles and fake side effects,
//! then inspects the widget tree. GTK must run on the main thread, so this is
//! one binary with its own `main` (harness = false). Needs a display; the
//! desktop session provides one.

use adw::prelude::*;
use multisignal::paths::Paths;
use multisignal::store::{Launch, Store, Trash};
use multisignal::system::{InstallOutcome, Installer};
use multisignal::ui::{self, Deps};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Moves into a directory next to the fixture, like the real Trash would.
struct DirTrash(PathBuf);
impl Trash for DirTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        std::fs::create_dir_all(&self.0).unwrap();
        std::fs::rename(path, self.0.join(path.file_name().unwrap())).map_err(|e| e.to_string())
    }
}

#[derive(Clone, Default)]
struct Recorder(Rc<RefCell<Vec<String>>>);
impl Launch for Recorder {
    fn launch(&self, _: &Paths, name: &str) -> std::io::Result<()> {
        self.0.borrow_mut().push(name.to_string());
        Ok(())
    }
    fn launch_default(&self, _: &Paths) -> std::io::Result<()> {
        self.0.borrow_mut().push("(default)".to_string());
        Ok(())
    }
}

/// "Installs" by flipping a flag, as a successful snap install would.
struct FakeInstaller(Arc<AtomicBool>);
impl Installer for FakeInstaller {
    fn is_installed(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    fn install(&self) -> InstallOutcome {
        self.0.store(true, Ordering::SeqCst);
        InstallOutcome::Installed
    }
    fn other_install(&self) -> Option<&'static str> {
        None
    }
}

struct Fixture {
    deps: Deps,
    paths: Paths,
    launched: Recorder,
}

fn fixture(home: &Path, installed: bool, profiles: &[&str], running: &[&str]) -> Fixture {
    let mut paths = Paths::for_home(home, None);
    paths.proc_root = home.join("proc");
    std::fs::create_dir_all(&paths.proc_root).unwrap();
    for p in profiles {
        std::fs::create_dir_all(paths.profile_dir(p)).unwrap();
    }
    for (i, r) in running.iter().enumerate() {
        let pid = paths.proc_root.join((100 + i).to_string());
        std::fs::create_dir_all(&pid).unwrap();
        std::fs::write(
            pid.join("cmdline"),
            format!(
                "/x/signal-desktop\0--user-data-dir={}\0",
                paths.profile_dir(r).display()
            ),
        )
        .unwrap();
    }
    let launched = Recorder::default();
    let deps = Deps {
        store: Store::new(
            paths.clone(),
            Box::new(DirTrash(home.join("trash"))),
            Box::new(launched.clone()),
        ),
        installer: Arc::new(FakeInstaller(Arc::new(AtomicBool::new(installed)))),
    };
    Fixture {
        deps,
        paths,
        launched,
    }
}

fn deps(home: &Path, installed: bool, profiles: &[&str], running: &[&str]) -> Deps {
    fixture(home, installed, profiles, running).deps
}

fn check(name: &str, ok: bool) {
    println!("{} {name}", if ok { "ok  " } else { "FAIL" });
    if !ok {
        std::process::exit(1);
    }
}

fn main() {
    adw::init().expect("a display is available");
    let tmp = tempfile::tempdir().unwrap();

    // Task 10: window shell, states, split view.
    let w = ui::build_window(deps(&tmp.path().join("a"), false, &[], &[]));
    check(
        "not installed → install page",
        w.visible_page() == "install",
    );

    let w = ui::build_window(deps(&tmp.path().join("b"), true, &[], &[]));
    check("no profiles → empty page", w.visible_page() == "empty");

    let w = ui::build_window(deps(
        &tmp.path().join("c"),
        true,
        &["UKR", "Damon", "Personal"],
        &["UKR"],
    ));
    check("profiles → app page", w.visible_page() == "app");
    check(
        "sidebar sorted by name",
        w.sidebar_names() == ["Damon", "Personal", "UKR"],
    );
    check(
        "first profile selected on start",
        w.selected().as_deref() == Some("Damon"),
    );
    check(
        "footer summary",
        w.footer() == "3 profiles · 1 running · Empty",
    );
    check(
        "default size is desktop-sized",
        w.window.default_width() == 1100 && w.window.default_height() == 720,
    );

    w.set_collapsed_for_test(true);
    w.select("UKR");
    check(
        "collapsed: selecting shows the detail page",
        w.showing_content(),
    );
    w.go_back_for_test();
    check("collapsed: back returns to the list", !w.showing_content());
    w.set_collapsed_for_test(false);

    // The stylesheets parse: GTK CSS is not web CSS and only warns at runtime.
    let errors = ui::css_errors();
    check(&format!("CSS parses cleanly {errors:?}"), errors.is_empty());

    // Task 11: sidebar rows, detail pane, actions.
    let home = tmp.path().join("e");
    let f = fixture(&home, true, &["UKR", "Damon", "Personal"], &["UKR"]);
    let paths = f.paths.clone();
    let launched = f.launched.clone();
    multisignal::launcher::write(&paths, "UKR").unwrap();
    std::fs::write(
        paths.applications.join("signal-desktop-damon.desktop"),
        format!(
            "[Desktop Entry]\nName=Damon\nExec=env X=1 /snap/bin/signal-desktop --user-data-dir={} %U\n",
            paths.profile_dir("Damon").display()
        ),
    )
    .unwrap();
    let w = ui::build_window(f.deps);
    check(
        "running row says Running",
        w.sidebar_secondary("UKR").starts_with("Running"),
    );
    check(
        "missing launcher row warns",
        w.sidebar_secondary("Personal")
            .starts_with("No app menu entry"),
    );
    check(
        "stopped row shows only the size",
        w.sidebar_secondary("Damon") == "Empty",
    );
    w.select("UKR");
    check("detail title", w.detail_title() == "UKR");
    check(
        "running: primary button says Show Signal",
        w.detail_primary_label() == "Show Signal",
    );
    check(
        "running: trash disabled",
        !w.action_enabled("win.trash-selected"),
    );
    check(
        "running: context menu says Show Signal",
        w.context_menu_labels()[0] == "Show Signal",
    );
    w.select("Damon");
    check(
        "stopped: primary says Open Signal",
        w.detail_primary_label() == "Open Signal",
    );
    check(
        "stopped: trash enabled",
        w.action_enabled("win.trash-selected"),
    );
    check(
        "hand-made launcher is labelled",
        w.detail_value("Launcher file") == "signal-desktop-damon.desktop · made by hand",
    );
    check(
        "menu entry shows the launcher's own name",
        w.detail_value("Menu entry") == "Damon",
    );
    check(
        "context menu has open, show, trash",
        w.context_menu_labels() == ["Open Signal", "Show in Files", "Move to Trash…"],
    );
    w.activate_action_for_test("open-selected");
    check(
        "Open Signal launches the selected profile",
        *launched.0.borrow() == ["Damon"],
    );
    w.select("Personal");
    check("missing launcher offers Repair", w.detail_has_repair());
    w.activate_action_for_test("repair");
    check(
        "Repair writes the launcher",
        paths.own_launcher("Personal").is_file(),
    );
    check(
        "after Repair the row no longer warns",
        w.sidebar_secondary("Personal") == "Empty",
    );
    check("after Repair no Repair button", !w.detail_has_repair());
    check(
        "selection survives a reload",
        w.selected().as_deref() == Some("Personal"),
    );

    // Task 12: dialogs.
    use multisignal::ui::create_dialog::{Validation, validate};
    let d = deps(&tmp.path().join("f"), true, &["Work"], &[]);
    check("valid name", validate(&d.store, "Travel") == Validation::Ok);
    check(
        "space → suggestion",
        validate(&d.store, "My Work")
            == Validation::Invalid {
                suggestion: Some("My-Work".into()),
            },
    );
    check(
        "case duplicate",
        validate(&d.store, "work") == Validation::Exists("Work".into()),
    );
    check(
        "empty shows the hint, not an error",
        validate(&d.store, "") == Validation::Empty,
    );
    check(
        "blank shows the hint, not an error",
        validate(&d.store, "   ") == Validation::Empty,
    );

    let f = fixture(
        &tmp.path().join("g"),
        true,
        &["Damon", "Personal", "Work"],
        &[],
    );
    let paths = f.paths.clone();
    let w = ui::build_window(f.deps);
    let dialog = w.open_create_dialog();
    dialog.entry.set_text("My Work");
    check(
        "invalid name disables Create",
        !dialog.create.is_sensitive(),
    );
    check(
        "invalid name offers the fix",
        dialog.suggestion.get_visible(),
    );
    dialog.suggestion.emit_clicked();
    check("fix-it fills the field", dialog.entry.text() == "My-Work");
    check("fixed name enables Create", dialog.create.is_sensitive());
    dialog.entry.set_text("Travel");
    dialog.create.emit_clicked();
    check(
        "create writes the profile",
        paths.own_launcher("Travel").is_file(),
    );
    check(
        "new profile selected",
        w.selected().as_deref() == Some("Travel"),
    );
    check(
        "new profile listed",
        w.sidebar_names() == ["Damon", "Personal", "Travel", "Work"],
    );

    w.select("Personal");
    let trash = w
        .open_trash_dialog()
        .expect("a stopped profile is selected");
    trash.cancel.emit_clicked();
    check(
        "cancel keeps the profile",
        paths.profile_dir("Personal").is_dir(),
    );
    let trash = w
        .open_trash_dialog()
        .expect("a stopped profile is selected");
    check(
        "trash dialog names the profile",
        trash.title.text() == "Move “Personal” to the Trash?",
    );
    trash.confirm.emit_clicked();
    check(
        "confirm trashes the data",
        !paths.profile_dir("Personal").exists(),
    );
    check(
        "confirm trashes into the Trash",
        tmp.path().join("g/trash/Personal").is_dir(),
    );
    check(
        "neighbour selected after delete",
        w.selected().as_deref() == Some("Travel"),
    );
    check(
        "deleted profile unlisted",
        w.sidebar_names() == ["Damon", "Travel", "Work"],
    );
    check(
        "Delete key opens the trash dialog",
        w.press_delete_for_test(),
    );

    // The snap's own (default) Signal: listed first, never trashed.
    use multisignal::profiles::DEFAULT_NAME;
    let f = fixture(&tmp.path().join("i"), true, &["Work"], &[]);
    let launched = f.launched.clone();
    std::fs::create_dir_all(&f.paths.default_data_dir).unwrap();
    let pid = f.paths.proc_root.join("300");
    std::fs::create_dir_all(&pid).unwrap();
    std::fs::write(
        pid.join("cmdline"),
        "/snap/signal-desktop/945/opt/Signal/signal-desktop --no-sandbox --disable-gpu\0",
    )
    .unwrap();
    let w = ui::build_window(f.deps);
    check(
        "default Signal listed first",
        w.sidebar_names() == [DEFAULT_NAME, "Work"],
    );
    check(
        "default Signal selected on start",
        w.selected().as_deref() == Some(DEFAULT_NAME),
    );
    check(
        "default Signal shows Running",
        w.sidebar_secondary(DEFAULT_NAME).starts_with("Running"),
    );
    check(
        "actions are enabled on start, before any click",
        w.action_enabled("win.open-selected"),
    );
    check(
        "default Signal: Show Signal",
        w.detail_primary_label() == "Show Signal",
    );
    check(
        "default Signal: menu entry is Signal",
        w.detail_value("Menu entry") == "Signal",
    );
    check(
        "default Signal: no trash",
        !w.action_enabled("win.trash-selected"),
    );
    check("default Signal: no repair", !w.action_enabled("win.repair"));
    check(
        "default Signal: context menu has no trash",
        w.context_menu_labels() == ["Show Signal", "Show in Files"],
    );
    w.activate_action_for_test("open-selected");
    check(
        "default Signal: open starts the default",
        *launched.0.borrow() == ["(default)"],
    );
    w.select("Work");
    check(
        "a profile after it can still be trashed",
        w.action_enabled("win.trash-selected"),
    );

    // Task 13: install page.
    let w = ui::build_window(deps(&tmp.path().join("h"), false, &[], &[]));
    check("install page shown", w.visible_page() == "install");
    w.run_install_for_test();
    check("after install → empty page", w.visible_page() == "empty");

    println!("ui: all checks passed");
}
