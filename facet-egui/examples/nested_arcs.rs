use eframe::egui;
use facet::Facet;
use facet_egui::FacetProbe;
use std::sync::{Arc, Mutex, RwLock, Weak};

#[derive(Debug, Facet, Default)]
struct SharedState {
    message: String,
    count: u32,
}

#[derive(Debug, Facet)]
struct Model {
    // a weak reference to an outer Mutex that contains an Arc to the inner RwLock
    weak_outer: Weak<Mutex<Arc<RwLock<Mutex<SharedState>>>>>,
    // a weak reference directly to the inner nested RwLock
    weak_inner: Weak<RwLock<RwLock<SharedState>>>,
}

struct App {
    strong_inner: Arc<RwLock<RwLock<SharedState>>>,
    strong_outer: Arc<Mutex<Arc<RwLock<Mutex<SharedState>>>>>,
    model: Model,
    extra: ExtraWeaks,
}

impl App {
    fn new() -> Self {
        // strong_inner: Arc<RwLock<RwLock<SharedState>>>
        let inner = RwLock::new(SharedState {
            message: "Nested Arc/Mutex/RwLock example (inner)".to_owned(),
            count: 0,
        });
        let strong_inner = Arc::new(RwLock::new(inner));

        // strong_outer: Arc<Mutex<Arc<RwLock<Mutex<SharedState>>>>> - independent nested value
        let outer_inner = Mutex::new(SharedState {
            message: "Nested Arc/Mutex/RwLock example (outer)".to_owned(),
            count: 0,
        });
        let outer_arc = Arc::new(RwLock::new(outer_inner));
        let strong_outer: Arc<Mutex<Arc<RwLock<Mutex<SharedState>>>>> =
            Arc::new(Mutex::new(outer_arc));

        let model = Model {
            weak_outer: Arc::downgrade(&strong_outer),
            weak_inner: Arc::downgrade(&strong_inner),
        };

        let extra = ExtraWeaks {
            weak_outer: Arc::downgrade(&strong_outer),
            weak_inner: Arc::downgrade(&strong_inner),
            weak_behind_rwlock: RwLock::new(Arc::downgrade(&strong_inner)),
        };

        Self {
            strong_inner,
            strong_outer,
            model,
            extra,
        }
    }
}

#[derive(Debug, Facet)]
struct ExtraWeaks {
    weak_outer: Weak<Mutex<Arc<RwLock<Mutex<SharedState>>>>>,
    weak_inner: Weak<RwLock<RwLock<SharedState>>>,
    // a Weak placed behind an RwLock to demonstrate "Weak behind RwLock"
    weak_behind_rwlock: RwLock<Weak<RwLock<RwLock<SharedState>>>>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Nested Arc/Weak + Mutex + RwLock example");
            ui.label("Probe the `Model` below; it contains Weak pointers to both levels.");
            ui.separator();

            // Show the facet probe bound to the model (demonstrates upgrading Weak)
            FacetProbe::new(&mut self.model)
                .readonly(false)
                .expand_all(true)
                .show(ui);

            ui.separator();
            ui.label("Extra Weaks (separate field):");
            FacetProbe::new(&mut self.extra)
                .readonly(true)
                .expand_all(true)
                .show(ui);

            ui.separator();
            // Read from `strong_inner` (outer RwLock -> inner RwLock -> SharedState)
            if let Ok(outer_read) = self.strong_inner.read() {
                if let Ok(inner_read) = outer_read.read() {
                    ui.label(format!(
                        "Inner state => message: {}, count: {}",
                        inner_read.message, inner_read.count
                    ));
                }
            }

            // Read from `strong_outer` (Mutex -> Arc<RwLock<Mutex<SharedState>>> -> RwLock -> Mutex -> SharedState)
            if let Ok(outer_guard) = self.strong_outer.lock() {
                let arc_rw = outer_guard.clone();
                if let Ok(rw) = arc_rw.read() {
                    if let Ok(ms) = rw.lock() {
                        ui.label(format!(
                            "Outer state => message: {}, count: {}",
                            ms.message, ms.count
                        ));
                    }
                }
            }

            if ui.button("Increment via strong inner").clicked() {
                if let Ok(mut outer_write) = self.strong_inner.write() {
                    if let Ok(mut inner_write) = outer_write.write() {
                        inner_write.count += 1;
                    }
                }
            }

            if ui.button("Increment via strong outer").clicked() {
                if let Ok(mut outer_guard) = self.strong_outer.lock() {
                    let arc_rw = outer_guard.clone();
                    if let Ok(mut rw_write) = arc_rw.write() {
                        if let Ok(mut ms) = rw_write.lock() {
                            ms.count += 1;
                        }
                    }
                }
            }

            if ui.button("Replace inner (drop strong)").clicked() {
                let new_inner = RwLock::new(SharedState::default());
                self.strong_inner = Arc::new(RwLock::new(new_inner));
            }

            if ui.button("Replace outer (drop outer)").clicked() {
                let new_outer_inner = Mutex::new(SharedState::default());
                let new_arc = Arc::new(RwLock::new(new_outer_inner));
                self.strong_outer = Arc::new(Mutex::new(new_arc));
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Nested Arcs example",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
