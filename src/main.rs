use adw::prelude::*;
use gtk::glib;
use multisignal::paths::Paths;
use multisignal::store::Store;
use multisignal::system::{GioTrash, SetsidLauncher, SnapInstaller};
use multisignal::ui::{self, Deps, link_handler};
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
    link_handler::claim_once(&paths.settings);
    let app = ui::application(APP_ID, move || Deps {
        store: Store::new(paths.clone(), Box::new(GioTrash), Box::new(SetsidLauncher)),
        installer: Arc::new(SnapInstaller),
    });
    app.run()
}
