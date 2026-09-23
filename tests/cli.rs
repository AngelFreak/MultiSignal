//! The command-line tool, `multisignal.sh`, run in a sandbox HOME with a fake
//! /proc and a stub Signal. It shares the on-disk format with the app: the
//! launchers it writes must match the app's byte for byte.

use multisignal::paths::Paths;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/multisignal.sh");

struct Sandbox {
    _tmp: tempfile::TempDir,
    home: PathBuf,
    paths: Paths,
    launched: PathBuf,
}

fn sandbox() -> Sandbox {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let data = home.join(".local/share");
    let mut paths = Paths::for_home(&home, Some(&data));
    paths.proc_root = tmp.path().join("proc");
    paths.signal_bin = tmp.path().join("signal-desktop");
    let launched = tmp.path().join("launched");
    for dir in [&paths.signal_base, &paths.applications, &paths.proc_root] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(
        &paths.signal_bin,
        format!("#!/bin/sh\necho \"$@\" > {}\n", launched.display()),
    )
    .unwrap();
    std::fs::set_permissions(&paths.signal_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    Sandbox {
        _tmp: tmp,
        home,
        paths,
        launched,
    }
}

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(sb: &Sandbox, args: &[&str]) -> Output {
    let out = Command::new("bash")
        .arg(SCRIPT)
        .args(args)
        .env("HOME", &sb.home)
        .env("XDG_DATA_HOME", sb.home.join(".local/share"))
        .env("MULTISIGNAL_SIGNAL_BIN", &sb.paths.signal_bin)
        .env("MULTISIGNAL_PROC", &sb.paths.proc_root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    Output {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn fake_running(sb: &Sandbox, pid: u32, cmdline: &str) {
    let dir = sb.paths.proc_root.join(pid.to_string());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("cmdline"), cmdline).unwrap();
}

fn line_for<'a>(stdout: &'a str, name: &str) -> &'a str {
    stdout
        .lines()
        .find(|l| l.split_whitespace().next() == Some(name))
        .unwrap_or_else(|| panic!("no line for {name} in:\n{stdout}"))
}

fn wait_for(path: &Path) -> String {
    (0..100)
        .find_map(|_| {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::read_to_string(path).ok()
        })
        .expect("the stub Signal ran")
}

#[test]
fn create_writes_the_same_launcher_as_the_app() {
    let sb = sandbox();
    let out = run(&sb, &["create", "  Work  "]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(sb.paths.profile_dir("Work").is_dir());
    let launcher = sb.paths.own_launcher("Work");
    assert_eq!(
        std::fs::read_to_string(&launcher).unwrap(),
        multisignal::launcher::render(&sb.paths, "Work").unwrap()
    );
    let mode = std::fs::metadata(&launcher).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o644);
}

#[test]
fn create_rejects_bad_and_taken_names() {
    let sb = sandbox();
    let out = run(&sb, &["create", "My Work"]);
    assert_ne!(out.code, 0);
    assert!(
        out.stderr.contains("My-Work"),
        "suggests a fix: {}",
        out.stderr
    );
    for bad in ["../evil", ".", "-rf", ""] {
        assert_ne!(run(&sb, &["create", bad]).code, 0, "{bad:?}");
    }
    assert_eq!(std::fs::read_dir(&sb.paths.signal_base).unwrap().count(), 0);

    assert_eq!(run(&sb, &["create", "Work"]).code, 0);
    let out = run(&sb, &["create", "work"]);
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("already exists"), "{}", out.stderr);
}

#[test]
fn list_shows_running_state_and_menu_entries() {
    let sb = sandbox();
    for name in ["Alpha", "Beta", "Gamma"] {
        assert_eq!(run(&sb, &["create", name]).code, 0);
    }
    std::fs::remove_file(sb.paths.own_launcher("Gamma")).unwrap();
    // Beta runs; its command line is rewritten as Signal (Chromium) does.
    fake_running(
        &sb,
        10,
        &format!(
            "/snap/signal-desktop/1/signal-desktop --no-sandbox --user-data-dir={}\0",
            sb.paths.profile_dir("Beta").display()
        ),
    );
    let out = run(&sb, &["list"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(line_for(&out.stdout, "Alpha").contains("Signal-Alpha.desktop"));
    assert!(line_for(&out.stdout, "Beta").contains("running"));
    assert!(!line_for(&out.stdout, "Alpha").contains("running"));
    assert!(line_for(&out.stdout, "Gamma").contains("missing"));
}

#[test]
fn open_starts_signal_with_the_profile_and_an_optional_link() {
    let sb = sandbox();
    assert_eq!(run(&sb, &["create", "Work"]).code, 0);
    assert_eq!(run(&sb, &["open", "Work", "signalcaptcha://token"]).code, 0);
    assert_eq!(
        wait_for(&sb.launched).trim(),
        format!(
            "--user-data-dir={} signalcaptcha://token",
            sb.paths.profile_dir("Work").display()
        )
    );
    assert_ne!(run(&sb, &["open", "Nope"]).code, 0);
    assert_ne!(run(&sb, &["open", "Work", "https://example.com"]).code, 0);
}

#[test]
fn delete_moves_the_launcher_and_data_to_the_trash() {
    let sb = sandbox();
    assert_eq!(run(&sb, &["create", "Old"]).code, 0);
    assert_eq!(run(&sb, &["create", "Keep"]).code, 0);

    let out = run(&sb, &["delete", "Old"]);
    assert_ne!(out.code, 0, "no terminal to confirm, and no --yes");
    assert!(sb.paths.profile_dir("Old").is_dir());

    let out = run(&sb, &["delete", "--yes", "Old"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let trash = sb.home.join(".local/share/Trash/files");
    assert!(trash.join("Old").is_dir());
    assert!(trash.join("Signal-Old.desktop").is_file());
    assert!(!sb.paths.profile_dir("Old").exists());
    assert!(sb.paths.profile_dir("Keep").is_dir());
}

#[test]
fn delete_refuses_a_running_profile() {
    let sb = sandbox();
    assert_eq!(run(&sb, &["create", "Busy"]).code, 0);
    fake_running(
        &sb,
        11,
        &format!(
            "/x/signal-desktop\0--user-data-dir={}\0",
            sb.paths.profile_dir("Busy").display()
        ),
    );
    let out = run(&sb, &["delete", "--yes", "Busy"]);
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("Quit Signal (Busy)"), "{}", out.stderr);
    assert!(sb.paths.profile_dir("Busy").is_dir());
}

#[test]
fn repair_updates_ours_creates_missing_and_skips_hand_made() {
    let sb = sandbox();
    let p = &sb.paths;
    for d in ["Mine", "Bare", "Hand"] {
        std::fs::create_dir_all(p.profile_dir(d)).unwrap();
    }
    std::fs::write(
        p.own_launcher("Mine"),
        "[Desktop Entry]\nExec=old --user-data-dir=x\n",
    )
    .unwrap();
    let hand = p.applications.join("hand.desktop");
    let hand_text = format!(
        "[Desktop Entry]\nName=Hand\nExec=x --user-data-dir={} %U\n",
        p.profile_dir("Hand").display()
    );
    std::fs::write(&hand, &hand_text).unwrap();

    let out = run(&sb, &["repair"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("Updated: Mine"), "{}", out.stdout);
    assert!(out.stdout.contains("Created: Bare"), "{}", out.stdout);
    assert!(out.stdout.contains("Hand"), "mentions the skipped one");
    for name in ["Mine", "Bare"] {
        assert_eq!(
            std::fs::read_to_string(p.own_launcher(name)).unwrap(),
            multisignal::launcher::render(p, name).unwrap()
        );
    }
    assert_eq!(std::fs::read_to_string(&hand).unwrap(), hand_text);
    assert!(!p.own_launcher("Hand").exists());
}

#[test]
fn help_and_unknown_commands() {
    let sb = sandbox();
    let out = run(&sb, &["help"]);
    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("Usage"));
    assert_eq!(run(&sb, &["frobnicate"]).code, 2);
    assert_eq!(run(&sb, &[]).code, 2);
}
