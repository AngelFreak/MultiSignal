//! The whole-window "Signal Desktop Isn’t Installed" page. The install itself
//! runs in `MainWindow::install`; this only shows its state.

use super::push_button;
use adw::prelude::*;

pub struct InstallPage {
    pub root: adw::ToolbarView,
    pub button: gtk::Button,
    progress: gtk::Box,
    error: gtk::Label,
}

impl InstallPage {
    /// `other`: how Signal is installed otherwise ("Flatpak", "apt"), if at all.
    pub fn new(other: Option<&str>) -> Self {
        let mut text =
            "Signal Profiles uses the Signal snap. Installing it needs your password.".to_string();
        if let Some(other) = other {
            text.push_str(&format!(
                " The {other} version of Signal that is installed can’t be used for profiles."
            ));
        }
        let (root, _bar, actions) = super::window::whole_window_page(
            "system-software-install-symbolic",
            "Signal Desktop Isn’t Installed",
            &text,
        );

        let button = push_button("Install Signal", &["suggested-action", "large"], None);
        let spinner = gtk::Spinner::new();
        spinner.start();
        let progress = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        progress.add_css_class("install-progress");
        progress.append(&spinner);
        progress.append(&gtk::Label::new(Some(
            "Installing… you may be asked for your password",
        )));
        progress.set_visible(false);
        let error = gtk::Label::builder()
            .wrap(true)
            .justify(gtk::Justification::Center)
            .max_width_chars(60)
            .selectable(true)
            .css_classes(["install-error"])
            .visible(false)
            .build();

        actions.append(&button);
        actions.append(&progress);
        actions.append(&error);
        Self {
            root,
            button,
            progress,
            error,
        }
    }

    pub fn show_installing(&self) {
        self.button.set_visible(false);
        self.error.set_visible(false);
        self.progress.set_visible(true);
    }

    pub fn show_idle(&self) {
        self.progress.set_visible(false);
        self.button.set_visible(true);
    }

    /// Shows the last lines of the installer's output and offers a retry.
    pub fn show_failed(&self, message: &str) {
        self.show_idle();
        super::set_button_label(&self.button, "Try Again");
        self.error.set_text(message);
        self.error.set_visible(!message.is_empty());
    }
}
