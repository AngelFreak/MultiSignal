use adw::prelude::*;
use gtk::glib;

const APP_ID: &str = "io.github.multisignal.MultiSignal";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        adw::ApplicationWindow::builder()
            .application(app)
            .title("Signal Profiles")
            .default_width(1100)
            .default_height(720)
            .build()
            .present();
    });
    app.run()
}
