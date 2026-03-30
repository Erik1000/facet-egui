mod layout;
mod maybe_mut;
#[cfg(feature = "egui")]
mod probe;
mod state;
pub use maybe_mut::MaybeMut;

#[cfg(feature = "egui")]
pub use probe::{FacetProbe, MaybeMutT};

facet::define_attr_grammar! {
    ns "egui";
    crate_path ::facet_egui;

    pub enum EguiAttr {
        /// Mark a field as readonly
        Readonly,
    }
}
