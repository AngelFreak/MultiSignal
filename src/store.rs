//! Create, delete, repair and launch profiles.

use crate::{launcher, links, lock, names, paths::Paths, procs, profiles};
use std::fs;
use std::io;
use std::path::Path;

/// Moves a path to the Trash. Real: gio. Tests: move into a temp dir.
pub trait Trash {
    fn trash(&self, path: &Path) -> Result<(), String>;
}

/// Starts Signal for a profile. Real: `setsid -f`. Tests: record the call.
/// With a `link`, a Signal already running for that profile receives it.
pub trait Launch {
    fn launch(&self, paths: &Paths, name: &str, link: Option<&str>) -> io::Result<()>;
    /// Starts the Signal snap's own profile (no `--user-data-dir`).
    fn launch_default(&self, paths: &Paths, link: Option<&str>) -> io::Result<()>;
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

#[derive(Debug, thiserror::Error)]
pub enum AdoptError {
    #[error("The default Signal has no data to move")]
    NoDefault,
    #[error("Quit the default Signal first. Closing its window can leave it running in the tray.")]
    Running,
    #[error(transparent)]
    Name(#[from] CreateError),
    #[error("Could not move the default Signal: {0}")]
    Move(io::Error),
    #[error(
        "Moved to ~/Signal/{name}, but its app menu entry could not be written ({source}). Use Repair."
    )]
    Launcher { name: String, source: io::Error },
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
        let name = self.new_name(input)?;
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

    /// The trimmed name if it's valid and not taken (ignoring case).
    fn new_name<'a>(&self, input: &'a str) -> Result<&'a str, CreateError> {
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
        Ok(name)
    }

    /// Moves the default Signal's data into `~/Signal/<name>` (one rename, so
    /// nothing is copied or lost) and gives it a launcher. The default Signal
    /// is left empty.
    pub fn adopt_default(&self, input: &str) -> Result<String, AdoptError> {
        let default = &self.paths.default_data_dir;
        if !default.is_dir() {
            return Err(AdoptError::NoDefault);
        }
        if procs::running(&self.paths.proc_root).default {
            return Err(AdoptError::Running);
        }
        let name = self.new_name(input)?;
        let dir = self.paths.profile_dir(name);
        if dir.exists() {
            return Err(CreateError::Exists(name.to_string()).into());
        }
        fs::create_dir_all(&self.paths.signal_base).map_err(AdoptError::Move)?;
        fs::rename(default, &dir).map_err(AdoptError::Move)?;
        launcher::write(&self.paths, name).map_err(|e| AdoptError::Launcher {
            name: name.to_string(),
            source: e,
        })?;
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
        self.launch_profile(name, None)
    }

    /// Starts (or brings forward) the default Signal, unless it's locked.
    pub fn launch_default(&self) -> io::Result<()> {
        self.launch_default_with(None)
    }

    /// Hands a Signal link (`sgnl://`, `signalcaptcha://`) to a profile, by
    /// its key; a running instance receives it, otherwise Signal starts with
    /// it.
    pub fn open_link(&self, name: &str, link: &str) -> io::Result<()> {
        if !links::is_signal_link(link) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a Signal link: {link}"),
            ));
        }
        if name == profiles::DEFAULT_NAME {
            self.launch_default_with(Some(link))
        } else {
            self.launch_profile(name, Some(link))
        }
    }

    fn launch_profile(&self, name: &str, link: Option<&str>) -> io::Result<()> {
        if !names::is_valid(name) || !self.paths.profile_dir(name).is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no profile named {name:?}"),
            ));
        }
        self.launcher.launch(&self.paths, name, link)
    }

    fn launch_default_with(&self, link: Option<&str>) -> io::Result<()> {
        if lock::is_locked(&self.paths) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the default Signal is locked; unlock it first",
            ));
        }
        self.launcher.launch_default(&self.paths, link)
    }

    /// Hides the snap's Signal entry so the default isn't opened by accident.
    pub fn lock_default(&self) -> io::Result<()> {
        lock::lock(&self.paths)
    }

    pub fn unlock_default(&self) -> io::Result<()> {
        lock::unlock(&self.paths)
    }
}
