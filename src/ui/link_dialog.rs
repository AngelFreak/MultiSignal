//! "Which Signal is this link for?": asks which profile gets a Signal link
//! when more than one (or none) is running.

use super::{MainWindow, avatar, dialog_frame, push_button};
use crate::profiles::Profile;
use adw::prelude::*;
use std::rc::Rc;

pub struct LinkDialog {
    pub dialog: adw::Dialog,
    choices: Vec<(String, gtk::ListBoxRow)>,
}

impl LinkDialog {
    /// The profiles offered, as shown.
    pub fn titles(&self) -> Vec<String> {
        self.choices.iter().map(|(t, _)| t.clone()).collect()
    }

    /// Picks a profile by its shown name, as a click would.
    pub fn choose(&self, title: &str) {
        if let Some((_, row)) = self.choices.iter().find(|(t, _)| t == title) {
            row.activate();
        }
    }
}

pub fn present(win: &Rc<MainWindow>, link: &str, candidates: &[Profile]) -> LinkDialog {
    let icon = gtk::Image::builder()
        .icon_name("insert-link-symbolic")
        .pixel_size(28)
        .css_classes(["link-badge"])
        .build();
    let any_running = candidates.iter().any(|p| p.running);
    let frame = dialog_frame(
        &icon,
        "Which Signal is this link for?",
        if any_running {
            "More than one Signal is running. Choose the one that asked for this link, for example to finish a captcha."
        } else {
            "No Signal is running. Choose the profile to open with this link."
        },
        440,
    );

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.add_css_class("link-choices");
    let mut choices = Vec::new();
    let mut keys = Vec::new();
    for p in candidates {
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        content.append(&avatar::with_status(&p.title, 30, p.running, "dot-30"));
        let name = gtk::Label::builder()
            .label(&p.title)
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["row-name"])
            .build();
        content.append(&name);
        if p.running {
            content.append(
                &gtk::Label::builder()
                    .label("Running")
                    .css_classes(["status-running"])
                    .build(),
            );
        }
        let row = gtk::ListBoxRow::builder()
            .child(&content)
            .activatable(true)
            .build();
        list.append(&row);
        choices.push((p.title.clone(), row));
        keys.push(p.name.clone());
    }
    frame.extra.append(&list);
    frame.extra.set_margin_start(0);
    frame.extra.set_visible(true);

    let cancel = push_button("Cancel", &[], Some("esc"));
    frame.buttons.append(&cancel);
    let dialog = frame.dialog.clone();
    dialog.set_default_widget(Some(&cancel));
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    list.connect_row_activated({
        let win = Rc::downgrade(win);
        let dialog = dialog.clone();
        let link = link.to_string();
        move |_, row| {
            let Some(win) = win.upgrade() else { return };
            if let Some(key) = keys.get(row.index() as usize) {
                dialog.close();
                win.deliver_link(key, &link);
            }
        }
    });

    dialog.present(Some(&win.window));
    LinkDialog { dialog, choices }
}
