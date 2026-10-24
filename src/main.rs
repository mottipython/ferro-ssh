mod app;
mod modals;
mod profiles;
mod security;
mod session;
mod sftp;
mod terminal;
mod theme;
mod toolbox;
mod tunnels;

use app::FerroSshApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FerroSSH - Secure GUI SSH Client")
            .with_inner_size([1150.0, 750.0])
            .with_min_inner_size([800.0, 520.0])
            .with_app_id("ferro-ssh"),
        ..Default::default()
    };

    eframe::run_native(
        "FerroSSH",
        native_options,
        Box::new(|cc| Ok(Box::new(FerroSshApp::new(cc)))),
    )
}
