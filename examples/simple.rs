use eframe::egui;
use facet::Facet;
use facet_egui::FacetProbe;

#[derive(Facet, Default)]
struct Person {
    name: String,
    age: u32,
    active: bool,
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "FacetProbe Example",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

#[derive(Default)]
struct MyApp {
    person: Person,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("FacetProbe Demo");
            ui.separator();

            let mut probe = FacetProbe::new(&mut self.person);
            probe.ui(ui);

            ui.separator();
            ui.label(format!(
                "Current values: name={}, age={}, active={}",
                self.person.name, self.person.age, self.person.active
            ));
        });
    }
}
