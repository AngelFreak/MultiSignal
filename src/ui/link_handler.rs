//! Making Signal Profiles the handler for Signal links, through GIO (the
//! user's mimeapps.list), so each link can be routed to the right profile.

use super::settings;
use crate::links::SCHEMES;
use gtk::gio;
use gtk::gio::prelude::*;
use std::path::Path;

/// Our desktop-file ID (data/io.github.multisignal.MultiSignal.desktop).
pub const DESKTOP_ID: &str = "io.github.multisignal.MultiSignal.desktop";

fn content_type(scheme: &str) -> String {
    format!("x-scheme-handler/{scheme}")
}

fn our_app() -> Option<gio::AppInfo> {
    gio::AppInfo::all()
        .into_iter()
        .find(|app| app.id().is_some_and(|id| id == DESKTOP_ID))
}

/// Whether Signal links open in Signal Profiles.
pub fn is_default() -> bool {
    SCHEMES.iter().all(|scheme| {
        gio::AppInfo::default_for_uri_scheme(scheme)
            .and_then(|app| app.id())
            .is_some_and(|id| id == DESKTOP_ID)
    })
}

/// Makes Signal Profiles the handler for Signal links, or stops it being one
/// (links then go wherever they would without it).
pub fn set_default(on: bool) -> Result<(), String> {
    let app = our_app().ok_or("Signal Profiles isn't installed, so it can't handle links")?;
    for scheme in SCHEMES {
        let content = content_type(scheme);
        let done = if on {
            app.set_as_default_for_type(&content)
        } else {
            gio::AppInfo::reset_type_associations(&content);
            app.remove_supports_type(&content)
        };
        done.map_err(|e| e.message().to_string())?;
    }
    Ok(())
}

/// On the first run only, makes Signal Profiles the link handler; after that
/// the user's choice (the ⋯ menu, or the system settings) is left alone.
pub fn claim_once(settings_path: &Path) {
    let file = settings::load(settings_path);
    if file.boolean("links", "claimed").unwrap_or(false) {
        return;
    }
    match set_default(true) {
        Ok(()) => {
            file.set_boolean("links", "claimed", true);
            if let Err(e) = settings::save(settings_path, &file) {
                eprintln!(
                    "multisignal: could not save {}: {e}",
                    settings_path.display()
                );
            }
        }
        // Not installed (e.g. `cargo run`): try again on a later run.
        Err(e) => eprintln!("multisignal: not handling Signal links: {e}"),
    }
}
