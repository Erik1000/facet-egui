#[cfg(feature = "std")]
use std::sync::{Arc, RwLock};

use facet::Facet;
use facet_egui::FacetProbe;

fn render_shapes(mut add_probe: impl FnMut(&mut egui::Ui)) -> usize {
    let ctx = egui::Context::default();
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        add_probe(ui);
    });
    output.shapes.len()
}

#[derive(Facet, Default)]
struct Visible {
    enabled: bool,
    retries: u32,
}

#[derive(Facet, Default)]
#[facet(facet_egui::skip)]
struct Hidden {
    value: u32,
}

#[derive(Facet, Default)]
struct Shared {
    value: u32,
}

#[derive(Facet, Default)]
struct Nested {
    title: String,
    values: Vec<u32>,
    maybe_value: Option<u32>,
}

#[test]
fn skip_type_renders_like_baseline() {
    let baseline_shapes = render_shapes(|_ui| {});

    let mut hidden = Hidden::default();
    let hidden_shapes = render_shapes(|ui| {
        FacetProbe::new(&mut hidden).show(ui);
    });

    let mut visible = Visible::default();
    let visible_shapes = render_shapes(|ui| {
        FacetProbe::new(&mut visible).show(ui);
    });

    assert!(hidden_shapes <= baseline_shapes + 1);
    assert!(visible_shapes > baseline_shapes);
    assert!(visible_shapes > hidden_shapes);
}

#[cfg(feature = "std")]
#[test]
fn shared_rwlock_probe_renders_without_interaction() {
    let shared = Arc::new(RwLock::new(Shared::default()));

    let shapes = render_shapes(|ui| {
        let response = FacetProbe::new(&shared).show(ui);
        assert!(!response.changed());
    });

    assert!(shapes > 0);
}

#[test]
fn expand_all_renders_nested_data_without_panicking() {
    let mut nested = Nested {
        title: "example".to_string(),
        values: vec![1, 2, 3],
        maybe_value: Some(42),
    };

    let shapes = render_shapes(|ui| {
        let response = FacetProbe::new(&mut nested)
            .with_header("nested")
            .expand_all()
            .show(ui);
        assert!(!response.changed());
    });

    assert!(shapes > 0);
}
