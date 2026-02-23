mod maybe_mut;
mod probe;
pub use maybe_mut::MaybeMut;

pub use probe::{FacetProbe, MaybeMutT};

facet::define_attr_grammar! {
    ns "egui";
    crate_path ::facet_egui;

    pub enum EguiAttr {
        /// Mark a field as readonly
        Readonly,
    }
}
