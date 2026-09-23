//! The name dialog: "New Profile", or "Move to a Profile" for the default
//! Signal's data. It validates the name as you type.

use super::{MainWindow, avatar, dialog_frame, push_button};
use crate::store::Store;
use crate::{names, profiles, units};
use adw::prelude::*;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Validation {
    /// Nothing typed yet: show the hint, not an error.
    Empty,
    Ok,
    Invalid {
        suggestion: Option<String>,
    },
    Exists(String),
}

/// Checks a typed name without touching the disk.
pub fn validate(store: &Store, input: &str) -> Validation {
    let name = input.trim();
    if name.is_empty() {
        Validation::Empty
    } else if !names::is_valid(name) {
        Validation::Invalid {
            suggestion: names::suggest(name),
        }
    } else if let Some(existing) = profiles::existing_ignoring_case(&store.paths, name) {
        Validation::Exists(existing)
    } else {
        Validation::Ok
    }
}

const HINT: &str = "Letters, digits, dot, underscore and dash.";

/// The line under the field: (text, is_error).
fn status(v: &Validation) -> (String, bool) {
    match v {
        Validation::Empty | Validation::Ok => (HINT.to_string(), false),
        Validation::Invalid {
            suggestion: Some(_),
        } => ("Spaces and symbols aren’t allowed.".to_string(), true),
        Validation::Invalid { suggestion: None } => (
            "Use letters, digits, dot, underscore and dash.".to_string(),
            true,
        ),
        Validation::Exists(name) => (format!("“{name}” already exists."), true),
    }
}

/// What the name is for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Purpose {
    /// A new, empty profile.
    NewProfile,
    /// A profile made from the default Signal's data (its size in bytes).
    AdoptDefault(u64),
}

impl Purpose {
    fn title(self) -> &'static str {
        match self {
            Purpose::NewProfile => "New Profile",
            Purpose::AdoptDefault(_) => "Move to a Profile",
        }
    }

    fn button(self) -> &'static str {
        match self {
            Purpose::NewProfile => "Create Profile",
            Purpose::AdoptDefault(_) => "Move",
        }
    }

    fn body(self, name: &str) -> String {
        match self {
            Purpose::NewProfile => format!(
                "A separate Signal account with its own messages. It appears in your app menu as “Signal ({name})”."
            ),
            Purpose::AdoptDefault(bytes) => format!(
                "Moves the default Signal’s messages ({}) into ~/Signal/{name}, with its own app menu entry “Signal ({name})”. The default Signal is left empty.",
                units::size(bytes)
            ),
        }
    }
}

pub struct CreateDialog {
    pub dialog: adw::Dialog,
    pub entry: gtk::Entry,
    pub create: gtk::Button,
    /// The "Use “My-Work”" fix-it, visible when a fixed name exists.
    pub suggestion: gtk::Button,
}

/// Builds and presents the dialog over the main window, with `initial` in
/// the field.
pub fn present(win: &Rc<MainWindow>, purpose: Purpose, initial: &str) -> CreateDialog {
    let picture = avatar::new("", 52);
    let frame = dialog_frame(&picture, purpose.title(), &purpose.body("name"), 460);

    let label = gtk::Label::builder()
        .label("Name")
        .xalign(0.0)
        .css_classes(["field-label"])
        .build();
    let entry = gtk::Entry::builder()
        .placeholder_text("e.g. Work")
        .activates_default(true)
        .css_classes(["field"])
        .build();
    label.set_mnemonic_widget(Some(&entry));
    let message = gtk::Label::builder()
        .label(HINT)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["field-status"])
        .build();
    let suggestion = gtk::Button::builder()
        .css_classes(["flat", "fix-it"])
        .visible(false)
        .build();
    let status_line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    status_line.add_css_class("field-status-line");
    status_line.append(&message);
    status_line.append(&suggestion);
    frame.extra.append(&label);
    frame.extra.append(&entry);
    frame.extra.append(&status_line);
    frame.extra.set_visible(true);

    let cancel = push_button("Cancel", &[], Some("esc"));
    let create = push_button(purpose.button(), &["suggested-action"], Some("↵"));
    create.set_sensitive(false);
    frame.buttons.append(&cancel);
    frame.buttons.append(&create);
    let dialog = frame.dialog.clone();
    dialog.set_default_widget(Some(&create));

    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });

    entry.connect_changed({
        let win = Rc::downgrade(win);
        let (create, suggestion, message, body_label) = (
            create.clone(),
            suggestion.clone(),
            message.clone(),
            frame.body.clone(),
        );
        move |entry| {
            let Some(win) = win.upgrade() else { return };
            let text = entry.text();
            // A move may reuse the default's own display name.
            let v = match purpose {
                Purpose::NewProfile => win.validate_new_name(&text),
                Purpose::AdoptDefault(_) => validate(&win.deps.store, &text),
            };
            let (line, is_error) = status(&v);
            avatar::set_name(&picture, &text);
            create.set_sensitive(v == Validation::Ok);
            message.set_text(&line);
            for widget in [entry.upcast_ref::<gtk::Widget>(), message.upcast_ref()] {
                if is_error {
                    widget.add_css_class("error");
                } else {
                    widget.remove_css_class("error");
                }
            }
            let fix = match &v {
                Validation::Invalid {
                    suggestion: Some(s),
                } => Some(s.clone()),
                _ => None,
            };
            suggestion.set_label(&format!("Use “{}”", fix.as_deref().unwrap_or("")));
            suggestion.set_visible(fix.is_some());
            let preview = if v == Validation::Ok {
                text.trim()
            } else {
                "name"
            };
            body_label.set_text(&purpose.body(preview));
        }
    });

    suggestion.connect_clicked({
        let entry = entry.clone();
        move |button| {
            let text = button.label().unwrap_or_default();
            let fixed = text.trim_start_matches("Use “").trim_end_matches('”');
            entry.set_text(fixed);
            entry.set_position(-1);
            entry.grab_focus();
        }
    });

    create.connect_clicked({
        let win = Rc::downgrade(win);
        let (dialog, entry, message) = (dialog.clone(), entry.clone(), message.clone());
        move |_| {
            let Some(win) = win.upgrade() else { return };
            let done = match purpose {
                Purpose::NewProfile => win.create_profile(&entry.text()).map_err(|e| e.to_string()),
                Purpose::AdoptDefault(_) => {
                    win.adopt_default(&entry.text()).map_err(|e| e.to_string())
                }
            };
            match done {
                Ok(_) => {
                    dialog.close();
                }
                Err(e) => {
                    message.set_text(&e);
                    message.add_css_class("error");
                }
            }
        }
    });

    entry.set_text(initial);
    entry.set_position(-1);
    dialog.set_focus(Some(&entry));
    dialog.present(Some(&win.window));
    CreateDialog {
        dialog,
        entry,
        create,
        suggestion,
    }
}
