//! Real side effects: gio trash, `setsid -f` launching, snap via pkexec.

use crate::paths::{Paths, SNAP_DESKTOP_HINT};
use crate::store::{Launch, Trash};
use gtk::gio;
use gtk::gio::prelude::*;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

/// Moves files to the desktop Trash, so they can be restored from Files.
pub struct GioTrash;

impl Trash for GioTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        gio::File::for_path(path)
            .trash(gio::Cancellable::NONE)
            .map_err(|e| e.to_string())
    }
}

pub struct SetsidLauncher;

impl Launch for SetsidLauncher {
    fn launch(&self, paths: &Paths, name: &str, link: Option<&str>) -> io::Result<()> {
        let data_dir = format!("--user-data-dir={}", paths.profile_dir(name).display());
        let args: Vec<String> = std::iter::once(data_dir)
            .chain(link.map(String::from))
            .collect();
        detached(paths, &args)
    }

    fn launch_default(&self, paths: &Paths, link: Option<&str>) -> io::Result<()> {
        let args: Vec<String> = link.map(String::from).into_iter().collect();
        detached(paths, &args)
    }
}

/// `setsid -f` detaches Signal so it outlives the manager and never becomes a
/// zombie child of it.
fn detached(paths: &Paths, args: &[String]) -> io::Result<()> {
    let status = Command::new("setsid")
        .arg("-f")
        .arg(&paths.signal_bin)
        .args(args)
        .env("BAMF_DESKTOP_FILE_HINT", SNAP_DESKTOP_HINT)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("setsid exited with {status}")))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InstallOutcome {
    Installed,
    Cancelled,
    Failed(String),
}

/// Installs Signal Desktop. `Send + Sync` so the UI can run `install` on a
/// worker thread.
pub trait Installer: Send + Sync {
    fn is_installed(&self) -> bool;
    /// Blocking; call from `gio::spawn_blocking`.
    fn install(&self) -> InstallOutcome;
    /// "apt" / "Flatpak" if Signal is installed some other way.
    fn other_install(&self) -> Option<&'static str>;
}

pub struct SnapInstaller;

impl Installer for SnapInstaller {
    fn is_installed(&self) -> bool {
        succeeds(Command::new("snap").args(["list", "signal-desktop"]))
    }

    fn install(&self) -> InstallOutcome {
        let out = match Command::new("pkexec")
            .args(["snap", "install", "signal-desktop"])
            .stdin(Stdio::null())
            .output()
        {
            Ok(o) => o,
            Err(e) => return InstallOutcome::Failed(format!("Could not run pkexec: {e}")),
        };
        if self.is_installed() {
            return InstallOutcome::Installed;
        }
        match out.status.code() {
            // pkexec: 126 = authentication dialog dismissed, 127 = not authorised.
            Some(126 | 127) => InstallOutcome::Cancelled,
            _ => InstallOutcome::Failed(last_lines(&String::from_utf8_lossy(&out.stderr), 5)),
        }
    }

    fn other_install(&self) -> Option<&'static str> {
        if succeeds(Command::new("flatpak").args(["info", "org.signal.Signal"])) {
            Some("Flatpak")
        } else if Path::new("/opt/Signal/signal-desktop").exists() {
            Some("apt")
        } else {
            None
        }
    }
}

fn succeeds(cmd: &mut Command) -> bool {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The last `n` non-empty lines, for showing a failed command's output.
fn last_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::last_lines;

    #[test]
    fn keeps_the_last_non_empty_lines() {
        assert_eq!(last_lines("a\nb\n\nc\nd\n", 2), "c\nd");
        assert_eq!(last_lines("only\n", 5), "only");
        assert_eq!(last_lines("", 5), "");
    }
}
