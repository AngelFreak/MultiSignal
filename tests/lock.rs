//! Proves that locking hides the snap's Signal entry from GIO (which GNOME's
//! launcher uses) and unlocking brings it back. GIO reads XDG_DATA_HOME and
//! XDG_DATA_DIRS once per process, so this is its own test binary.

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib;
use multisignal::lock::{self, SNAP_ENTRY_ID};
use multisignal::paths::Paths;
use std::time::{Duration, Instant};

/// Waits (running GIO's file monitors) until the entry's presence among the
/// apps GIO lists, as a launcher would, matches `found`, or 10 s pass.
fn gio_sees_entry(found: bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        glib::MainContext::default().iteration(false);
        let listed = gio::AppInfo::all()
            .iter()
            .any(|app| app.id().is_some_and(|id| id == SNAP_ENTRY_ID));
        if listed == found {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn locking_hides_the_snap_entry_from_gio() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let data = home.join(".local/share");
    let system = tmp.path().join("system");
    std::fs::create_dir_all(data.join("applications")).unwrap();
    std::fs::create_dir_all(system.join("applications")).unwrap();
    std::fs::write(
        system.join("applications").join(SNAP_ENTRY_ID),
        "[Desktop Entry]\nType=Application\nName=Signal\nExec=/snap/bin/signal-desktop %U\n\
         MimeType=x-scheme-handler/sgnl;x-scheme-handler/signalcaptcha;\n",
    )
    .unwrap();
    // SAFETY: the only test in this binary, before any other thread exists.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_DATA_HOME", &data);
        std::env::set_var("XDG_DATA_DIRS", &system);
    }
    let paths = Paths::for_home(&home, Some(&data));

    assert!(
        gio_sees_entry(true),
        "the snap's entry is visible before locking"
    );
    lock::lock(&paths).unwrap();
    assert!(gio_sees_entry(false), "locking hides it");
    lock::unlock(&paths).unwrap();
    assert!(gio_sees_entry(true), "unlocking brings it back");
}
