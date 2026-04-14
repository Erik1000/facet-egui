use eframe::egui;
use facet::Facet;
use facet_egui::FacetProbe;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Shared String Example",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

#[derive(Debug, Facet)]
pub struct User {
    #[facet(facet_egui::readonly)]
    name: Arc<RwLock<String>>,
}

struct App {
    shared_string: Arc<RwLock<String>>,
    normal: String,
}

impl App {
    fn new() -> Self {
        let shared_string = Arc::new(RwLock::new("Edit me!".to_string()));
        let other = shared_string.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let s = other.read().unwrap();
                println!("On the other thread: {s}")
            }
        });

        Self {
            shared_string,
            normal: "This is not synced".to_string(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Left panel
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.heading("Left Panel");
            ui.separator();
            let t = &self.shared_string;
            FacetProbe::new(&t).show(ui);
            ui.label("Normal:");
            FacetProbe::new(&mut self.normal).show(ui);
        });

        // Right panel
        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            ui.heading("Right Panel");
            ui.separator();
            let t = &self.shared_string;
            FacetProbe::new(&t).show(ui);
            ui.label("Normal:");
            // FacetProbe::new(&mut self.shared_string).show(ui);
        });

        // Central panel showing the current value
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Central Panel");
            ui.separator();

            // // Another editor
            ui.label("Third editor:");
            let t = &self.shared_string;
            FacetProbe::new(&t).show(ui);
            ui.separator();

            // Display read-only value
            if let Ok(value) = self.shared_string.read() {
                ui.label(format!("Current value: {}", *value));
            }

            ui.label("Normal");
            ui.text_edit_multiline(&mut self.normal);
        });
    }
}
