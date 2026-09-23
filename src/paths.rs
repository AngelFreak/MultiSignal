//! Every location the app touches, so tests can point it at a temp dir.

use std::path::{Path, PathBuf};

pub const SNAP_DESKTOP_HINT: &str =
    "/var/lib/snapd/desktop/applications/signal-desktop_signal-desktop.desktop";
pub const SIGNAL_ICON: &str = "/snap/signal-desktop/current/meta/gui/signal-desktop.png";

#[derive(Clone, Debug)]
pub struct Paths {
    /// `~/Signal`: one sub-directory per profile.
    pub signal_base: PathBuf,
    /// The Signal snap's own data, used when Signal runs without a profile
    /// (`~/snap/signal-desktop/current/.config/Signal`).
    pub default_data_dir: PathBuf,
    /// `$XDG_DATA_HOME/applications` (default `~/.local/share/applications`).
    pub applications: PathBuf,
    /// Signal binary. `MULTISIGNAL_SIGNAL_BIN` overrides it for smoke tests.
    pub signal_bin: PathBuf,
    /// `/proc`, replaced by a fake tree in tests.
    pub proc_root: PathBuf,
    /// Window size and appearance (`$XDG_CONFIG_HOME/multisignal/settings.ini`).
    pub settings: PathBuf,
}

impl Paths {
    pub fn for_home(home: &Path, xdg_data_home: Option<&Path>) -> Self {
        let data = xdg_data_home.map_or_else(|| home.join(".local/share"), Path::to_path_buf);
        Self {
            signal_base: home.join("Signal"),
            default_data_dir: home.join("snap/signal-desktop/current/.config/Signal"),
            applications: data.join("applications"),
            signal_bin: PathBuf::from("/snap/bin/signal-desktop"),
            proc_root: PathBuf::from("/proc"),
            settings: home.join(".config/multisignal/settings.ini"),
        }
    }

    /// Reads `HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME` and
    /// `MULTISIGNAL_SIGNAL_BIN`.
    pub fn from_env() -> Result<Self, String> {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        let xdg = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty());
        let mut paths = Self::for_home(Path::new(&home), xdg.as_deref().map(Path::new));
        if let Some(config) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            paths.settings = Path::new(&config).join("multisignal/settings.ini");
        }
        if let Some(bin) = std::env::var_os("MULTISIGNAL_SIGNAL_BIN") {
            paths.signal_bin = bin.into();
        }
        Ok(paths)
    }

    pub fn profile_dir(&self, name: &str) -> PathBuf {
        self.signal_base.join(name)
    }

    pub fn own_launcher(&self, name: &str) -> PathBuf {
        self.applications.join(format!("Signal-{name}.desktop"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_locations_from_home() {
        let p = Paths::for_home(Path::new("/h"), None);
        assert_eq!(p.profile_dir("Work"), Path::new("/h/Signal/Work"));
        assert_eq!(
            p.own_launcher("Work"),
            Path::new("/h/.local/share/applications/Signal-Work.desktop")
        );
    }

    #[test]
    fn respects_xdg_data_home() {
        let p = Paths::for_home(Path::new("/h"), Some(Path::new("/data")));
        assert_eq!(p.applications, Path::new("/data/applications"));
    }

    #[test]
    fn default_signal_data_is_the_snaps_own_folder() {
        let p = Paths::for_home(Path::new("/h"), None);
        assert_eq!(
            p.default_data_dir,
            Path::new("/h/snap/signal-desktop/current/.config/Signal")
        );
    }

    #[test]
    fn settings_live_in_the_config_dir() {
        let p = Paths::for_home(Path::new("/h"), None);
        assert_eq!(p.settings, Path::new("/h/.config/multisignal/settings.ini"));
    }
}
