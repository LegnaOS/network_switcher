mod config;
mod gui;
mod network;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 600.0])
            .with_min_inner_size([400.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Network Switcher - 网络配置切换器",
        options,
        Box::new(|cc| Ok(Box::new(gui::NetworkSwitcherApp::new(cc)))),
    )
}
