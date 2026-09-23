//! Create, delete, repair and launch profiles.

use crate::{launcher, names, paths::Paths, procs, profiles};
use std::fs;
use std::io;
use std::path::Path;

/// Moves a path to the Trash. Real: gio. Tests: move into a temp dir.
pub trait Trash {
    fn trash(&self, path: &Path) -> Result<(), String>;
}

/// Starts Signal for a profile. Real: `setsid -f`. Tests: record the call.
pub trait Launch {
    fn launch(&self, paths: &Paths, name: &str) -> io::Result<()>;
    /// Starts the Signal snap's own profile (no `--user-data-dir`).
    fn launch_default(&self, paths: &Paths) -> io::Result<()>;
}

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("“{input}” isn't a valid profile name")]
    Invalid {
        input: String,
        suggestion: Option<String>,
    },
    #[error("A profile named “{0}” already exists")]
    Exists(String),
    #[error("Could not create the profile: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error("There is no profile named “{0}”")]
    NotFound(String),
    #[error("Quit Signal ({0}) first. Closing its window can leave it running in the tray.")]
    Running(String),
    #[error("Could not move “{name}” to the Trash: {reason}")]
    Trash { name: String, reason: String },
}

#[derive(Debug, Default, PartialEq)]
pub struct RepairReport {
    pub updated: Vec<String>,
    pub created: Vec<String>,
    /// (profile, launcher file name) left alone because it's hand-made.
    pub skipped: Vec<(String, String)>,
}

pub struct Store {
    pub paths: Paths,
    trash: Box<dyn Trash>,
    launcher: Box<dyn Launch>,
}

impl Store {
    pub fn new(paths: Paths, trash: Box<dyn Trash>, launcher: Box<dyn Launch>) -> Self {
        Self {
            paths,
            trash,
            launcher,
        }
    }

    pub fn load(&self) -> io::Result<Vec<profiles::Profile>> {
        profiles::load_all(&self.paths)
    }

    /// Validates (trimmed input), refuses duplicates ignoring case, creates
    /// the data directory and launcher. Returns the final name.
    pub fn create(&self, input: &str) -> Result<String, CreateError> {
        let name = input.trim();
        if !names::is_valid(name) {
            return Err(CreateError::Invalid {
                input: name.to_string(),
                suggestion: names::suggest(name),
            });
        }
        if let Some(existing) = profiles::existing_ignoring_case(&self.paths, name) {
            return Err(CreateError::Exists(existing));
        }
        fs::create_dir_all(&self.paths.signal_base)?;
        let dir = self.paths.profile_dir(name);
        match fs::create_dir(&dir) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return Err(CreateError::Exists(name.to_string()));
            }
            other => other?,
        }
        if let Err(e) = launcher::write(&self.paths, name) {
            // The directory is new and empty, so removing it loses nothing and
            // lets the user simply try again.
            let _ = fs::remove_dir(&dir);
            return Err(e.into());
        }
        Ok(name.to_string())
    }

    /// Trashes launchers first, then data: if the second step fails, what's
    /// left is data without a launcher (fixable with Repair), never a launcher
    /// that would start Signal on a missing directory.
    pub fn delete(&self, name: &str) -> Result<(), DeleteError> {
        let dir = self.paths.profile_dir(name);
        if !names::is_valid(name) || !dir.is_dir() {
            return Err(DeleteError::NotFound(name.to_string()));
        }
        if procs::running_data_dirs(&self.paths.proc_root).contains(&dir) {
            return Err(DeleteError::Running(name.to_string()));
        }
        let trash_err = |reason| DeleteError::Trash {
            name: name.to_string(),
            reason,
        };
        let launchers = launcher::find(&self.paths, name).map_err(|e| trash_err(e.to_string()))?;
        for path in launchers.iter().map(AsRef::as_ref).chain([dir.as_path()]) {
            self.trash.trash(path).map_err(trash_err)?;
        }
        Ok(())
    }

    /// Rewrites this tool's launchers in the current format and creates missing
    /// ones. Hand-made launchers are left alone.
    pub fn repair(&self) -> io::Result<RepairReport> {
        let mut report = RepairReport::default();
        for name in profiles::list_names(&self.paths)? {
            if self.paths.own_launcher(&name).exists() {
                launcher::write(&self.paths, &name)?;
                report.updated.push(name);
            } else if let Some(other) = launcher::find(&self.paths, &name)?.first() {
                let file = other
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                report.skipped.push((name, file));
            } else {
                launcher::write(&self.paths, &name)?;
                report.created.push(name);
            }
        }
        Ok(report)
    }

    pub fn launch(&self, name: &str) -> io::Result<()> {
        if !names::is_valid(name) || !self.paths.profile_dir(name).is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no profile named {name:?}"),
            ));
        }
        self.launcher.launch(&self.paths, name)
    }

    /// Starts (or brings forward) the default Signal.
    pub fn launch_default(&self) -> io::Result<()> {
        self.launcher.launch_default(&self.paths)
    }
}
