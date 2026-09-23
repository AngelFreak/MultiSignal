//! The store against a temp HOME and a fake /proc: one test per behaviour of
//! MultiSignal.test.sh (see the parity table in the plan).

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
    for d in [
        &paths.signal_base,
        &paths.applications,
        &paths.proc_root,
        &trash,
    ] {
        std::fs::create_dir_all(d).unwrap();
    }
    let launched = Recorder::default();
    let store = Store::new(
        paths,
        Box::new(DirTrash(trash.clone())),
        Box::new(launched.clone()),
    );
    Fixture {
        _tmp: tmp,
        store,
        trash,
        launched,
    }
}

fn mark_running(f: &Fixture, name: &str) {
    let pid = f.store.paths.proc_root.join("4242");
    std::fs::create_dir_all(&pid).unwrap();
    let dir = f.store.paths.profile_dir(name);
    std::fs::write(
        pid.join("cmdline"),
        format!("/x/signal-desktop\0--user-data-dir={}\0", dir.display()),
    )
    .unwrap();
}

fn hand_made_launcher(f: &Fixture, file: &str, profile: &str) -> (PathBuf, String) {
    let path = f.store.paths.applications.join(file);
    let text = format!(
        "[Desktop Entry]\nName={profile}\nExec=env X=1 /snap/bin/signal-desktop --user-data-dir={} %U\nType=Application\n",
        f.store.paths.profile_dir(profile).display()
    );
    std::fs::write(&path, &text).unwrap();
    (path, text)
}

fn desktop_file_validate(path: &Path) -> bool {
    std::process::Command::new("desktop-file-validate")
        .arg(path)
        .status()
        .expect("desktop-file-validate is installed")
        .success()
}

#[test]
fn create_trims_and_writes_dir_and_launcher() {
    let f = fixture();
    assert_eq!(f.store.create("  Work  ").unwrap(), "Work");
    assert!(f.store.paths.profile_dir("Work").is_dir());
    assert!(desktop_file_validate(&f.store.paths.own_launcher("Work")));
    assert!(f.launched.0.borrow().is_empty(), "creating doesn't launch");
}

#[test]
fn create_rejects_bad_names_without_touching_disk() {
    let f = fixture();
    for bad in ["../evil", "My Profile", "a|b", ".", "", "   "] {
        assert!(
            matches!(f.store.create(bad), Err(CreateError::Invalid { .. })),
            "{bad:?}"
        );
    }
    assert_eq!(
        std::fs::read_dir(&f.store.paths.signal_base)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        std::fs::read_dir(&f.store.paths.applications)
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn create_suggests_a_fixed_name() {
    let f = fixture();
    match f.store.create("My Work") {
        Err(CreateError::Invalid { suggestion, .. }) => {
            assert_eq!(suggestion.as_deref(), Some("My-Work"))
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn create_rejects_duplicates_ignoring_case() {
    let f = fixture();
    f.store.create("Work").unwrap();
    assert!(matches!(f.store.create("Work"), Err(CreateError::Exists(n)) if n == "Work"));
    assert!(matches!(f.store.create("work"), Err(CreateError::Exists(n)) if n == "Work"));
    assert!(!f.store.paths.profile_dir("work").exists());
}

#[test]
fn create_failure_leaves_no_half_made_profile() {
    let f = fixture();
    // The applications "directory" is a file, so the launcher can't be written.
    std::fs::remove_dir(&f.store.paths.applications).unwrap();
    std::fs::write(&f.store.paths.applications, "").unwrap();
    assert!(matches!(f.store.create("Work"), Err(CreateError::Io(_))));
    assert!(!f.store.paths.profile_dir("Work").exists());
}

#[test]
fn delete_trashes_launchers_and_data_only_for_that_profile() {
    let f = fixture();
    f.store.create("A").unwrap();
    std::fs::create_dir_all(f.store.paths.profile_dir("B")).unwrap();
    let (b_launcher, _) = hand_made_launcher(&f, "custom-b.desktop", "B");
    f.store.delete("A").unwrap();
    assert!(f.trash.join("A").is_dir());
    assert!(f.trash.join("Signal-A.desktop").is_file());
    assert!(!f.store.paths.profile_dir("A").exists());
    assert!(f.store.paths.profile_dir("B").is_dir());
    assert!(b_launcher.is_file());
}

#[test]
fn delete_trashes_hand_made_launchers_too() {
    let f = fixture();
    std::fs::create_dir_all(f.store.paths.profile_dir("Damon")).unwrap();
    hand_made_launcher(&f, "signal-desktop-damon.desktop", "Damon");
    f.store.delete("Damon").unwrap();
    assert!(f.trash.join("signal-desktop-damon.desktop").is_file());
}

#[test]
fn delete_never_touches_the_signal_base_dir() {
    let f = fixture();
    f.store.create("B").unwrap();
    std::fs::write(f.store.paths.applications.join("Signal-.desktop"), "").unwrap();
    for bad in ["", ".", "..", "../B"] {
        assert!(
            matches!(f.store.delete(bad), Err(DeleteError::NotFound(_))),
            "{bad:?}"
        );
    }
    assert!(f.store.paths.profile_dir("B").is_dir());
    assert_eq!(std::fs::read_dir(&f.trash).unwrap().count(), 0);
}

#[test]
fn delete_refuses_running_profile() {
    let f = fixture();
    f.store.create("A").unwrap();
    mark_running(&f, "A");
    assert!(matches!(f.store.delete("A"), Err(DeleteError::Running(_))));
    assert!(f.store.paths.profile_dir("A").is_dir());
    assert!(f.store.paths.own_launcher("A").is_file());
}

#[test]
fn delete_refuses_profile_running_as_the_real_snap_does() {
    // Signal rewrites /proc/<pid>/cmdline into one space-joined string.
    let f = fixture();
    f.store.create("A").unwrap();
    let pid = f.store.paths.proc_root.join("777");
    std::fs::create_dir_all(&pid).unwrap();
    std::fs::write(
        pid.join("cmdline"),
        format!(
            "/snap/signal-desktop/945/opt/Signal/signal-desktop --no-sandbox --user-data-dir={}\0\0",
            f.store.paths.profile_dir("A").display()
        ),
    )
    .unwrap();
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
    // A legacy launcher of ours in an old, invalid format.
    std::fs::write(
        p.own_launcher("UKR"),
        format!(
            "[Desktop Entry]\nExec=sh -c 'x --user-data-dir={} %U'\n",
            p.profile_dir("UKR").display()
        ),
    )
    .unwrap();
    let (damon, damon_text) = hand_made_launcher(&f, "signal-desktop-damon.desktop", "Damon");

    let report = f.store.repair().unwrap();
    assert_eq!(report.updated, ["UKR"]);
    assert_eq!(report.created, ["Personal"]);
    assert_eq!(
        report.skipped,
        [(
            "Damon".to_string(),
            "signal-desktop-damon.desktop".to_string()
        )]
    );
    assert!(desktop_file_validate(&p.own_launcher("UKR")));
    assert_eq!(std::fs::read_to_string(&damon).unwrap(), damon_text);
    assert!(!p.own_launcher("Damon").exists());

    // Running it again rewrites both; nothing is created any more.
    let again = f.store.repair().unwrap();
    assert_eq!(again.updated, ["Personal", "UKR"]);
    assert!(again.created.is_empty());
}

#[test]
fn launch_passes_the_profile_to_the_launcher() {
    let f = fixture();
    f.store.create("Work").unwrap();
    f.store.launch("Work").unwrap();
    assert_eq!(*f.launched.0.borrow(), ["Work"]);
    assert!(f.store.launch("Nope").is_err());
    assert!(f.store.launch("../home").is_err());
    assert_eq!(f.launched.0.borrow().len(), 1);
}
