//! "Move “Damon” to the Trash?": a desktop dialog where Cancel is the default.

use super::{MainWindow, avatar, dialog_frame, push_button};
use crate::profiles::Profile;
use crate::units;
use adw::prelude::*;
use std::rc::Rc;

pub struct TrashDialog {
    pub dialog: adw::Dialog,
    pub title: gtk::Label,
    pub cancel: gtk::Button,
    pub confirm: gtk::Button,
}

/// Builds and presents the dialog for `profile` over the main window.
pub fn present(win: &Rc<MainWindow>, profile: &Profile) -> TrashDialog {
    let icon = gtk::Overlay::new();
    icon.set_child(Some(&avatar::new(&profile.name, 52)));
    let badge = gtk::Image::from_icon_name("user-trash-symbolic");
    badge.add_css_class("trash-badge");
    badge.set_halign(gtk::Align::End);
    badge.set_valign(gtk::Align::End);
    icon.add_overlay(&badge);

    let frame = dialog_frame(
        &icon,
        &format!("Move “{}” to the Trash?", profile.name),
        &format!(
            "Its messages ({}) and app menu entry move to the Trash. You can restore them from the Trash until it’s emptied.",
            units::size(profile.size_bytes)
        ),
        440,
    );
    let cancel = push_button("Cancel", &[], Some("esc"));
    let confirm = push_button("Move to Trash", &["destructive-action"], None);
    frame.buttons.append(&cancel);
    frame.buttons.append(&confirm);
    let dialog = frame.dialog.clone();
    dialog.set_default_widget(Some(&cancel));
    dialog.set_focus(Some(&cancel));

    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    confirm.connect_clicked({
        let win = Rc::downgrade(win);
        let dialog = dialog.clone();
        let name = profile.name.clone();
        move |_| {
            dialog.close();
            if let Some(win) = win.upgrade() {
                win.trash_profile(&name);
            }
        }
    });

    dialog.present(Some(&win.window));
    TrashDialog {
        dialog,
        title: frame.title,
        cancel,
        confirm,
    }
}
