#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod layout;
use facet_maybe_mut::{self as maybe_mut};

mod probe;
pub use facet_maybe_mut::MaybeMut;
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

#[cfg(feature = "custom_ui")]
pub use custom::{
    UiClosure, UiHandler, get_registered_handler, register_custom_ui, register_custom_ui_shape,
};

/// Allows registering custom UI handlers for shapes
#[cfg(feature = "custom_ui")]
mod custom {
    use std::{collections::HashMap, sync::LazyLock};

    use alloc::{boxed::Box, sync::Arc};
    use egui::Ui;
    use egui::mutex::{Mutex, RwLock};
    use facet::{Facet, Shape};
    use facet_maybe_mut::Guard;

    /// The type of function your custom ui handler must look like
    pub type UiClosure = dyn for<'a, 'facet, 'ui> FnMut(Guard<'a, 'facet>, &'ui mut Ui) -> egui::Response
        + Send
        + 'static;

    /// A shared handler with exclusive access to its mutable closure state.
    /// Calling the same handler recursively while locked can deadlock.
    pub type UiHandler = Arc<Mutex<Box<UiClosure>>>;

    type CustomUiStore = RwLock<HashMap<Shape, UiHandler>>;

    /// Stores the custom ui handler pointers
    static REGISTERED_CUSTOM_UIS: LazyLock<CustomUiStore> =
        LazyLock::new(|| RwLock::new(HashMap::new()));

    /// For some Shape of T register custom ui code that will be called with
    /// the value of T
    pub fn register_custom_ui<'facet, T: Facet<'facet>>(
        handler: impl for<'a, 'value, 'ui> FnMut(Guard<'a, 'value>, &'ui mut Ui) -> egui::Response
        + Send
        + 'static,
    ) -> Option<UiHandler> {
        register_custom_ui_shape(*T::SHAPE, Box::new(handler))
    }

    /// Given a Shape, register custom ui code that is called when it is
    /// encountered during a probe
    pub fn register_custom_ui_shape(shape: Shape, handler: Box<UiClosure>) -> Option<UiHandler> {
        REGISTERED_CUSTOM_UIS
            .write()
            .insert(shape, Arc::new(Mutex::new(handler)))
    }

    /// Clones the handler handle, releasing the registry lock before invocation.
    pub fn get_registered_handler(shape: Shape) -> Option<UiHandler> {
        REGISTERED_CUSTOM_UIS.read().get(&shape).cloned()
    }
}
