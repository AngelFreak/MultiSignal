//! One sidebar row per profile: avatar with running dot, name, and a
//! secondary line ("No app menu entry ·", "Running ·", size).

use super::avatar;
use crate::profiles::Profile;
use crate::units;
use adw::prelude::*;

/// `compact`: the narrow-window list (bigger avatar, chevron, card rows).
/// The row's widget name is the profile name.
pub fn build(p: &Profile, compact: bool) -> gtk::ListBoxRow {
    let (avatar_size, dot) = if compact {
        (36, "dot-36")
    } else {
        (30, "dot-30")
    };

    let name = gtk::Label::builder()
        .label(&p.name)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["row-name"])
        .build();

    let secondary = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    if p.launchers.is_empty() {
        secondary.append(&small_label("No app menu entry ·", "row-warning"));
    }
    if p.running {
        secondary.append(&small_label("Running ·", "status-running"));
    }
    let size = small_label(&units::size(p.size_bytes), "row-secondary");
    size.add_css_class("numeric");
    secondary.append(&size);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    text.append(&name);
    text.append(&secondary);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, if compact { 12 } else { 10 });
    content.append(&avatar::with_status(&p.name, avatar_size, p.running, dot));
    if compact {
        // Text and chevron share a full-height box that carries the row
        // separator, so it starts under the text like an iOS list.
        let chevron = gtk::Image::from_icon_name("go-next-symbolic");
        chevron.add_css_class("chevron");
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        inner.add_css_class("row-inner");
        inner.set_hexpand(true);
        inner.append(&text);
        inner.append(&chevron);
        content.append(&inner);
    } else {
        content.append(&text);
    }

    let row = gtk::ListBoxRow::new();
    row.set_widget_name(&p.name);
    row.set_child(Some(&content));
    row.update_property(&[gtk::accessible::Property::Label(&p.name)]);
    row
}

fn small_label(text: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .css_classes([class])
        .build()
}

/// The secondary line as one string, e.g. "Running · 77 MB", for tests.
pub fn secondary_text(row: &gtk::ListBoxRow) -> String {
    super::labels_in(row)
        .iter()
        .filter(|l| {
            ["row-warning", "status-running", "row-secondary"]
                .iter()
                .any(|c| l.has_css_class(c))
        })
        .map(|l| l.text().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}
