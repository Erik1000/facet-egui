//! # FacetProbe Gallery Example
//!
//! This example demonstrates the full capabilities of `FacetProbe` in action.
//!
//! It showcases:
//! - **Editable mode** (left panel) — Real-time modification of nested data structures
//! - **Readonly mode** (right panel) — Display of data without allowing edits
//! - **Complex nested types** — Structs, enums, vectors, maps, and custom types
//! - **Custom attributes** — `#[facet(readonly)]`, `#[facet(skip)]`, `#[facet(rename)]`
//! - **Enum variant switching** — Interactive enum selection
//! - **List/map manipulation** — Push/pop and element management
//! - **Smart pointer handling** — Transparent navigation through data structures
//!
//! Run with: `cargo run --example probe_gallery`

use eframe::egui;
use egui::{Color32, Frame, RichText, ScrollArea, Stroke, Vec2};
use facet::Facet;
use facet_egui::FacetProbe;
use std::{
    collections::BTreeMap,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn main() -> eframe::Result<()> {
    let mut options = eframe::NativeOptions::default();
    options.viewport.inner_size = Some(Vec2::new(1200.0, 780.0));

    eframe::run_native(
        "FacetProbe Gallery",
        options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}

#[derive(Debug, Facet, Default)]
#[repr(C)]
enum ThemeMode {
    #[default]
    Light,
    Dark,
    Auto,
}

#[derive(Debug, Facet, Default)]
struct Audio {
    volume: f32,
    muted: bool,
    output: String,
}

#[derive(Debug, Facet, Default)]
struct Player {
    nickname: String,
    active: bool,
    score: u64,
    #[facet(facet_egui::rename("HP"))]
    health_points: f32,
    loadout: Vec<String>,
}

#[derive(Debug, Facet)]
struct Profile {
    title: String,
    theme: ThemeMode,
    tags: Vec<String>,
    properties: BTreeMap<String, i32>,
    audio: Audio,
    squad: Vec<Player>,
    #[facet(facet_egui::readonly)]
    build_id: String,
    #[facet(facet_egui::skip)]
    _internal_cache: Vec<u8>,
}

struct App {
    profile: Profile,
    scratch_profile: Profile,
    screenshot_status: Option<String>,
}

impl App {
    const SCREENSHOT_TAG: &'static str = "probe_gallery_capture";

    fn seeded_profile() -> Profile {
        let mut properties = BTreeMap::new();
        properties.insert("max_particles".to_owned(), 5000);
        properties.insert("physics_rate".to_owned(), 120);

        Profile {
            title: "Aurora Runner".to_owned(),
            theme: ThemeMode::Dark,
            tags: vec!["featured".to_owned(), "demo".to_owned(), "beta".to_owned()],
            properties,
            audio: Audio {
                volume: 0.82,
                muted: false,
                output: "Studio Headphones".to_owned(),
            },
            squad: vec![
                Player {
                    nickname: "Nova".to_owned(),
                    active: true,
                    score: 12_500,
                    health_points: 96.0,
                    loadout: vec!["Pulse Rifle".to_owned(), "Shield Pack".to_owned()],
                },
                Player {
                    nickname: "Milo".to_owned(),
                    active: false,
                    score: 8_420,
                    health_points: 74.0,
                    loadout: vec!["Drone".to_owned()],
                },
            ],
            build_id: "v0.9.14+gallery".to_owned(),
            _internal_cache: vec![1, 2, 3, 5, 8],
        }
    }

    fn request_screenshot(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
            Self::SCREENSHOT_TAG.to_owned(),
        )));
        self.screenshot_status = Some("Capturing screenshot...".to_owned());
    }

    fn handle_screenshot_events(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            let egui::Event::Screenshot {
                user_data, image, ..
            } = event
            else {
                continue;
            };

            let is_ours = user_data
                .data
                .as_ref()
                .and_then(|data| data.downcast_ref::<String>())
                .is_some_and(|tag| tag == Self::SCREENSHOT_TAG);

            if !is_ours {
                continue;
            }

            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let filename = format!("probe_gallery_{stamp}.png");

            self.screenshot_status = match save_color_image_png(Path::new(&filename), &image) {
                Ok(()) => Some(format!("Saved screenshot to {filename}")),
                Err(err) => Some(format!("Failed to save screenshot: {err}")),
            };
        }
    }
}

fn save_color_image_png(path: &Path, image: &egui::ColorImage) -> Result<(), String> {
    let [width, height] = image.size;

    let mut rgba = Vec::with_capacity(width * height * 4);
    for pixel in &image.pixels {
        rgba.extend_from_slice(&pixel.to_array());
    }

    let image_buffer = image::RgbaImage::from_raw(width as u32, height as u32, rgba)
        .ok_or_else(|| "failed to build RGBA image buffer".to_owned())?;

    image_buffer.save(path).map_err(|err| format!("{err}"))?;

    Ok(())
}

impl Default for Profile {
    fn default() -> Self {
        App::seeded_profile()
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_screenshot_events(ui.ctx());

        let bg = Color32::from_rgb(18, 21, 31);
        let card = Color32::from_rgb(25, 30, 42);
        let border = Color32::from_rgb(74, 116, 203);

        ui.visuals_mut().panel_fill = bg;
        ui.visuals_mut().widgets.noninteractive.bg_fill = bg;

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.add_space(6.0);
            ui.heading(
                RichText::new("FacetProbe")
                    .size(28.0)
                    .color(Color32::from_rgb(220, 230, 255)),
            );
            ui.horizontal(|ui| {
                if ui.button("Take screenshot").clicked() {
                    self.request_screenshot(ui.ctx());
                }
                if let Some(status) = &self.screenshot_status {
                    ui.label(RichText::new(status).color(Color32::from_rgb(166, 185, 230)));
                }
            });
            ui.label(
                RichText::new(
                    "A compact visual showcase of nested, readonly, skipped, and list/map editing.",
                )
                .color(Color32::from_rgb(166, 185, 230)),
            );
            ui.add_space(12.0);

            ui.columns(2, |columns| {
                Frame::new()
                    .fill(card)
                    .stroke(Stroke::new(1.0_f32, border))
                    .corner_radius(10.0)
                    .inner_margin(14.0)
                    .show(&mut columns[0], |ui| {
                        ui.heading("Editable Profile");
                        ui.separator();
                        ui.label("This panel uses edit mode.");
                        ScrollArea::vertical()
                            .id_salt("editable_scroll")
                            .show(ui, |ui| {
                                FacetProbe::new(&mut self.profile)
                                    .with_id_source("editable_probe")
                                    .with_header("profile")
                                    .show(ui);
                            });
                    });

                Frame::new()
                    .fill(card)
                    .stroke(Stroke::new(1.0_f32, border))
                    .corner_radius(10.0)
                    .inner_margin(14.0)
                    .show(&mut columns[1], |ui| {
                        ui.heading("Readonly Snapshot");
                        ui.separator();
                        ui.label("This panel uses readonly mode with the same data shape.");
                        ScrollArea::vertical()
                            .id_salt("readonly_scroll")
                            .show(ui, |ui| {
                                FacetProbe::new(&mut self.scratch_profile)
                                    .with_id_source("readonly_probe")
                                    .with_header("snapshot")
                                    .readonly()
                                    .show(ui);
                            });
                    });
            });
        });
    }
}

impl Default for App {
    fn default() -> Self {
        let seeded = App::seeded_profile();
        Self {
            profile: seeded,
            scratch_profile: App::seeded_profile(),
            screenshot_status: None,
        }
    }
}
