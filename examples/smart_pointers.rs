use eframe::egui;
use facet::Facet;
use facet_egui::FacetProbe;
use std::sync::{Arc, RwLock};

/// This example demonstrates editing values behind smart pointers.
/// Since Arc<RwLock<T>> requires locking to access the inner value,
/// we show a pattern where you:
/// 1. Read the current value (via read lock)
/// 2. Edit a local copy
/// 3. Write it back (via write lock) if changed

#[derive(Facet, Debug, Clone, Default)]
struct Config {
    app_name: String,
    version: String,
    debug_mode: bool,
    max_connections: u32,
}

#[derive(Facet, Debug, Clone, Default)]
struct CacheEntry {
    key: String,
    value: String,
    hits: u64,
    expired: bool,
}

#[derive(Facet, Debug, Clone, Default)]
struct SharedCache {
    entries: Vec<CacheEntry>,
    max_size: usize,
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Smart Pointers Example",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::new()))),
    )
}

struct MyApp {
    // The actual shared data
    config: Arc<RwLock<Config>>,
    cache: Arc<RwLock<SharedCache>>,

    // Local copies for editing (synced each frame)
    config_edit: Config,
    cache_edit: SharedCache,
}

impl MyApp {
    fn new() -> Self {
        let config = Config {
            app_name: "SmartPointerDemo".to_string(),
            version: "1.0.0".to_string(),
            debug_mode: true,
            max_connections: 100,
        };

        let cache = SharedCache {
            entries: vec![
                CacheEntry {
                    key: "user:1".to_string(),
                    value: r#"{"name": "Alice"}"#.to_string(),
                    hits: 42,
                    expired: false,
                },
                CacheEntry {
                    key: "user:2".to_string(),
                    value: r#"{"name": "Bob"}"#.to_string(),
                    hits: 17,
                    expired: true,
                },
            ],
            max_size: 1000,
        };

        Self {
            config_edit: config.clone(),
            cache_edit: cache.clone(),
            config: Arc::new(RwLock::new(config)),
            cache: Arc::new(RwLock::new(cache)),
        }
    }

    /// Sync local edits back to the Arc<RwLock<T>>
    fn sync_to_shared(&self) {
        if let Ok(mut config) = self.config.write() {
            *config = self.config_edit.clone();
        }
        if let Ok(mut cache) = self.cache.write() {
            *cache = self.cache_edit.clone();
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Left panel: "Reader 1" showing shared state
        egui::SidePanel::left("reader1").show(ctx, |ui| {
            ui.heading("Reader 1");
            ui.label("(reads from shared Arc)");
            ui.separator();

            if let Ok(config) = self.config.read() {
                ui.group(|ui| {
                    ui.label("Config:");
                    ui.monospace(format!("app_name: {}", config.app_name));
                    ui.monospace(format!("version: {}", config.version));
                    ui.monospace(format!("debug_mode: {}", config.debug_mode));
                    ui.monospace(format!("max_connections: {}", config.max_connections));
                });
            }

            ui.separator();

            if let Ok(cache) = self.cache.read() {
                ui.group(|ui| {
                    ui.label("Cache:");
                    ui.monospace(format!("max_size: {}", cache.max_size));
                    ui.monospace(format!("entries: {}", cache.entries.len()));
                    for (i, entry) in cache.entries.iter().enumerate() {
                        ui.monospace(format!("  [{}] {} = {}", i, entry.key, entry.hits));
                    }
                });
            }
        });

        // Right panel: "Reader 2" showing same shared state
        egui::SidePanel::right("reader2").show(ctx, |ui| {
            ui.heading("Reader 2");
            ui.label("(also reads from same Arc)");
            ui.separator();

            if let Ok(config) = self.config.read() {
                ui.colored_label(egui::Color32::LIGHT_GREEN, "Config state:");
                ui.label(format!("App: {}", config.app_name));
                ui.label(format!(
                    "Debug: {}",
                    if config.debug_mode { "ON" } else { "OFF" }
                ));
            }

            ui.separator();

            if let Ok(cache) = self.cache.read() {
                ui.colored_label(egui::Color32::LIGHT_BLUE, "Cache state:");
                ui.label(format!(
                    "{} entries, max {}",
                    cache.entries.len(),
                    cache.max_size
                ));
                for entry in &cache.entries {
                    let color = if entry.expired {
                        egui::Color32::RED
                    } else {
                        egui::Color32::GREEN
                    };
                    ui.colored_label(color, format!("{}: {} hits", entry.key, entry.hits));
                }
            }

            ui.separator();
            ui.label(format!(
                "Arc strong refs: {}",
                Arc::strong_count(&self.config)
            ));
        });

        // Center panel: Editor
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Editor (writes to shared Arc)");
            ui.label("Edit values here - changes sync to both readers!");
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Edit Config
                ui.heading("Config (Arc<RwLock<Config>>)");
                ui.push_id("config_section", |ui| {
                    let mut probe = FacetProbe::new(&mut self.config_edit);
                    probe.ui(ui);
                });

                ui.separator();

                // Edit Cache
                ui.heading("Cache (Arc<RwLock<SharedCache>>)");
                ui.push_id("cache_section", |ui| {
                    let mut probe = FacetProbe::new(&mut self.cache_edit);
                    probe.ui(ui);
                });
            });

            ui.separator();

            // Sync button (or auto-sync)
            if ui.button("Sync to shared state").clicked() {
                self.sync_to_shared();
            }

            // Show Arc info
            ui.collapsing("Arc Debug Info", |ui| {
                ui.label(format!(
                    "Config Arc strong count: {}",
                    Arc::strong_count(&self.config)
                ));
                ui.label(format!(
                    "Cache Arc strong count: {}",
                    Arc::strong_count(&self.cache)
                ));

                // Show actual shared data (read lock)
                if let Ok(config) = self.config.read() {
                    ui.label(format!("Shared config app_name: {}", config.app_name));
                    ui.label(format!("Shared config debug_mode: {}", config.debug_mode));
                }
                if let Ok(cache) = self.cache.read() {
                    ui.label(format!("Shared cache entries: {}", cache.entries.len()));
                }
            });
        });

        // Auto-sync every frame (optional - could also be explicit via button)
        self.sync_to_shared();
    }
}
