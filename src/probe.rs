use std::{borrow::Cow, fmt::Formatter, ops::DerefMut};

use derive_more::{Deref, DerefMut, From};
use egui::{Checkbox, Color32, TextEdit, Ui};
use facet::{Facet, ScalarType};
use facet_reflect::{
    HasFields, Peek, PeekEnum, PeekListLike, PeekMap, PeekOption, PeekPointer, PeekResult, PeekSet,
    PeekStruct, PeekTuple, Poke, PokeEnum, PokeStruct, ReflectError,
};

use crate::{EguiAttr, MaybeMut, maybe_mut::Guard};

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
    pub fn new_peek(value: Peek<'mem, 'facet>) -> Self {
        Self {
            inner: MaybeMut::Not(value),
        }
    }

    pub fn new_poke(value: Poke<'mem, 'facet>) -> Self {
        Self {
            inner: MaybeMut::Mut(value),
        }
    }

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

    pub fn show<'lock>(self, ui: &mut Ui)
    where
        'mem: 'lock,
    {
        let mut attributes = self
            .shape()
            .attributes
            .iter()
            .filter_map(|x| x.get_as::<crate::EguiAttr>());
        let readonly = attributes.any(|x| matches!(x, EguiAttr::Readonly));
        let mut guard: Guard<'lock, 'facet> = if readonly {
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
        // lifetime of borrow lives at most as long as 'lock
        let maybe_mut = guard.deref_mut();
        match maybe_mut {
            MaybeMut::Mut(m) => {
                let Some(poke) = m.try_reborrow() else {
                    ui.colored_label(Color32::RED, "Cannot reborrow");
                    return;
                };
                Self::show_poke(poke, ui);
            }
            MaybeMut::Not(n) => {
                // works because Peek implements Copy
                Self::show_peek(*n, ui);
            }
        };
        drop(guard);
    }
}

impl FacetProbe<'_, '_> {
    fn show_poke(mut poke: Poke<'_, '_>, ui: &mut Ui) {
        if !poke.is_scalar() {
            // continue unwrapping until we find a scalar that we can display
            if poke.is_enum() {
                Self::poke_enum(poke.into_enum().unwrap(), ui);
            } else if poke.is_struct() {
                Self::poke_struct(poke.into_struct().unwrap(), ui);
            } else {
                ui.colored_label(Color32::YELLOW, "Unsupported poke type");
            }
        } else {
            // TODO: try out all known scalar types? seems stupid
            if let Ok(s) = poke.get_mut::<String>() {
                ui.text_edit_singleline(s);
            }
        }
    }

    fn poke_enum(poke: PokeEnum<'_, '_>, ui: &mut Ui) {
        for variant in poke.variants() {
            let checked =
                poke.active_variant().unwrap().effective_name() == variant.effective_name();
            if ui
                .selectable_label(checked, variant.effective_name())
                .clicked()
                && !checked
            {
                // TODO: handle setting enum variant
                // this could get complicating if the enum has variants with fields
                // perhaps we can use Partial and some custom constructor?
            }
        }
    }

    fn poke_struct(poke: PokeStruct<'_, '_>, ui: &mut Ui) {
        ui.label("Struct");
        let fields: Vec<_> = poke
            .as_peek_struct()
            .fields()
            //.map(|(field, _peek)| field)
            .collect();
        for (field, peek) in fields {
            ui.label(field.effective_name());
            // FIXME: this is unsound and i just wanted to try it out lol
            // let poke = unsafe { Poke::from_raw_parts(peek.data().into_mut(), peek.shape()) };
            // Self::show_poke(poke, ui);
        }
    }
}

/// [`Peek`] / readonly implementation
impl FacetProbe<'_, '_> {
    fn show_peek(peek: Peek<'_, '_>, ui: &mut Ui) {
        if let Some(scalar_type) = peek.scalar_type() {
            Self::show_peek_scalar(peek, scalar_type, ui).expect("casting works everywhere")
            // handle scalar type
        } else if let Ok(enu) = peek.into_enum() {
            Self::show_peek_enum(enu, ui);
        } else if let Ok(list) = peek.into_list_like() {
            Self::show_peek_list(list, ui);
        } else if let Ok(map) = peek.into_map() {
            Self::show_peek_map(map, ui);
        } else if let Ok(option) = peek.into_option() {
            Self::show_peek_option(option, ui);
        } else if let Ok(pointer) = peek.into_pointer() {
            Self::show_peek_pointer(pointer, ui);
        } else if let Ok(result) = peek.into_result() {
            Self::show_peek_result(result, ui);
        } else if let Ok(set) = peek.into_set() {
            Self::show_peek_set(set, ui);
        } else if let Ok(struc) = peek.into_struct() {
            Self::show_peek_struct(struc, ui);
        } else if let Ok(tuple) = peek.into_tuple() {
            Self::show_peek_tuple(tuple, ui);
        } else {
            ui.colored_label(Color32::RED, "Unsupported Peek type");
        }
    }

    fn show_peek_scalar(
        peek: Peek<'_, '_>,
        scalar_type: ScalarType,
        ui: &mut Ui,
    ) -> Result<(), ReflectError> {
        match scalar_type {
            ScalarType::Bool => {
                // this is only marked mutable to satisfy the function signature.
                // the value is not actually updated since it is copied before
                // ui interaction is disabled because it is readonly
                let mut value = *peek.get::<bool>()?;
                ui.add_enabled(false, Checkbox::without_text(&mut value));
            }
            ScalarType::Char => {
                // TODO: this allocates a String with one char each render which is inefficent
                let c = peek.get::<char>()?.to_string();
                ui.add_enabled(false, TextEdit::singleline(&mut c.as_str()));
            }
            ScalarType::Str => {
                let mut value = peek.get::<str>()?;
                // TODO: how to decide if multiline or single line?
                ui.add_enabled(false, TextEdit::multiline(&mut value));
            }
            ScalarType::CowStr => {
                let mut value = peek.get::<Cow<'_, str>>()?.clone();
                ui.add_enabled(false, TextEdit::multiline(&mut value));
            }
            ScalarType::String => {
                let mut value: Cow<'_, str> = Cow::Borrowed(peek.get::<String>()?.as_str());
                ui.add_enabled(false, TextEdit::multiline(&mut value));
            }
            // fallback to display implementation if the type has one
            _ if peek.shape().is_display() => {
                ui.label(format!("{}", peek));
            }
            // or fallback to the debug implementation if the type has one
            _ if peek.shape().is_debug() => {
                ui.label(format!("{:?}", peek));
            }
            _ => {
                ui.label(format!("Cannot display scalar type: {peek}"));
            }
        };
        Ok(())
    }

    fn show_peek_enum(peek: PeekEnum<'_, '_>, ui: &mut Ui) {
        ui.vertical(|ui| {
            if let Ok(variant) = peek.active_variant() {
                ui.label(variant.effective_name());
            }

            for (field, value) in peek.fields() {
                ui.weak(field.effective_name());
                Self::show_peek(value, ui);
                ui.spacing();
            }
        });
    }

    fn show_peek_list(peek: PeekListLike<'_, '_>, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.weak(format!("[{}]", peek.len()));
        });
        ui.vertical(|ui| {
            for (idx, item) in peek.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("[{idx}]"));
                    ui.spacing();
                    Self::show_peek(item, ui);
                });
            }
        });
    }

    fn show_peek_map(peek: PeekMap<'_, '_>, ui: &mut Ui) {
        ui.weak(format!("[{}]", peek.len()));
        ui.vertical(|ui| {
            for (key, value) in peek.iter() {
                Self::show_peek(key, ui);
                ui.spacing();
                Self::show_peek(value, ui);
            }
        });
    }

    fn show_peek_option(peek: PeekOption<'_, '_>, ui: &mut Ui) {
        if let Some(value) = peek.value() {
            Self::show_peek(value, ui);
        } else {
            ui.label("None");
        }
    }

    fn show_peek_pointer(peek: PeekPointer<'_, '_>, ui: &mut Ui) {
        if let Some(borrow) = peek.borrow_inner() {
            Self::show_peek(borrow, ui);
        } else {
            ui.label(format!("Pointer not borrowable: {:?}", peek.def().known));
        }
    }

    fn show_peek_result(peek: PeekResult<'_, '_>, ui: &mut Ui) {
        if let Some(ok) = peek.ok() {
            ui.colored_label(Color32::GREEN, "Ok:");
            ui.spacing();
            Self::show_peek(ok, ui);
        } else if let Some(err) = peek.err() {
            ui.colored_label(Color32::RED, "Error:");
            ui.spacing();
            Self::show_peek(err, ui);
        }
    }

    fn show_peek_set(peek: PeekSet<'_, '_>, ui: &mut Ui) {
        ui.weak(format!("[{}]", peek.len()));
        ui.vertical(|ui| {
            for item in peek.iter() {
                Self::show_peek(item, ui);
            }
        });
    }

    fn show_peek_struct(peek: PeekStruct<'_, '_>, ui: &mut Ui) {
        ui.vertical(|ui| {
            for (_field, value) in peek.fields() {
                Self::show_peek(value, ui);
                ui.spacing();
            }
        });
    }

    fn show_peek_tuple(peek: PeekTuple<'_, '_>, ui: &mut Ui) {
        ui.vertical(|ui| {
            for (_field, value) in peek.fields() {
                Self::show_peek(value, ui);
                ui.spacing();
            }
        });
    }
}
