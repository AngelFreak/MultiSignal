//! The GTK application: one window, keyboard shortcuts, and Signal links
//! passed on the command line (`multisignal signalcaptcha://…`, which is how
//! the desktop runs a link handler). Links are read from the raw arguments,
//! not as GFiles: GIO would normalise `signalcaptcha://token` to
//! `signalcaptcha://token/`, corrupting the token. A second launch forwards
//! its arguments to the running app.

use super::{Deps, MainWindow, build_window, load_css};
use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// Builds the application. `deps` makes the window's dependencies when the
/// window is first needed.
pub fn application(app_id: &str, deps: impl Fn() -> Deps + 'static) -> adw::Application {
    let app = adw::Application::builder()
        .application_id(app_id)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    app.connect_startup(|app| {
        load_css();
        let quit = gio::SimpleAction::new("quit", None);
        quit.connect_activate({
            let app = app.clone();
            move |_, _| app.quit()
        });
        app.add_action(&quit);
        app.set_accels_for_action("app.quit", &["<Control>q"]);
        app.set_accels_for_action("window.close", &["<Control>w"]);
        app.set_accels_for_action("win.new-profile", &["<Control>n"]);
    });

    // The one window; a second launch or a link brings it forward.
    let current: Rc<RefCell<Weak<MainWindow>>> = Rc::default();
    let show = Rc::new(move |app: &adw::Application| -> Rc<MainWindow> {
        if let Some(win) = current.borrow().upgrade() {
            win.window.present();
            return win;
        }
        let win = build_window(deps());
        win.window.set_application(Some(app));
        win.window.present();
        *current.borrow_mut() = Rc::downgrade(&win);
        win
    });

    app.connect_activate({
        let show = show.clone();
        move |app| {
            show(app);
        }
    });
    app.connect_command_line(move |app, command_line| {
        let win = show(app);
        let links = command_line
            .arguments()
            .into_iter()
            .skip(1) // the program name
            .filter_map(|arg| arg.into_string().ok());
        for link in links {
            win.open_link(&link);
        }
        glib::ExitCode::SUCCESS
    });
    app
}
