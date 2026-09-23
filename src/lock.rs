//! Locking the default Signal: a per-user override of the snap's menu entry
//! with `Hidden=true`, so the launcher no longer offers it and it no longer
//! claims Signal links. Only overrides carrying our marker are ever removed.

use crate::paths::Paths;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// The snap's desktop-file ID; a file with this name in the user's
/// applications folder overrides the snap's own entry.
pub const SNAP_ENTRY_ID: &str = "signal-desktop_signal-desktop.desktop";
const MARKER: &str = "X-MultiSignal-Lock=true";
const OVERRIDE: &str = "[Desktop Entry]
Type=Application
Name=Signal
Comment=Hidden by Signal Profiles (the default Signal is locked)
Exec=/snap/bin/signal-desktop %U
Hidden=true
X-MultiSignal-Lock=true
";

pub fn override_path(paths: &Paths) -> PathBuf {
    paths.applications.join(SNAP_ENTRY_ID)
}

/// Whether the snap's Signal entry is hidden for this user.
pub fn is_locked(paths: &Paths) -> bool {
    read(paths).is_some_and(|text| text.lines().any(|l| l.trim() == "Hidden=true"))
}

/// Hides the snap's Signal entry. Refuses to replace an override someone else
/// made (unless it already hides the entry).
pub fn lock(paths: &Paths) -> io::Result<()> {
    match read(paths) {
        Some(text) if is_ours(&text) => return Ok(()),
        Some(_) if is_locked(paths) => return Ok(()),
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} was made by hand; Signal Profiles leaves it alone",
                    override_path(paths).display()
                ),
            ));
        }
        None => {}
    }
    fs::create_dir_all(&paths.applications)?;
    let target = override_path(paths);
    let tmp = paths
        .applications
        .join(format!(".{SNAP_ENTRY_ID}.{}.tmp", std::process::id()));
    let written = fs::write(&tmp, OVERRIDE)
        .and_then(|()| fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644)))
        .and_then(|()| fs::rename(&tmp, &target));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// Removes our override. A hand-made one that hides the entry can't be
/// undone here; one that doesn't is simply left alone.
pub fn unlock(paths: &Paths) -> io::Result<()> {
    match read(paths) {
        Some(text) if is_ours(&text) => fs::remove_file(override_path(paths)),
        Some(_) if is_locked(paths) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{} hides Signal but wasn't made by Signal Profiles; remove it by hand",
                override_path(paths).display()
            ),
        )),
        _ => Ok(()),
    }
}

fn read(paths: &Paths) -> Option<String> {
    fs::read_to_string(override_path(paths)).ok()
}

fn is_ours(text: &str) -> bool {
    text.lines().any(|l| l.trim() == MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn paths(dir: &Path) -> Paths {
        Paths::for_home(dir, None)
    }

    #[test]
    fn lock_hides_the_snap_entry_and_unlock_restores_it() {
        let tmp = tempfile::tempdir().unwrap();
        let p = paths(tmp.path());
        assert!(!is_locked(&p));
        lock(&p).unwrap();
        assert!(is_locked(&p));
        let text = fs::read_to_string(override_path(&p)).unwrap();
        assert!(text.contains("Hidden=true"));
        let status = std::process::Command::new("desktop-file-validate")
            .arg(override_path(&p))
            .status()
            .unwrap();
        assert!(status.success());
        lock(&p).unwrap(); // locking twice is fine
        unlock(&p).unwrap();
        assert!(!is_locked(&p));
        assert!(!override_path(&p).exists());
        unlock(&p).unwrap(); // and so is unlocking twice
    }

    #[test]
    fn leaves_a_hand_made_override_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let p = paths(tmp.path());
        fs::create_dir_all(&p.applications).unwrap();
        let mine = "[Desktop Entry]\nType=Application\nName=My Signal\nExec=x\n";
        fs::write(override_path(&p), mine).unwrap();
        assert!(!is_locked(&p));
        assert!(lock(&p).is_err());
        assert!(unlock(&p).is_ok(), "nothing of ours to remove");
        assert_eq!(fs::read_to_string(override_path(&p)).unwrap(), mine);
    }
}
