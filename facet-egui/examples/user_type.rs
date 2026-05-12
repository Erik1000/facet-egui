use eframe::egui;
use egui::ScrollArea;
use facet::Facet;
use facet_egui::FacetProbe;
use std::{
    collections::BTreeMap,
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

#[derive(Debug, Facet, Default)]
#[repr(C)]
pub enum Role {
    Admin,
    Developer,
    #[default]
    Normal,
}

#[derive(Debug, Facet, Default)]
struct MetaInfo {
    not_shared: String,
    foo: Arc<RwLock<bool>>,
}

#[derive(Debug, Facet, Default)]
pub struct User {
    map: BTreeMap<String, User>,
    name: Arc<RwLock<String>>,
    role: Role,
    reports_to: Vec<User>,
    meta: Arc<RwLock<MetaInfo>>,
    option: Option<Role>,
    option2: Option<MetaInfo>,
}

struct App {
    shared: Arc<RwLock<User>>,
}

impl App {
    fn new() -> Self {
        let shared = Arc::new(RwLock::new(User::default()));
        let other_thread = shared.clone();
        std::thread::spawn(move || {
            loop {
                let guard = other_thread.read().unwrap();
                println!("{guard:#?}");
                drop(guard);
                std::thread::sleep(Duration::from_secs_f32(0.5));
            }
        });
        Self { shared }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Central panel showing the current value
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Central Panel");
            ui.separator();
            ui.label("Value:");
            ScrollArea::both().show(ui, |ui| {
                FacetProbe::new(&self.shared).show(ui);
            });
        });
    }
}
