//! Profiles are the directories in `~/Signal`, not the launchers, so profiles
//! with a missing or hand-made launcher are still found.

use crate::{launcher, names, paths::Paths, procs};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The name shown for the snap's own profile. Not a valid profile name, so it
/// never collides with a folder in `~/Signal`, and delete/repair refuse it.
pub const DEFAULT_NAME: &str = "Signal (default)";

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// The key: the folder name in `~/Signal`, or `DEFAULT_NAME`.
    pub name: String,
    /// What the UI shows; the folder name, or the user's name for the default.
    pub title: String,
    pub dir: PathBuf,
    pub size_bytes: u64,
    pub running: bool,
    pub launchers: Vec<PathBuf>,
    /// The Signal snap's own profile (opened from the normal "Signal" entry),
    /// shown for completeness; it is never created, deleted or repaired here.
    pub is_default: bool,
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
    list_names(paths)
        .ok()?
        .into_iter()
        .find(|n| n.eq_ignore_ascii_case(name))
}

/// The default Signal first (when the snap has data), then every profile in
/// `~/Signal`.
pub fn load_all(paths: &Paths) -> io::Result<Vec<Profile>> {
    let running = procs::running(&paths.proc_root);
    let mut profiles = Vec::new();
    if paths.default_data_dir.is_dir() {
        profiles.push(Profile {
            name: DEFAULT_NAME.to_string(),
            title: DEFAULT_NAME.to_string(),
            dir: paths.default_data_dir.clone(),
            size_bytes: dir_size(&paths.default_data_dir),
            running: running.default,
            launchers: Vec::new(),
            is_default: true,
        });
    }
    for name in list_names(paths)? {
        let dir = paths.profile_dir(&name);
        profiles.push(Profile {
            size_bytes: dir_size(&dir),
            running: running.data_dirs.contains(&dir),
            launchers: launcher::find(paths, &name)?,
            is_default: false,
            title: name.clone(),
            dir,
            name,
        });
    }
    Ok(profiles)
}

/// Total size of regular files, not following symlinks. Unreadable entries
/// count as 0 rather than failing the whole list.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

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
        std::fs::write(
            pid.join("cmdline"),
            format!("/x/signal-desktop\0--user-data-dir={}\0", dir.display()),
        )
        .unwrap();

        let profiles = load_all(&p).unwrap();
        assert_eq!(profiles.len(), 1);
        let ukr = &profiles[0];
        assert_eq!(ukr.size_bytes, 5000);
        assert!(ukr.running);
        assert_eq!(ukr.launchers, vec![p.own_launcher("UKR")]);
    }

    #[test]
    fn lists_the_default_signal_first_when_the_snap_has_data() {
        let (_t, p) = setup();
        std::fs::create_dir_all(p.signal_base.join("Alpha")).unwrap();
        std::fs::create_dir_all(&p.default_data_dir).unwrap();
        std::fs::write(p.default_data_dir.join("db"), vec![0u8; 700]).unwrap();
        let pid = p.proc_root.join("50");
        std::fs::create_dir_all(&pid).unwrap();
        std::fs::write(pid.join("cmdline"), "/snap/bin/signal-desktop\0").unwrap();

        let profiles = load_all(&p).unwrap();
        let names: Vec<_> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, [DEFAULT_NAME, "Alpha"]);
        let default = &profiles[0];
        assert!(default.is_default);
        assert_eq!(default.title, DEFAULT_NAME);
        assert_eq!(profiles[1].title, "Alpha");
        assert!(default.running);
        assert_eq!(default.size_bytes, 700);
        assert_eq!(default.dir, p.default_data_dir);
        assert!(!profiles[1].is_default);
    }

    #[test]
    fn no_default_entry_without_snap_data() {
        let (_t, p) = setup();
        std::fs::create_dir_all(p.signal_base.join("Alpha")).unwrap();
        assert!(load_all(&p).unwrap().iter().all(|p| !p.is_default));
    }

    #[test]
    fn the_default_name_can_never_be_a_profile_folder() {
        assert!(!names::is_valid(DEFAULT_NAME));
    }

    #[test]
    fn size_does_not_follow_symlinks() {
        let (t, p) = setup();
        let big = t.path().join("elsewhere");
        std::fs::create_dir_all(&big).unwrap();
        std::fs::write(big.join("blob"), vec![0u8; 9000]).unwrap();
        let dir = p.profile_dir("A");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("db"), vec![0u8; 100]).unwrap();
        std::os::unix::fs::symlink(&big, dir.join("link")).unwrap();
        assert_eq!(dir_size(&dir), 100);
    }
}
