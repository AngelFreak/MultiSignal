//! Which profiles have a running Signal instance.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Data directories of running Signal processes: any process whose argv[0]
/// is a `signal-desktop` binary and that has a `--user-data-dir=` argument.
pub fn running_data_dirs(proc_root: &Path) -> HashSet<PathBuf> {
    let Ok(entries) = fs::read_dir(proc_root) else {
        return HashSet::new();
    };
    entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .as_encoded_bytes()
                .iter()
                .all(u8::is_ascii_digit)
        })
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
        fake_proc(
            tmp.path(),
            10,
            &[
                "/snap/signal-desktop/945/opt/Signal/signal-desktop",
                "--user-data-dir=/h/Signal/UKR",
            ],
        );
        fake_proc(
            tmp.path(),
            11,
            &[
                "/opt/google/chrome/chrome",
                "--user-data-dir=/h/Signal/Fake",
            ],
        );
        fake_proc(
            tmp.path(),
            12,
            &[
                "bash",
                "-c",
                "echo signal-desktop --user-data-dir=/h/Signal/Shell",
            ],
        );
        std::fs::create_dir_all(tmp.path().join("self")).unwrap(); // non-numeric entries are skipped
        let running = running_data_dirs(tmp.path());
        assert!(running.contains(Path::new("/h/Signal/UKR")));
        assert!(
            !running.contains(Path::new("/h/Signal/Fake")),
            "not a Signal process"
        );
        assert!(
            !running.contains(Path::new("/h/Signal/Shell")),
            "text inside another argument"
        );
    }

    #[test]
    fn missing_proc_root_means_nothing_runs() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(running_data_dirs(&tmp.path().join("nope")).is_empty());
    }

    #[test]
    fn real_proc_does_not_panic() {
        let _ = running_data_dirs(Path::new("/proc"));
    }
}
