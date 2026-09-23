//! The small settings file: window size and appearance, as a key file.

use gtk::glib;
use std::path::Path;

/// The settings, or an empty file on first run or if it can't be read.
pub fn load(path: &Path) -> glib::KeyFile {
    let file = glib::KeyFile::new();
    let _ = file.load_from_file(path, glib::KeyFileFlags::NONE);
    file
}

pub fn save(path: &Path, file: &glib::KeyFile) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    file.save_to_file(path).map_err(|e| e.to_string())
}

/// The user's name for the default Signal (`[default] name=`), if set.
pub fn default_title(path: &Path) -> Option<String> {
    let name = load(path).string("default", "name").ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Light or dark, or follow GNOME ("system", shown as Automatic).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::System, Appearance::Light, Appearance::Dark];

    /// The value stored in the settings file and used as the action target.
    pub fn id(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Appearance::System => "Automatic",
            Appearance::Light => "Light",
            Appearance::Dark => "Dark",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }

    pub fn apply(self) {
        adw::StyleManager::default().set_color_scheme(match self {
            Appearance::System => adw::ColorScheme::Default,
            Appearance::Light => adw::ColorScheme::ForceLight,
            Appearance::Dark => adw::ColorScheme::ForceDark,
        });
    }

    /// The saved choice, if the user ever made one.
    pub fn load(path: &Path) -> Option<Self> {
        let mode = load(path).string("appearance", "mode").ok()?;
        Self::from_id(&mode)
    }

    pub fn save(self, path: &Path) -> Result<(), String> {
        let file = load(path);
        file.set_string("appearance", "mode", self.id());
        save(path, &file)
    }
}

#[cfg(test)]
mod tests {
    use super::Appearance;

    #[test]
    fn ids_round_trip() {
        for a in Appearance::ALL {
            assert_eq!(Appearance::from_id(a.id()), Some(a));
        }
        assert_eq!(Appearance::from_id("purple"), None);
    }
}
