use eframe::egui;
use egui::{Grid, Ui, Vec2};
use egui_probe::Probe;
use facet_egui::FacetProbe;

#[path = "comparison_support/comparison_types.rs"]
mod comparison_types;

use comparison_types::DemoValue;

fn main() -> eframe::Result<()> {
    facet_testhelpers::setup();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "facet-egui vs egui-probe",
        options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}

#[derive(Default)]
struct App {
    value: DemoValue,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Visual comparison");
            ui.separator();
            show_compare_grid(ui, &mut self.value);
        });
    }
}

fn show_compare_grid(ui: &mut Ui, value: &mut DemoValue) {
    Grid::new(ui.next_auto_id())
        .num_columns(2)
        .spacing(Vec2::new(100.0, 0.0))
        .show(ui, |ui| {
            ui.heading("egui-probe");
            ui.heading("facet-egui");

            ui.end_row();
            Probe::new(value).show(ui);
            FacetProbe::new(value).show(ui);
        });
}
