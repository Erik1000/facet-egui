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
    name: Arc<RwLock<String>>,
    role: Role,
    reports_to: Vec<User>,
    meta: Arc<RwLock<MetaInfo>>,
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Left panel

        // Central panel showing the current value
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Central Panel");
            ui.separator();
            ui.label("Value:");
            FacetProbe::new(&self.shared).show(ui);
        });
    }
}
