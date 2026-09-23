//! Round initial badges with Apple-style gradients. Not `AdwAvatar`, whose
//! colours come from libadwaita's palette; the colour here is picked from the
//! name, so a profile keeps its colour when others are added or removed.

use adw::prelude::*;

const COLORS: usize = 6;

/// A `size`-pixel round badge showing the name's initial ("+" when empty).
pub fn new(name: &str, size: i32) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("avatar");
    label.add_css_class(&format!("avatar-{size}"));
    label.set_size_request(size, size);
    label.set_halign(gtk::Align::Center);
    label.set_valign(gtk::Align::Center);
    label.set_accessible_role(gtk::AccessibleRole::Presentation);
    set_name(&label, name);
    label
}

/// Updates the initial and colour, e.g. while the user types a new name.
pub fn set_name(label: &gtk::Label, name: &str) {
    label.set_text(&initial(name));
    for i in 0..COLORS {
        label.remove_css_class(&format!("avatar-c{i}"));
    }
    label.add_css_class(&format!("avatar-c{}", color_index(name)));
}

/// A `size`-pixel avatar with the green "running" dot in its corner.
pub fn with_status(name: &str, size: i32, running: bool, dot_class: &str) -> gtk::Overlay {
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&new(name, size)));
    overlay.set_halign(gtk::Align::Center);
    overlay.set_valign(gtk::Align::Center);
    if running {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("status-dot");
        dot.add_css_class(dot_class);
        dot.set_halign(gtk::Align::End);
        dot.set_valign(gtk::Align::End);
        overlay.add_overlay(&dot);
    }
    overlay
}

fn initial(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map_or_else(|| "+".to_string(), |c| c.to_uppercase().collect())
}

fn color_index(name: &str) -> usize {
    let name = name.trim().to_lowercase();
    name.bytes().map(usize::from).sum::<usize>() % COLORS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_the_initial_or_a_plus() {
        assert_eq!(initial("damon"), "D");
        assert_eq!(initial("  "), "+");
    }

    #[test]
    fn colour_depends_only_on_the_name() {
        assert_eq!(color_index("Work"), color_index("work"));
        assert!(color_index("Work") < COLORS);
    }
}
