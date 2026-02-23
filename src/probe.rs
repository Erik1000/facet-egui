use derive_more::{Deref, DerefMut, From};
use egui::{Color32, Ui};
use facet::Facet;
use facet_reflect::{Peek, Poke};

use crate::{EguiAttr, MaybeMut};

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
        let mut attributes = self
            .shape()
            .attributes
            .iter()
            .filter_map(|x| x.get_as::<crate::EguiAttr>());
        let readonly = attributes.any(|x| matches!(x, EguiAttr::Readonly));
        let mut guard = if readonly {
            let Ok(read) = self.inner.read() else {
                ui.colored_label(Color32::RED, "Cannot display readonly value");
                return;
            };
            read
        } else {
            let Ok(write) = self.inner.write() else {
                ui.colored_label(Color32::RED, "Cannot display writable value");
                return;
            };
            write
        };
        match &mut *guard {
            MaybeMut::Mut(m) => {
                // FIXME: need something like PeekMut
                //Self::show_poke(m);
            }
            MaybeMut::Not(n) => {
                // works because Peek implements Copy
                Self::show_peek(*n);
            }
        };
        drop(guard);
    }

    fn show_peek(peek: Peek<'_, '_>) {
        // TODO
    }
    fn show_poke(poke: Poke<'_, '_>) {
        // TODO
    }
}
