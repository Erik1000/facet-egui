use std::fmt::Display;

use egui_probe::{EguiProbe, angle};
use facet::Facet;

#[derive(EguiProbe, Facet, Default)]
pub struct DemoValue {
    just_display: OnlyDisplay,
    boolean: bool,

    #[egui_probe(toggle_switch)]
    boolean_toggle: bool,

    float: f32,

    #[egui_probe(range = 22..=55)]
    range: usize,

    #[egui_probe(as angle)]
    angle: f32,

    #[egui_probe(name = "renamed ^_^")]
    renamed: u8,

    inner: InnerValue,

    value_enum: DemoEnumInlined,
    value_enum2: DemoEnumComboBox,
    names: Vec<String>,
    semi_advanced_list: Vec<DemoEnumInlined>,
    advanced_list: Vec<InnerValue>,
}

#[derive(Default, EguiProbe, Facet)]
struct InnerValue {
    line: String,
    #[egui_probe(multiline)]
    multi_line: String,
}

#[derive(Debug, EguiProbe, Facet, Default)]
#[egui_probe(tags inlined)]
#[repr(C)]
pub enum DemoEnumInlined {
    #[default]
    Foo,
    Bar(String, u32, f32),
    Baz {
        name: u32,
    },
}

#[derive(Debug, Facet, Default, EguiProbe)]
#[facet(facet_egui::as_display)]
pub struct OnlyDisplay;

impl Display for OnlyDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "im just a display implementation")
    }
}

#[derive(Debug, EguiProbe, Facet, Default)]
#[egui_probe(tags combobox)]
#[repr(C)]
pub enum DemoEnumComboBox {
    #[default]
    Foo,
    Bar(String, u32),
    Baz {
        name: u32,
    },
}
