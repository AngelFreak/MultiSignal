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
        .filter_map(|raw| signal_data_dir(&raw))
        .collect()
}

const DATA_DIR_FLAG: &str = "--user-data-dir=";

/// The data directory of a Signal process, from its raw `/proc/<pid>/cmdline`.
fn signal_data_dir(raw: &[u8]) -> Option<PathBuf> {
    let args: Vec<String> = raw
        .split(|b| *b == 0)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8_lossy(a).into_owned())
        .collect();
    match args.as_slice() {
        // Chromium (and so Signal) rewrites its command line into a single
        // space-joined string. Arguments are then separated by " --", which
        // keeps paths with spaces intact.
        [joined] if joined.contains(" --") => {
            let argv0 = &joined[..joined.find(" --")?];
            let flag = format!(" {DATA_DIR_FLAG}");
            let value = &joined[joined.find(&flag)? + flag.len()..];
            let value = value.find(" --").map_or(value, |end| &value[..end]);
            is_signal(argv0).then(|| PathBuf::from(value))
        }
        [argv0, rest @ ..] if is_signal(argv0) => rest
            .iter()
            .find_map(|a| a.strip_prefix(DATA_DIR_FLAG))
            .map(PathBuf::from),
        _ => None,
    }
}

fn is_signal(argv0: &str) -> bool {
    Path::new(argv0)
        .file_name()
        .is_some_and(|n| n == "signal-desktop")
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

    /// Chromium rewrites its command line into one space-joined string
    /// (seen on the Signal snap), so there are no NULs between arguments.
    fn fake_rewritten_proc(root: &Path, pid: u32, cmdline: &str) {
        let dir = root.join(pid.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cmdline"), format!("{cmdline}\0\0\0")).unwrap();
    }

    #[test]
    fn finds_signal_with_a_rewritten_command_line() {
        let tmp = tempfile::tempdir().unwrap();
        fake_rewritten_proc(
            tmp.path(),
            20,
            "/snap/signal-desktop/945/opt/Signal/signal-desktop --no-sandbox --password-store=basic --disable-gpu --user-data-dir=/h/Signal/Damon",
        );
        fake_rewritten_proc(
            tmp.path(),
            21,
            "/snap/signal-desktop/945/opt/Signal/signal-desktop --type=renderer --user-data-dir=/home/my user/Signal/UKR --no-zygote",
        );
        fake_rewritten_proc(
            tmp.path(),
            22,
            "/opt/google/chrome/chrome --type=renderer --user-data-dir=/h/Signal/Chrome",
        );
        let running = running_data_dirs(tmp.path());
        assert!(running.contains(Path::new("/h/Signal/Damon")));
        assert!(
            running.contains(Path::new("/home/my user/Signal/UKR")),
            "a space in the path: {running:?}"
        );
        assert!(!running.contains(Path::new("/h/Signal/Chrome")));
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
