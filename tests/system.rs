//! Exercises the real adapters. gio reads HOME and XDG_DATA_HOME once per
//! process, so this file is its own test binary with a single test that sets
//! them before any gio call.

use multisignal::paths::Paths;
use multisignal::store::{Store, Trash};
use multisignal::system::{GioTrash, SetsidLauncher};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

#[test]
fn real_adapters_trash_and_launch() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let data = home.join(".local/share");
    std::fs::create_dir_all(&data).unwrap();
    // SAFETY: the only test in this binary, and no other thread is running.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_DATA_HOME", &data);
    }

    // gio trash moves into $XDG_DATA_HOME/Trash/files.
    let victim = home.join("Signal/Old");
    std::fs::create_dir_all(&victim).unwrap();
    GioTrash.trash(&victim).unwrap();
    assert!(!victim.exists());
    assert!(data.join("Trash/files/Old").is_dir());

    // The setsid launcher runs the configured binary with the profile dir.
    let log = tmp.path().join("launched.log");
    let stub = tmp.path().join("signal-desktop");
    std::fs::write(
        &stub,
        format!(
            "#!/bin/sh\necho \"$BAMF_DESKTOP_FILE_HINT $@\" > {}.part && mv {0}.part {0}\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut paths = Paths::for_home(&home, Some(&data));
    paths.signal_bin = stub;
    let store = Store::new(paths.clone(), Box::new(GioTrash), Box::new(SetsidLauncher));
    store.create("Work").unwrap();
    store.launch("Work").unwrap();
    let line = (0..100)
        .find_map(|_| {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::read_to_string(&log).ok()
        })
        .expect("stub ran");
    assert_eq!(
        line.trim(),
        format!(
            "{} --user-data-dir={}",
            multisignal::paths::SNAP_DESKTOP_HINT,
            paths.profile_dir("Work").display()
        )
    );

    // The default Signal starts with no profile folder argument.
    std::fs::remove_file(&log).unwrap();
    store.launch_default().unwrap();
    let line = (0..100)
        .find_map(|_| {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::read_to_string(&log).ok()
        })
        .expect("stub ran for the default Signal");
    assert_eq!(line.trim(), multisignal::paths::SNAP_DESKTOP_HINT);

    // A Signal link is passed on after the profile folder.
    std::fs::remove_file(&log).unwrap();
    store.open_link("Work", "signalcaptcha://token").unwrap();
    let line = (0..100)
        .find_map(|_| {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::read_to_string(&log).ok()
        })
        .expect("stub ran with the link");
    assert_eq!(
        line.trim(),
        format!(
            "{} --user-data-dir={} signalcaptcha://token",
            multisignal::paths::SNAP_DESKTOP_HINT,
            paths.profile_dir("Work").display()
        )
    );

    // And delete goes through gio for real: data and launcher.
    store.delete("Work").unwrap();
    assert!(data.join("Trash/files/Work").is_dir());
    assert!(data.join("Trash/files/Signal-Work.desktop").is_file());
    assert!(!paths.own_launcher("Work").exists());
}
