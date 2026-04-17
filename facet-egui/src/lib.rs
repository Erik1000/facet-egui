mod layout;
#[cfg(feature = "egui")]
use facet_maybe_mut as maybe_mut;
mod probe;
pub use maybe_mut::MaybeMut;

#[cfg(feature = "egui")]
pub use probe::{FacetProbe, MaybeMutT};

facet::define_attr_grammar! {
    ns "egui";
    crate_path ::facet_egui;

    pub enum Attr {
        /// Skips a field or type in the UI and does not display it
        Skip,
        /// Mark a field as readonly
        Readonly,
        /// Rename the field or type in the UI
        Rename(&'static str),
        /// Expand all sections recursively
        ExpandAll,
        /// Displays the underlying type using its [`Display`](core::fmt::Display) implementation.
        AsDisplay,
    }
}
