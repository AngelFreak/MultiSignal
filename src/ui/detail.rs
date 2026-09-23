//! The detail pane: hero (avatar, name, status, buttons), Storage and App Menu
//! cards, and the Move to Trash card. Buttons fire `win.*` actions, so their
//! sensitivity follows the actions.

use super::{avatar, push_button};
use crate::launcher;
use crate::profiles::Profile;
use crate::units;
use adw::prelude::*;
use std::path::Path;

pub struct DetailWidgets {
    pub root: gtk::Box,
    pub title: gtk::Label,
    pub primary: gtk::Button,
    pub repair: Option<gtk::Button>,
    values: Vec<(&'static str, gtk::Label)>,
}

impl DetailWidgets {
    /// The value shown next to a card row's title, e.g. "Size".
    pub fn value(&self, title: &str) -> Option<String> {
        self.values
            .iter()
            .find(|(t, _)| *t == title)
            .map(|(_, label)| label.text().to_string())
    }
}

pub fn build(p: &Profile, compact: bool) -> DetailWidgets {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 28);
    root.add_css_class("detail");
    if compact {
        root.set_margin_top(16);
        root.set_margin_start(16);
        root.set_margin_end(16);
        root.set_margin_bottom(40);
    } else {
        root.set_margin_top(24);
        root.set_margin_start(40);
        root.set_margin_end(40);
        root.set_margin_bottom(48);
    }

    let (hero, title, primary) = hero(p, compact);
    root.append(&hero);

    let mut values = Vec::new();

    let storage = card();
    values.push(add_value_row(&storage, "Data folder", &tilde(&p.dir)));
    values.push(add_value_row(&storage, "Size", &units::size(p.size_bytes)));
    root.append(&section("Storage", &storage));

    let app_menu = card();
    let mut repair = None;
    match p.launchers.first() {
        Some(file) => {
            let name =
                launcher::display_name(file).unwrap_or_else(|| format!("Signal ({})", p.name));
            let value = value_label(&name);
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.add_css_class("success-icon");
            let suffix = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            suffix.append(&value);
            suffix.append(&check);
            add_row(&app_menu, "Menu entry", &suffix);
            values.push(("Menu entry", value));
        }
        None => {
            let value = gtk::Label::builder()
                .label("Missing")
                .css_classes(["row-warning-value"])
                .build();
            let button = push_button("Repair", &["small", "link-text"], None);
            button.set_action_name(Some("win.repair"));
            button.set_valign(gtk::Align::Center);
            let suffix = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            suffix.append(&value);
            suffix.append(&button);
            add_row(&app_menu, "Menu entry", &suffix);
            values.push(("Menu entry", value));
            repair = Some(button);
        }
    }
    values.push(add_value_row(&app_menu, "Launcher file", &launcher_file(p)));
    root.append(&section("App Menu", &app_menu));

    let danger = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let trash_card = card();
    let trash = gtk::Label::builder()
        .label("Move to Trash…")
        .xalign(0.0)
        .css_classes(["destructive-row"])
        .build();
    let trash_row = gtk::ListBoxRow::builder()
        .child(&trash)
        .activatable(true)
        .action_name("win.trash-selected")
        .build();
    trash_card.append(&trash_row);
    danger.append(&trash_card);
    if p.running {
        danger.append(&footnote(&format!(
            "Quit Signal ({}) before moving it to the Trash. Closing its window can leave it running in the tray.",
            p.name
        )));
    }
    root.append(&danger);

    DetailWidgets {
        root,
        title,
        primary,
        repair,
        values,
    }
}

fn hero(p: &Profile, compact: bool) -> (gtk::Box, gtk::Label, gtk::Button) {
    let size = if compact { 88 } else { 96 };
    let picture = avatar::with_status(&p.name, size, p.running, "dot-hero");
    picture.add_css_class("hero-avatar");

    let title = gtk::Label::builder()
        .label(&p.name)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["hero-title"])
        .build();

    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("status-dot-small");
    let status_text = gtk::Label::new(Some(if p.running { "Running" } else { "Not running" }));
    let status = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    status.add_css_class(if p.running {
        "status-running"
    } else {
        "status-stopped"
    });
    dot.set_valign(gtk::Align::Center);
    status.append(&dot);
    status.append(&status_text);

    let primary = push_button(
        if p.running {
            "Show Signal"
        } else {
            "Open Signal"
        },
        &["suggested-action"],
        None,
    );
    primary.set_action_name(Some("win.open-selected"));
    let files = push_button("Show in Files", &[], None);
    files.set_action_name(Some("win.show-selected"));
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_margin_top(8);
    buttons.append(&primary);
    buttons.append(&files);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
    text.append(&title);
    text.append(&status);
    text.append(&buttons);

    let hero = if compact {
        let hero = gtk::Box::new(gtk::Orientation::Vertical, 14);
        for w in [
            title.upcast_ref::<gtk::Widget>(),
            status.upcast_ref(),
            buttons.upcast_ref(),
            text.upcast_ref(),
        ] {
            w.set_halign(gtk::Align::Center);
        }
        hero.append(&picture);
        hero.append(&text);
        hero
    } else {
        let hero = gtk::Box::new(gtk::Orientation::Horizontal, 24);
        for w in [
            title.upcast_ref::<gtk::Widget>(),
            status.upcast_ref(),
            buttons.upcast_ref(),
        ] {
            w.set_halign(gtk::Align::Start);
        }
        text.set_valign(gtk::Align::Center);
        hero.append(&picture);
        hero.append(&text);
        hero
    };
    (hero, title, primary)
}

fn card() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list
}

fn section(title: &str, card: &gtk::ListBox) -> gtk::Box {
    let heading = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["section-title"])
        .build();
    let section = gtk::Box::new(gtk::Orientation::Vertical, 6);
    section.append(&heading);
    section.append(card);
    section
}

fn add_row(card: &gtk::ListBox, title: &str, suffix: &impl IsA<gtk::Widget>) {
    let row = adw::ActionRow::builder()
        .title(title)
        .use_markup(false)
        .activatable(false)
        .build();
    row.add_suffix(suffix);
    card.append(&row);
}

fn add_value_row(
    card: &gtk::ListBox,
    title: &'static str,
    value: &str,
) -> (&'static str, gtk::Label) {
    let label = value_label(value);
    add_row(card, title, &label);
    (title, label)
}

fn value_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .css_classes(["row-value"])
        .build()
}

fn footnote(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["footnote"])
        .build()
}

/// "signal-desktop-damon.desktop · made by hand", or "—" without a launcher.
fn launcher_file(p: &Profile) -> String {
    let Some(file) = p.launchers.first() else {
        return "—".to_string();
    };
    let file_name = file.file_name().unwrap_or_default().to_string_lossy();
    if file_name == format!("Signal-{}.desktop", p.name) {
        file_name.into_owned()
    } else {
        format!("{file_name} · made by hand")
    }
}

/// `/home/me/Signal/Work` → `~/Signal/Work`.
fn tilde(path: &Path) -> String {
    match path.strip_prefix(gtk::glib::home_dir()) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}
