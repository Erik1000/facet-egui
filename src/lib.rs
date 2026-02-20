use std::ops::DerefMut;

use derive_more::{Deref, DerefMut, From};
use egui::Ui;
use facet::Facet;

mod maybe_mut;
use facet_reflect::{Peek, Poke};
pub use maybe_mut::MaybeMut;

/// The container that stores a [`MaybeMut`] of the type `T` that should be shown
/// in the [`Ui`](egui::Ui)
#[must_use = "use [`FacetProbe::show`] to display the probe in the [`Ui`]"]
#[derive(Deref, DerefMut)]
pub struct FacetProbe<'mem, 'facet> {
    inner: MaybeMut<'mem, 'facet>,
}

#[derive(Debug, From)]
pub enum MaybeMutT<'mem, T> {
    Not(&'mem T),
    Mut(&'mem mut T),
}

impl<'mem, 'facet> FacetProbe<'mem, 'facet> {
    pub fn new<T>(value: impl Into<MaybeMutT<'mem, T>>) -> Self
    where
        T: Facet<'facet> + 'mem,
    {
        let v: MaybeMutT<'mem, T> = value.into();
        let inner: MaybeMut = match v {
            MaybeMutT::Mut(v) => Poke::new(v).into(),
            MaybeMutT::Not(v) => Peek::new(v).into(),
        };
        Self { inner }
    }

    pub fn show(self, ui: &mut Ui) {
        match self.inner.make_mut() {
            Ok(mut mutable) => match &mut *mutable {
                MaybeMut::Mut(mutable) => {
                    let as_string: &mut String = mutable.get_mut().unwrap();
                    ui.text_edit_multiline(as_string);
                }
                _ => unreachable!(),
            },
            Err(e) => {
                ui.label("Cannot display mutable item");
                ui.label(format!("{}", e.kind));
                ui.label(format!("Readonly: {}", e.unchanged.as_str().unwrap()));
            }
        };
    }
}
