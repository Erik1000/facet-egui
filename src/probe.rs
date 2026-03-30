use std::{borrow::Cow, hash::Hash, ops::DerefMut, slice::ChunkBy};

use derive_more::{Deref, DerefMut, From};
use egui::{
    Align, Checkbox, Color32, Id, Layout, Pos2, Rect, Response, ScrollArea, TextEdit, Ui,
    UiBuilder, WidgetText,
};
use facet::{Facet, ScalarType};
use facet_reflect::{
    HasFields, Peek, PeekEnum, PeekListLike, PeekMap, PeekOption, PeekPointer, PeekResult, PeekSet,
    PeekStruct, PeekTuple, Poke, PokeEnum, PokeStruct, ReflectError,
};

use crate::{
    EguiAttr, MaybeMut,
    maybe_mut::{Guard, MakeLockErrorKind},
};

/// The container that stores a [`MaybeMut`] of the type `T` that should be shown
/// in the [`Ui`](egui::Ui)
#[must_use = "use [`FacetProbe::show`] to display the probe in the [`Ui`]"]
#[derive(Deref, DerefMut)]
pub struct FacetProbe<'mem, 'facet> {
    header: Option<WidgetText>,
    read_only: bool,
    /// SAFETY: if used, there is a high chance what you do is unsound.
    ///
    /// If you use this, you will have to manually ensure your variances are
    /// okay for use with reborrowing. Normally, this is determined by facet but
    /// there may be cases where a type does not implement Facet or has opaque
    /// parts that would (if used with facet) be ok.
    force_reborrow: bool,
    #[deref]
    #[deref_mut]
    inner: MaybeMut<'mem, 'facet>,
}

#[derive(Debug, From)]
pub enum MaybeMutT<'mem, T> {
    Not(&'mem T),
    Mut(&'mem mut T),
}

impl<'mem, 'facet> FacetProbe<'mem, 'facet> {
    pub fn readonly(self) -> Self {
        Self {
            read_only: true,
            ..self
        }
    }

    pub fn with_header(mut self, label: impl Into<WidgetText>) -> Self {
        self.header = Some(label.into());
        self
    }

    /// # Safety
    ///
    /// If used, there is a high chance what you do is unsound.
    ///
    /// If you use this, you will have to manually ensure your variances are
    /// okay for use with reborrowing. Normally, this is determined by facet but
    /// there may be cases where a type does not implement Facet or has opaque
    /// parts that would (if used with facet) be ok.
    pub unsafe fn force_reborrow(self) -> Self {
        Self {
            force_reborrow: true,
            ..self
        }
    }

    pub fn new_peek(value: Peek<'mem, 'facet>) -> Self {
        Self {
            header: None,
            read_only: true,
            force_reborrow: false,
            inner: MaybeMut::Not(value),
        }
    }

    pub fn new_poke(value: Poke<'mem, 'facet>) -> Self {
        Self {
            header: None,
            read_only: false,
            force_reborrow: false,
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
        Self {
            header: None,
            read_only: false,
            force_reborrow: false,
            inner,
        }
    }

    pub fn show<'lock>(self, ui: &mut Ui) -> Response
    where
        'mem: 'lock,
    {
        let mut changed = false;

        let mut attributes = self
            .shape()
            .attributes
            .iter()
            .filter_map(|x| x.get_as::<crate::EguiAttr>());
        let readonly = attributes.any(|x| matches!(x, EguiAttr::Readonly)) || self.read_only;
        let mut guard: Guard<'lock, 'facet> = if readonly {
            let Ok(read) = self.inner.read() else {
                return ui.colored_label(Color32::RED, "Read Failure");
            };
            read
        } else {
            match self.inner.write() {
                Ok(write) => write,
                // fallback to readonly
                Err(e) if matches!(e.kind, MakeLockErrorKind::NotLockable) => {
                    let Ok(read) = MaybeMut::Not(e.unchanged).read() else {
                        return ui.colored_label(Color32::RED, "Cannot display readonly value");
                    };
                    read
                }
                Err(e) if matches!(e.kind, MakeLockErrorKind::LockFailure) => {
                    return ui.colored_label(Color32::RED, "Lock Failure");
                }
                Err(e) => {
                    return ui.colored_label(Color32::RED, format!("Error: {e}"));
                }
            }
        };
        // lifetime of borrow lives at most as long as 'lock
        let maybe_mut = guard.deref_mut();
        let mut r = ui.allocate_ui(ui.available_size(), |ui| {
            let child_ui = &mut ui.new_child(
                UiBuilder::new()
                    .max_rect(ui.max_rect())
                    .layout(Layout::top_down(Align::Min)),
            );
            let id = child_ui.next_auto_id();
            match maybe_mut {
                MaybeMut::Mut(m) => {
                    let poke = match m.try_reborrow() {
                        Some(poke) => poke,
                        None if self.force_reborrow => {
                            // SAFETY: this is unsound, see Self.force_reborrow for details
                            unsafe { Poke::from_raw_parts(m.data_mut(), m.shape()) }
                        }
                        None => {
                            ui.colored_label(Color32::RED, "Cannot reborrow");
                            return;
                        }
                    };
                    Self::show_poke(poke, child_ui);
                }
                MaybeMut::Not(n) => {
                    // works because Peek implements Copy
                    Self::show_peek(*n, child_ui, id);
                }
            };
        });
        drop(guard);
        if changed {
            ui.ctx().request_repaint();
        }
        ui.response()
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
        let poke = poke.into_inner();
        let name = poke.shape().effective_name();
        let mut poke = poke
            .into_struct()
            .expect("valid it was a poke struct before");
        // FIXME: get struct name somehow
        ui.label(name);
        ScrollArea::both()
            .id_salt(ui.next_auto_id())
            .show(ui, |ui| {
                for field_idx in 0..poke.field_count() {
                    let field_name = poke.ty().fields[field_idx].effective_name();
                    let field = poke.field(field_idx);
                    if let Ok(field) = field {
                        ui.horizontal(|ui| {
                            ui.label(field_name);
                            Self::show_poke(field, ui);
                        });
                    } else {
                        ui.colored_label(Color32::RED, "field error");
                    }
                }
            });
    }
}

/// [`Peek`] / readonly implementation
impl FacetProbe<'_, '_> {
    fn show_peek(peek: Peek<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        if let Some(scalar_type) = peek.scalar_type() {
            Self::show_peek_scalar(peek, scalar_type, ui, id).expect("casting works everywhere")
            // handle scalar type
        } else if let Ok(enu) = peek.into_enum() {
            Self::show_peek_enum(enu, ui, id)
        } else if let Ok(list) = peek.into_list_like() {
            Self::show_peek_list(list, ui, id)
        } else if let Ok(map) = peek.into_map() {
            Self::show_peek_map(map, ui, id)
        } else if let Ok(option) = peek.into_option() {
            Self::show_peek_option(option, ui, id)
        } else if let Ok(pointer) = peek.into_pointer() {
            Self::show_peek_pointer(pointer, ui, id)
        } else if let Ok(result) = peek.into_result() {
            Self::show_peek_result(result, ui, id)
        } else if let Ok(set) = peek.into_set() {
            Self::show_peek_set(set, ui, id)
        } else if let Ok(struc) = peek.into_struct() {
            Self::show_peek_struct(struc, ui, id)
        } else if let Ok(tuple) = peek.into_tuple() {
            Self::show_peek_tuple(tuple, ui, id)
        } else {
            ui.colored_label(Color32::RED, "Unsupported Peek type")
        }
    }

    fn show_peek_scalar(
        peek: Peek<'_, '_>,
        scalar_type: ScalarType,
        ui: &mut Ui,
        _id: Id,
    ) -> Result<Response, ReflectError> {
        Ok(match scalar_type {
            ScalarType::Bool => {
                // this is only marked mutable to satisfy the function signature.
                // the value is not actually updated since it is copied before
                // ui interaction is disabled because it is readonly
                let mut value = *peek.get::<bool>()?;
                ui.add_enabled(false, Checkbox::without_text(&mut value))
            }
            ScalarType::Char => {
                // TODO: this allocates a String with one char each render which is inefficent
                let c = peek.get::<char>()?.to_string();
                ui.add_enabled(false, TextEdit::singleline(&mut c.as_str()))
            }
            ScalarType::Str => {
                let mut value = peek.get::<str>()?;
                // TODO: how to decide if multiline or single line?
                ui.add_enabled(false, TextEdit::multiline(&mut value))
            }
            ScalarType::CowStr => {
                let mut value = peek.get::<Cow<'_, str>>()?.clone();
                ui.add_enabled(false, TextEdit::multiline(&mut value))
            }
            ScalarType::String => {
                let mut value: Cow<'_, str> = Cow::Borrowed(peek.get::<String>()?.as_str());
                ui.add_enabled(false, TextEdit::multiline(&mut value))
            }
            // fallback to display implementation if the type has one
            _ if peek.shape().is_display() => ui.label(format!("{}", peek)),
            // or fallback to the debug implementation if the type has one
            _ if peek.shape().is_debug() => ui.label(format!("{:?}", peek)),
            _ => ui.label(format!("Cannot display scalar type: {peek}")),
        })
    }

    fn show_peek_enum(peek: PeekEnum<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        let mut r = ui.response();
        ui.horizontal(|ui| {
            if let Ok(variant) = peek.active_variant() {
                if ui
                    .selectable_label(false, variant.effective_name())
                    .clicked()
                {
                    // TODO: show field selection
                }
                ui.end_row();
                r = ui
                    .vertical(|ui| {
                        for (field, value) in peek.fields() {
                            ui.weak(field.effective_name());
                            FacetProbe::new_peek(value).show(ui);
                            ui.spacing();
                        }
                    })
                    .response;
            }
        });
        r
    }

    fn show_peek_list(peek: PeekListLike<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        ui.horizontal(|ui| {
            ui.weak(format!("[{}]", peek.len()));
        });
        ui.vertical(|ui| {
            for (idx, item) in peek.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("[{idx}]"));
                    ui.spacing();
                    Self::show_peek(item, ui, id.with(idx));
                });
            }
        })
        .response
    }

    fn show_peek_map(peek: PeekMap<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        ui.weak(format!("[{}]", peek.len()));
        ui.vertical(|ui| {
            for (idx, (key, value)) in peek.iter().enumerate() {
                Self::show_peek(key, ui, id.with((idx, "key")));
                ui.spacing();
                Self::show_peek(value, ui, id.with((idx, "value")));
            }
        })
        .response
    }

    fn show_peek_option(peek: PeekOption<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        if let Some(value) = peek.value() {
            Self::show_peek(value, ui, id.with("some"))
        } else {
            ui.label("None")
        }
    }

    fn show_peek_pointer(peek: PeekPointer<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        if let Some(borrow) = peek.borrow_inner() {
            Self::show_peek(borrow, ui, id.with("ptr"))
        } else {
            ui.label(format!("Pointer not borrowable: {:?}", peek.def().known))
        }
    }

    fn show_peek_result(peek: PeekResult<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        if let Some(ok) = peek.ok() {
            ui.colored_label(Color32::GREEN, "Ok:");
            ui.spacing();
            Self::show_peek(ok, ui, id.with("ok"))
        } else if let Some(err) = peek.err() {
            ui.colored_label(Color32::RED, "Error:");
            ui.spacing();
            Self::show_peek(err, ui, id.with("err"))
        } else {
            ui.response()
        }
    }

    fn show_peek_set(peek: PeekSet<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        ui.weak(format!("[{}]", peek.len()));
        ui.vertical(|ui| {
            for (idx, item) in peek.iter().enumerate() {
                Self::show_peek(item, ui, id.with(idx));
            }
        })
        .response
    }

    fn show_peek_struct(peek: PeekStruct<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        ui.vertical(|ui| {
            for (idx, (field, value)) in peek.fields().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(field.effective_name());
                    Self::show_peek(value, ui, id.with(idx));
                });
            }
        })
        .response
    }

    fn show_peek_tuple(peek: PeekTuple<'_, '_>, ui: &mut Ui, id: Id) -> Response {
        ui.vertical(|ui| {
            for (idx, (_field, value)) in peek.fields().enumerate() {
                Self::show_peek(value, ui, id.with(idx));
                ui.spacing();
            }
        })
        .response
    }
}
