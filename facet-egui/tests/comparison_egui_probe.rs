use eframe::egui;
use egui::{Grid, Ui, Vec2};
use egui_probe::Probe;
use facet_egui::FacetProbe;

use crate::types::DemoValue;

mod types;

fn main() -> eframe::Result<()> {
    println!("starting");
    facet_testhelpers::setup();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Test Window",
        options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}

#[derive(Default)]
struct App {
    // Add your state here
    value: DemoValue,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Central Panel");
            ui.separator();
            // Add your UI elements here
            Grid::new(ui.next_auto_id())
                .num_columns(2)
                .spacing(Vec2::new(100.0, 0.0))
                .show(ui, |ui| {
                    ui.heading("egui-probe");
                    ui.heading("facet-egui");

                    ui.end_row();
                    Probe::new(&mut self.value).show(ui);
                    FacetProbe::new(&mut self.value).show(ui);
                });
        });
    }
}
