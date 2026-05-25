use eframe::egui;
use facet::Facet;
use facet_egui::FacetProbe;
use std::sync::{Arc, RwLock, Weak};

#[derive(Debug, Facet, Default)]
struct SharedState {
    message: String,
    count: u32,
}

#[derive(Debug, Facet)]
struct Model {
    weak_state: Weak<RwLock<SharedState>>,
}

struct App {
    strong_state: Arc<RwLock<SharedState>>,
    model: Model,
}

impl App {
    fn new() -> Self {
        let strong_state = Arc::new(RwLock::new(SharedState {
            message: "Weak<RwLock<T>> can be upgraded by MaybeMut::read()".to_owned(),
            count: 1,
        }));

        let model = Model {
            weak_state: Arc::downgrade(&strong_state),
        };

        Self {
            strong_state,
            model,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Weak<RwLock<T>> example");
            ui.label("This probe is bound to a Weak<RwLock<SharedState>>.");
            ui.separator();

            // This goes through `facet-maybe-mut` internals and upgrades Weak on read.
            FacetProbe::new(&mut self.model)
                .readonly(false)
                .expand_all(true)
                .show(ui);

            ui.separator();
            if let Ok(state) = self.strong_state.read() {
                ui.label(format!(
                    "Current state => message: {}, count: {}",
                    state.message, state.count
                ));
            }

            if ui.button("Increment from strong Arc").clicked()
                && let Ok(mut state) = self.strong_state.write()
            {
                state.count += 1;
            }

            if ui.button("Drop strong Arc in app").clicked() {
                // Replace the strong Arc with a fresh one, effectively dropping
                // this app-held strong reference to the old allocation.
                self.strong_state = Arc::new(RwLock::new(SharedState::default()));
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Weak<RwLock<T>> example",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
