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
            .expand_all(true)
            .show(ui);
        assert!(!response.changed());
    });

    assert!(shapes > 0);
}

#[cfg(feature = "std")]
#[test]
fn custom_handler_retains_state_and_respects_readonly() {
    use facet_egui::{get_registered_handler, register_custom_ui, register_custom_ui_shape};
    use std::cell::Cell;

    #[derive(Facet)]
    struct CustomValue {
        value: u32,
    }

    #[derive(Facet)]
    struct OtherValue;

    let observations = Arc::new(RwLock::new(Vec::new()));
    let captured = observations.clone();
    let mut calls = 0usize;
    let non_sync_capture = Cell::new(0usize);
    assert!(register_custom_ui::<CustomValue>(move |mut guard, ui| {
        calls += 1;
        non_sync_capture.set(calls);
        assert_eq!(guard.shape(), CustomValue::SHAPE);
        captured
            .write()
            .unwrap()
            .push((non_sync_capture.get(), guard.as_poke().is_some()));
        register_custom_ui_shape(*OtherValue::SHAPE, Box::new(|_guard, ui| ui.label("other")));
        let mut response = ui.label("custom");
        response.mark_changed();
        response
    })
    .is_none());

    let mut value = CustomValue { value: 7 };
    render_shapes(|ui| {
        assert!(FacetProbe::new(&mut value).show(ui).changed());
        assert!(FacetProbe::new(&mut value).readonly(true).show(ui).changed());
    });

    let recorded = observations.read().unwrap();
    assert!(!recorded.is_empty());
    for (index, &(calls, writable)) in recorded.iter().enumerate() {
        assert_eq!(calls, index + 1);
        assert_eq!(writable, index % 2 == 0);
    }
    let original = get_registered_handler(*CustomValue::SHAPE).unwrap();
    let replaced = register_custom_ui::<CustomValue>(|_guard, ui| ui.label("replacement"))
        .unwrap();
    assert!(Arc::ptr_eq(&original, &replaced));
    assert!(!Arc::ptr_eq(
        &original,
        &get_registered_handler(*CustomValue::SHAPE).unwrap(),
    ));
}
