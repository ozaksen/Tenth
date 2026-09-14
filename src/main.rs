#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
#[cfg(target_os = "macos")]
mod macos_menu_bar;
mod model;
mod storage;

use app::HourTrackerApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([980.0, 680.0])
            .with_min_inner_size([640.0, 520.0])
            .with_title("Tenth"),
        ..Default::default()
    };

    eframe::run_native(
        "Tenth",
        native_options,
        Box::new(|cc| Ok(Box::new(HourTrackerApp::new(cc)))),
    )
}
