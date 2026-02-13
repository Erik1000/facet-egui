use eframe::egui;
use facet::Facet;
use facet_egui::FacetProbe;
use std::collections::{HashMap, HashSet};

#[derive(Facet, Debug, Clone, PartialEq, Eq, Hash, Default)]
struct Tag {
    name: String,
    priority: u8,
}

#[derive(Facet, Debug, Clone, Default)]
#[repr(C)]
enum Status {
    #[default]
    Pending,
    InProgress {
        started_at: String,
        assigned_to: String,
    },
    Completed(String), // completion date
    Failed {
        reason: String,
        retries: u32,
    },
}

#[derive(Facet, Debug, Clone, Default)]
struct Task {
    id: u64,
    title: String,
    description: Option<String>,
    status: Status,
    tags: Vec<Tag>,
    metadata: HashMap<String, String>,
}

#[derive(Facet, Debug, Clone, Default)]
struct Project {
    name: String,
    tasks: Vec<Task>,
    active_task_index: Option<usize>,
    collaborators: HashSet<String>,
    settings: ProjectSettings,
}

#[derive(Facet, Debug, Clone, Default)]
struct ProjectSettings {
    max_tasks: u32,
    allow_duplicates: bool,
    priority_levels: [String; 3],
    nested_options: Option<Option<String>>,
}

#[derive(Facet, Default)]
struct AppState {
    project: Project,
    results: Vec<Result<String, String>>,
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Container Types Example",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::new()))),
    )
}

struct MyApp {
    state: AppState,
}

impl MyApp {
    fn new() -> Self {
        let mut state = AppState::default();

        // Pre-populate with sample data
        state.project.name = "My Project".to_string();
        state.project.settings.max_tasks = 100;
        state.project.settings.priority_levels =
            ["Low".to_string(), "Medium".to_string(), "High".to_string()];

        state.project.collaborators.insert("Alice".to_string());
        state.project.collaborators.insert("Bob".to_string());

        let mut task1 = Task::default();
        task1.id = 1;
        task1.title = "Implement feature X".to_string();
        task1.description = Some("A detailed description of the feature".to_string());
        task1.status = Status::InProgress {
            started_at: "2025-01-15".to_string(),
            assigned_to: "Alice".to_string(),
        };
        task1.tags.push(Tag {
            name: "urgent".to_string(),
            priority: 1,
        });
        task1.tags.push(Tag {
            name: "backend".to_string(),
            priority: 2,
        });
        task1
            .metadata
            .insert("estimate".to_string(), "3 days".to_string());

        let mut task2 = Task::default();
        task2.id = 2;
        task2.title = "Fix bug Y".to_string();
        task2.description = None;
        task2.status = Status::Completed("2025-01-10".to_string());

        state.project.tasks.push(task1);
        state.project.tasks.push(task2);
        state.project.active_task_index = Some(0);

        // Sample results
        state.results.push(Ok("Operation succeeded".to_string()));
        state.results.push(Err("Something went wrong".to_string()));

        Self { state }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Container Types Demo");
            ui.label("This example demonstrates Vec, HashMap, HashSet, Option, Result, arrays, and nested enums");
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut probe = FacetProbe::new(&mut self.state);
                probe.ui(ui);
            });
        });
    }
}
