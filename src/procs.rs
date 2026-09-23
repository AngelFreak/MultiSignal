//! Which profiles have a running Signal instance.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// What is running right now.
#[derive(Debug, Default)]
pub struct Running {
    /// `--user-data-dir` of every Signal process (profiles in `~/Signal`).
    pub data_dirs: HashSet<PathBuf>,
    /// A Signal started without `--user-data-dir`: the snap's own default
    /// profile, opened from the normal "Signal" app menu entry.
    pub default: bool,
}

/// Scans `/proc` for Signal processes: any process whose argv[0] is a
/// `signal-desktop` binary.
pub fn running(proc_root: &Path) -> Running {
    let mut found = Running::default();
    let Ok(entries) = fs::read_dir(proc_root) else {
        return found;
    };
    let cmdlines = entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .as_encoded_bytes()
                .iter()
                .all(u8::is_ascii_digit)
        })
        .filter_map(|e| fs::read(e.path().join("cmdline")).ok()); // processes vanish; ignore
    for raw in cmdlines {
        match signal_instance(&raw) {
            Some(Instance::DataDir(dir)) => {
                found.data_dirs.insert(dir);
            }
            Some(Instance::Default) => found.default = true,
            None => {}
        }
    }
    found
}

/// Data directories of running Signal processes.
pub fn running_data_dirs(proc_root: &Path) -> HashSet<PathBuf> {
    running(proc_root).data_dirs
}

const DATA_DIR_FLAG: &str = "--user-data-dir=";

enum Instance {
    Default,
    DataDir(PathBuf),
}

/// Which Signal a process is, from its raw `/proc/<pid>/cmdline`.
fn signal_instance(raw: &[u8]) -> Option<Instance> {
    let args: Vec<String> = raw
        .split(|b| *b == 0)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8_lossy(a).into_owned())
        .collect();
    let (argv0, flags): (&str, Vec<String>) = match args.as_slice() {
        // Chromium (and so Signal) rewrites its command line into a single
        // space-joined string. Arguments are then separated by " --", which
        // keeps paths with spaces intact.
        [joined] if joined.contains(" --") => {
            let start = joined.find(" --")?;
            let flags = joined[start + 3..]
                .split(" --")
                .map(|f| format!("--{f}"))
                .collect();
            (&joined[..start], flags)
        }
        [argv0, rest @ ..] => (argv0.as_str(), rest.to_vec()),
        [] => return None,
    };
    if !is_signal(argv0) {
        return None;
    }
    if let Some(dir) = flags.iter().find_map(|f| f.strip_prefix(DATA_DIR_FLAG)) {
        Some(Instance::DataDir(PathBuf::from(dir)))
    } else if flags.iter().any(|f| f.starts_with("--type=")) {
        None // a helper (renderer, GPU, …) of some instance
    } else {
        Some(Instance::Default)
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
    fn a_main_process_without_a_data_dir_is_the_default_signal() {
        let tmp = tempfile::tempdir().unwrap();
        // As the snap starts it from the normal "Signal" entry (rewritten).
        fake_rewritten_proc(
            tmp.path(),
            30,
            "/snap/signal-desktop/945/opt/Signal/signal-desktop --no-sandbox --password-store=basic --disable-gpu",
        );
        assert!(running(tmp.path()).default);
    }

    #[test]
    fn a_bare_signal_command_is_the_default_signal() {
        let tmp = tempfile::tempdir().unwrap();
        fake_proc(tmp.path(), 31, &["/snap/bin/signal-desktop"]);
        assert!(running(tmp.path()).default);
    }

    #[test]
    fn helpers_and_profiles_are_not_the_default_signal() {
        let tmp = tempfile::tempdir().unwrap();
        fake_rewritten_proc(
            tmp.path(),
            32,
            "/snap/signal-desktop/945/opt/Signal/signal-desktop --type=renderer --no-zygote",
        );
        fake_proc(
            tmp.path(),
            33,
            &["/x/signal-desktop", "--user-data-dir=/h/Signal/A"],
        );
        let r = running(tmp.path());
        assert!(!r.default);
        assert!(r.data_dirs.contains(Path::new("/h/Signal/A")));
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
