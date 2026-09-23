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
    let dir = dir
        .to_str()
        .ok_or_else(|| invalid("profile path is not UTF-8"))?;
    if dir.contains(['"', '`', '$', '\\', '%', '\n']) {
        return Err(invalid(&format!(
            "cannot write a launcher for the path {dir:?}"
        )));
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
    let tmp = paths
        .applications
        .join(format!(".Signal-{name}.{}.tmp", std::process::id()));
    let result = write_file(&tmp, &text).and_then(|()| fs::rename(&tmp, &target));
    if result.is_err() {
        let _ = fs::remove_file(&tmp); // best effort; the original error matters
    }
    result.map(|()| target)
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
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let matches = text.lines().filter(|l| l.starts_with("Exec=")).any(|line| {
            line.match_indices(&needle).any(|(i, _)| {
                // The path must end here, so "A" doesn't match "AB".
                matches!(
                    line[i + needle.len()..].chars().next(),
                    None | Some('"' | '\'' | ' ')
                )
            })
        });
        if matches {
            found.push(path);
        }
    }
    Ok(found)
}

/// The `Name=` shown in the app menu: the untranslated key of the
/// `[Desktop Entry]` group.
pub fn display_name(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    for line in text.lines() {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry && let Some(name) = line.strip_prefix("Name=") {
            return Some(name.to_string());
        }
    }
    None
}

fn write_file(path: &Path, text: &str) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.set_permissions(fs::Permissions::from_mode(0o644))
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.to_string())
}

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
        let out = std::process::Command::new("desktop-file-validate")
            .arg(p.own_launcher("Work"))
            .output()
            .expect("desktop-file-validate is installed");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
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
        let mode = std::fs::metadata(p.own_launcher("Work"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o644);
    }

    #[test]
    fn write_leaves_no_temp_files_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let p = paths(tmp.path());
        write(&p, "Work").unwrap();
        write(&p, "Work").unwrap();
        let names: Vec<_> = std::fs::read_dir(&p.applications)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["Signal-Work.desktop"]);
    }

    #[test]
    fn finds_own_legacy_and_hand_made_launchers_only_for_that_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let p = paths(tmp.path());
        std::fs::create_dir_all(&p.applications).unwrap();
        let a = p.profile_dir("A");
        let b = p.profile_dir("AB"); // "A" is a prefix of it
        std::fs::write(
            p.applications.join("Signal-A.desktop"),
            format!(
                "[Desktop Entry]\nExec=sh -c 'x --user-data-dir={} %U'\n",
                a.display()
            ),
        )
        .unwrap();
        std::fs::write(
            p.applications.join("custom.desktop"),
            format!(
                "[Desktop Entry]\nExec=env X=1 x --user-data-dir={}\n",
                a.display()
            ),
        )
        .unwrap();
        std::fs::write(
            p.applications.join("other.desktop"),
            format!(
                "[Desktop Entry]\nExec=x --user-data-dir={} %U\n",
                b.display()
            ),
        )
        .unwrap();
        let mut found = find(&p, "A").unwrap();
        found.sort();
        assert_eq!(
            found,
            vec![
                p.applications.join("Signal-A.desktop"),
                p.applications.join("custom.desktop")
            ]
        );
    }

    #[test]
    fn finds_nothing_when_applications_dir_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find(&paths(tmp.path()), "A").unwrap().is_empty());
    }

    #[test]
    fn reads_the_menu_name() {
        let tmp = tempfile::tempdir().unwrap();
        let p = paths(tmp.path());
        let ours = write(&p, "Work").unwrap();
        assert_eq!(display_name(&ours).as_deref(), Some("Signal (Work)"));
        let other = tmp.path().join("x.desktop");
        std::fs::write(
            &other,
            "[Desktop Action new]\nName=Other\n[Desktop Entry]\nName[de]=Arbeit\nName=Damon\n",
        )
        .unwrap();
        assert_eq!(display_name(&other).as_deref(), Some("Damon"));
        assert_eq!(display_name(&tmp.path().join("missing.desktop")), None);
    }

    #[test]
    fn refuses_paths_that_cannot_be_quoted_safely() {
        let p = Paths::for_home(Path::new("/home/we$ird"), None);
        assert!(render(&p, "Work").is_err());
    }
}
