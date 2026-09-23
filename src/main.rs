use adw::prelude::*;
use gtk::{gio, glib};
use multisignal::paths::Paths;
use multisignal::store::Store;
use multisignal::system::{GioTrash, SetsidLauncher, SnapInstaller};
use multisignal::ui::{self, Deps};
use std::sync::Arc;

const APP_ID: &str = "io.github.multisignal.MultiSignal";

fn main() -> glib::ExitCode {
    let paths = match Paths::from_env() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("multisignal: {e}");
            return glib::ExitCode::FAILURE;
        }
    };

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|app| {
        ui::load_css();
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
    app.connect_activate(move |app| {
        // A second launch just brings the existing window forward.
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let deps = Deps {
            store: Store::new(paths.clone(), Box::new(GioTrash), Box::new(SetsidLauncher)),
            installer: Arc::new(SnapInstaller),
        };
        let win = ui::build_window(deps);
        win.window.set_application(Some(app));
        win.window.present();
    });
    app.run()
}
