//! Example demonstrating the `dyn_trait` feature for editing heterogeneous collections.
//!
//! Run with: `cargo run --example dyn_trait --features dyn_trait`

use eframe::egui;
use facet::Facet;
use facet_egui::{poke_from_mut, FacetProbe, FacetShape};

// Different types that all implement Facet (and thus FacetShape via blanket impl)

#[derive(Facet, Debug, Clone)]
struct Player {
    name: String,
    health: i32,
    level: u32,
}

#[derive(Facet, Debug, Clone)]
struct Enemy {
    kind: String,
    damage: f32,
    is_boss: bool,
}

#[derive(Facet, Debug, Clone)]
struct Item {
    name: String,
    quantity: u32,
    weight: f32,
}

#[derive(Facet, Debug, Clone)]
struct Settings {
    volume: f32,
    fullscreen: bool,
    resolution_x: u32,
    resolution_y: u32,
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([600.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Dyn Trait Example",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::new()))),
    )
}

struct MyApp {
    /// A heterogeneous collection of different Facet types
    entities: Vec<Box<dyn FacetShape>>,
    /// Labels for display (since we can't get type name from dyn FacetShape easily)
    labels: Vec<&'static str>,
}

impl MyApp {
    fn new() -> Self {
        Self {
            entities: vec![
                Box::new(Player {
                    name: "Hero".to_string(),
                    health: 100,
                    level: 5,
                }),
                Box::new(Enemy {
                    kind: "Goblin".to_string(),
                    damage: 15.5,
                    is_boss: false,
                }),
                Box::new(Item {
                    name: "Health Potion".to_string(),
                    quantity: 3,
                    weight: 0.5,
                }),
                Box::new(Enemy {
                    kind: "Dragon".to_string(),
                    damage: 100.0,
                    is_boss: true,
                }),
                Box::new(Settings {
                    volume: 0.8,
                    fullscreen: true,
                    resolution_x: 1920,
                    resolution_y: 1080,
                }),
            ],
            labels: vec!["Player", "Enemy (Goblin)", "Item", "Enemy (Dragon)", "Settings"],
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Heterogeneous Collection Editor");
            ui.separator();
            ui.label("Each item below is a different concrete type, all edited through Box<dyn FacetShape>:");
            ui.add_space(10.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, entity) in self.entities.iter_mut().enumerate() {
                    ui.push_id(i, |ui| {
                        let label = self.labels.get(i).copied().unwrap_or("Unknown");
                        
                        egui::CollapsingHeader::new(format!("[{}] {}", i, label))
                            .default_open(true)
                            .show(ui, |ui| {
                                // Convert Box<dyn FacetShape> to Poke and render
                                let poke = poke_from_mut(&mut **entity);
                                FacetProbe::<()>::poke_ui(poke, ui);
                            });
                    });
                    ui.add_space(5.0);
                }
            });
        });
    }
}
