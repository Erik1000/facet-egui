use std::{borrow::Cow, ops::DerefMut};

use derive_more::{Deref, DerefMut as DeriveDerefMut, From};
use egui::{Align, Checkbox, Color32, Id, Layout, Response, TextEdit, Ui, UiBuilder, WidgetText};
use facet::{Def, Facet, ListDef, ScalarType, Type, UserType};
use facet_reflect::{
    HasFields, Partial, Peek, PeekEnum, PeekListLike, PeekMap, PeekOption, PeekPointer, PeekStruct,
    PeekTuple, Poke, PokeEnum, PokeList, PokeStruct,
};

use crate::{
    EguiAttr, MaybeMut,
    layout::{ProbeHeader, ProbeLayout},
    maybe_mut::{Guard, MakeLockErrorKind},
};

/// The container that stores a [`MaybeMut`] of the type `T` that should be shown
/// in the [`Ui`](egui::Ui)
#[must_use = "use [`FacetProbe::show`] to display the probe in the [`Ui`]"]
#[derive(Deref, DeriveDerefMut)]
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

        let maybe_mut = guard.deref_mut();
        let mut r = ui
            .allocate_ui(ui.available_size(), |ui| {
                let child_ui = &mut ui.new_child(
                    UiBuilder::new()
                        .max_rect(ui.max_rect())
                        .layout(Layout::top_down(Align::Min)),
                );
                let id = child_ui.next_auto_id();

                let mut layout = ProbeLayout::load(child_ui.ctx(), id);

                if let Some(label) = self.header {
                    // Show with a top-level header (like Probe::new(x).with_header("name"))
                    let mut header = show_header(
                        label,
                        maybe_mut,
                        &mut layout,
                        0,
                        child_ui,
                        id,
                        &mut changed,
                        self.force_reborrow,
                    );

                    if header.openness > 0.0 {
                        show_body(
                            maybe_mut,
                            &mut header,
                            &mut layout,
                            0,
                            child_ui,
                            id,
                            &mut changed,
                            self.force_reborrow,
                        );
                    } else {
                        header.set_has_inner(has_inner(maybe_mut));
                    }

                    header.store(child_ui.ctx());
                } else {
                    // Show directly without a top-level header (table of fields)
                    show_body_direct(
                        maybe_mut,
                        &mut layout,
                        0,
                        child_ui,
                        id,
                        &mut changed,
                        self.force_reborrow,
                    );
                }

                layout.store(child_ui.ctx());

                let final_rect = child_ui.min_rect();
                ui.advance_cursor_after_rect(final_rect);
            })
            .response;

        drop(guard);

        if changed {
            r.mark_changed();
            ui.ctx().request_repaint();
        }

        r
    }
}

// ---------------------------------------------------------------------------
// Core layout functions (egui-probe style)
// ---------------------------------------------------------------------------

/// Returns true if the given `MaybeMut` has inner fields/items to display
/// (i.e. it should get a collapse arrow).
fn has_inner(value: &MaybeMut<'_, '_>) -> bool {
    let peek = value.as_peek();
    // Structs with fields, enums with variant fields, lists, maps, options
    // with inner, tuples, sets, pointers to inner — all have inner content.
    if let Ok(s) = peek.into_struct() {
        return s.field_count() > 0;
    }
    if let Ok(e) = peek.into_enum()
        && let Ok(v) = e.active_variant()
    {
        return !v.data.fields.is_empty();
    }
    if let Ok(l) = peek.into_list_like() {
        return !l.is_empty();
    }
    if let Ok(m) = peek.into_map() {
        return !m.is_empty();
    }
    if let Ok(opt) = peek.into_option()
        && let Some(inner) = opt.value()
    {
        return has_inner(&MaybeMut::Not(inner));
    }
    if let Ok(t) = peek.into_tuple() {
        return !t.is_empty();
    }
    if let Ok(p) = peek.into_pointer()
        && let Some(inner) = p.borrow_inner()
    {
        return has_inner(&MaybeMut::Not(inner));
    }
    false
}

/// Show a single row: label on the left, inline value widget on the right.
/// Returns the ProbeHeader for the row (which tracks collapse state).
#[expect(clippy::too_many_arguments)]
fn show_header(
    label: impl Into<WidgetText>,
    value: &mut MaybeMut<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + Copy,
    changed: &mut bool,
    force_reborrow: bool,
) -> ProbeHeader {
    let id = ui.make_persistent_id(id_salt);
    let mut header = ProbeHeader::load(ui.ctx(), id);

    ui.horizontal(|ui| {
        let label_response = layout.inner_label_ui(indent, id.with("label"), ui, |ui| {
            if header.has_inner() {
                header.collapse_button(ui);
            }
            ui.label(label)
        });

        layout.inner_value_ui(id.with("value"), ui, |ui| {
            *changed |= show_inline_value(value, ui, id, force_reborrow)
                .labelled_by(label_response.id)
                .changed();
        });
    });

    header
}

/// Show the collapsible body (the inner fields/items) below a header row.
#[expect(clippy::too_many_arguments)]
fn show_body(
    value: &mut MaybeMut<'_, '_>,
    header: &mut ProbeHeader,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + Copy,
    changed: &mut bool,
    force_reborrow: bool,
) {
    let cursor = ui.cursor();
    let table_rect = egui::Rect::from_min_max(
        egui::pos2(cursor.min.x, cursor.min.y - header.body_shift()),
        ui.max_rect().max,
    );

    let mut table_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(table_rect)
            .layout(Layout::top_down(Align::Min))
            .id_salt(id_salt),
    );
    table_ui.set_clip_rect(
        ui.clip_rect()
            .intersect(egui::Rect::everything_below(ui.min_rect().max.y)),
    );

    let got_inner = show_inner_rows(
        value,
        layout,
        indent + 1,
        &mut table_ui,
        changed,
        force_reborrow,
    );
    header.set_has_inner(got_inner);

    let final_table_rect = table_ui.min_rect();
    ui.advance_cursor_after_rect(final_table_rect);
    let table_height = ui.cursor().min.y - table_rect.min.y;
    header.set_body_height(table_height);
}

/// Show the body directly (no collapse header wrapper). Used when there is no
/// top-level header.
fn show_body_direct(
    value: &mut MaybeMut<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + Copy,
    changed: &mut bool,
    force_reborrow: bool,
) {
    let cursor = ui.cursor();
    let table_rect =
        egui::Rect::from_min_max(egui::pos2(cursor.min.x, cursor.min.y), ui.max_rect().max);

    let mut table_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(table_rect)
            .layout(Layout::top_down(Align::Min))
            .id_salt(id_salt),
    );
    table_ui.set_clip_rect(
        ui.clip_rect()
            .intersect(egui::Rect::everything_below(ui.min_rect().max.y)),
    );

    show_inner_rows(
        value,
        layout,
        indent + 1,
        &mut table_ui,
        changed,
        force_reborrow,
    );

    let final_table_rect = table_ui.min_rect();
    ui.advance_cursor_after_rect(final_table_rect);
}

/// Iterate over the "inner" rows of a value and render each as a header+body pair.
/// Returns `true` if any inner rows were emitted.
fn show_inner_rows(
    value: &mut MaybeMut<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
) -> bool {
    // We dispatch on type shape and enumerate children.
    // Each child becomes a (label, value) header row that can itself be collapsed.

    match value {
        MaybeMut::Mut(poke) => {
            show_inner_rows_poke(poke, layout, indent, ui, changed, force_reborrow)
        }
        MaybeMut::Not(peek) => show_inner_rows_peek(*peek, layout, indent, ui, changed),
    }
}

fn show_inner_rows_poke(
    poke: &mut Poke<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
) -> bool {
    // For enums, we can use into_enum directly without reborrowing,
    // since PokeEnum.field() takes &mut self.
    if poke.is_enum() {
        let enu_poke = match poke.try_reborrow() {
            Some(rb) => rb,
            None if force_reborrow => unsafe {
                Poke::from_raw_parts(poke.data_mut(), poke.shape())
            },
            None => {
                return show_inner_rows_peek(poke.as_peek(), layout, indent, ui, changed);
            }
        };
        if let Ok(enu) = enu_poke.into_enum() {
            return show_inner_rows_poke_enum(enu, layout, indent, ui, changed, force_reborrow);
        }
        return show_inner_rows_peek(poke.as_peek(), layout, indent, ui, changed);
    }

    // For structs, reborrow to get mutable field access
    if poke.is_struct() {
        let reborrow = match poke.try_reborrow() {
            Some(rb) => rb,
            None if force_reborrow => unsafe {
                Poke::from_raw_parts(poke.data_mut(), poke.shape())
            },
            None => {
                return show_inner_rows_peek(poke.as_peek(), layout, indent, ui, changed);
            }
        };
        if let Ok(struc) = reborrow.into_struct() {
            return show_inner_rows_poke_struct(struc, layout, indent, ui, changed, force_reborrow);
        }
    }

    let data_mut = poke.data_mut();
    let shape = poke.shape();
    let poke = match poke.try_reborrow() {
        Some(rb) => rb,
        None if force_reborrow => unsafe { Poke::from_raw_parts(poke.data_mut(), poke.shape()) },
        None => return show_inner_rows_peek(poke.as_peek(), layout, indent, ui, changed),
    };
    if let Ok(poke_list) = poke.into_list() {
        show_inner_rows_poke_list(poke_list, layout, indent, ui, changed, force_reborrow)
    } else {
        // restore old poke
        // SAFETY: this is ok because there still is only one access to poke due to the if
        // branch not being reached
        let poke = unsafe { Poke::from_raw_parts(data_mut, shape) };
        // For list, map, tuple, option, pointer — fall through to peek
        show_inner_rows_peek(poke.as_peek(), layout, indent, ui, changed)
    }
}

fn show_inner_rows_poke_list(
    mut list: PokeList<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
) -> bool {
    let len = list.len();
    if len == 0 {
        return false;
    }
    for idx in 0..len {
        let label = format!("[{idx}]");
        if let Some(field_poke) = list.get_mut(idx) {
            let mut child = MaybeMut::Mut(field_poke);
            let mut header = show_header(
                &label,
                &mut child,
                layout,
                indent,
                ui,
                idx,
                changed,
                force_reborrow,
            );
            if header.openness > 0.0 {
                show_body(
                    &mut child,
                    &mut header,
                    layout,
                    indent,
                    ui,
                    idx,
                    changed,
                    force_reborrow,
                );
            } else {
                header.set_has_inner(has_inner(&child));
            }
            header.store(ui.ctx());
        }
    }
    true
}

fn show_inner_rows_poke_struct(
    mut struc: PokeStruct<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
) -> bool {
    let count = struc.field_count();
    if count == 0 {
        return false;
    }
    for idx in 0..count {
        let field_name = struc.ty().fields[idx].effective_name().to_owned();
        if let Ok(field_poke) = struc.field(idx) {
            let mut child = MaybeMut::Mut(field_poke);
            let mut header = show_header(
                &field_name,
                &mut child,
                layout,
                indent,
                ui,
                idx,
                changed,
                force_reborrow,
            );
            if header.openness > 0.0 {
                show_body(
                    &mut child,
                    &mut header,
                    layout,
                    indent,
                    ui,
                    idx,
                    changed,
                    force_reborrow,
                );
            } else {
                header.set_has_inner(has_inner(&child));
            }
            header.store(ui.ctx());
        }
    }
    true
}

fn show_inner_rows_poke_enum(
    mut enu: PokeEnum<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
) -> bool {
    let variant = match enu.active_variant() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let field_count = variant.data.fields.len();
    if field_count == 0 {
        return false;
    }
    for idx in 0..field_count {
        let field_name = variant.data.fields[idx].effective_name().to_owned();
        if let Ok(Some(field_poke)) = enu.field(idx) {
            let mut child = MaybeMut::Mut(field_poke);
            let mut header = show_header(
                &field_name,
                &mut child,
                layout,
                indent,
                ui,
                idx,
                changed,
                force_reborrow,
            );
            if header.openness > 0.0 {
                show_body(
                    &mut child,
                    &mut header,
                    layout,
                    indent,
                    ui,
                    idx,
                    changed,
                    force_reborrow,
                );
            } else {
                header.set_has_inner(has_inner(&child));
            }
            header.store(ui.ctx());
        }
    }
    true
}

fn show_inner_rows_peek(
    peek: Peek<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
) -> bool {
    if let Ok(struc) = peek.into_struct() {
        show_inner_rows_peek_struct(struc, layout, indent, ui, changed)
    } else if let Ok(enu) = peek.into_enum() {
        show_inner_rows_peek_enum(enu, layout, indent, ui, changed)
    } else if let Ok(list) = peek.into_list_like() {
        show_inner_rows_peek_list(list, layout, indent, ui, changed)
    } else if let Ok(map) = peek.into_map() {
        show_inner_rows_peek_map(map, layout, indent, ui, changed)
    } else if let Ok(opt) = peek.into_option() {
        show_inner_rows_peek_option(opt, layout, indent, ui, changed)
    } else if let Ok(tuple) = peek.into_tuple() {
        show_inner_rows_peek_tuple(tuple, layout, indent, ui, changed)
    } else if let Ok(ptr) = peek.into_pointer() {
        show_inner_rows_peek_pointer(ptr, layout, indent, ui, changed)
    } else {
        false
    }
}

fn show_inner_rows_peek_struct(
    struc: PeekStruct<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
) -> bool {
    let mut got_inner = false;
    for (idx, (field, value)) in struc.fields().enumerate() {
        got_inner = true;
        let field_name = field.effective_name().to_owned();
        let mut child = MaybeMut::Not(value);
        let mut header = show_header(
            &field_name,
            &mut child,
            layout,
            indent,
            ui,
            idx,
            changed,
            false,
        );
        if header.openness > 0.0 {
            show_body(
                &mut child,
                &mut header,
                layout,
                indent,
                ui,
                idx,
                changed,
                false,
            );
        } else {
            header.set_has_inner(has_inner(&child));
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_enum(
    enu: PeekEnum<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
) -> bool {
    let mut got_inner = false;
    for (idx, (field, value)) in enu.fields().enumerate() {
        got_inner = true;
        let field_name = field.effective_name().to_owned();
        let mut child = MaybeMut::Not(value);
        let mut header = show_header(
            &field_name,
            &mut child,
            layout,
            indent,
            ui,
            idx,
            changed,
            false,
        );
        if header.openness > 0.0 {
            show_body(
                &mut child,
                &mut header,
                layout,
                indent,
                ui,
                idx,
                changed,
                false,
            );
        } else {
            header.set_has_inner(has_inner(&child));
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_list(
    list: PeekListLike<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
) -> bool {
    let mut got_inner = false;
    for (idx, item) in list.iter().enumerate() {
        got_inner = true;
        let label = format!("[{idx}]");
        let mut child = MaybeMut::Not(item);
        let mut header = show_header(&label, &mut child, layout, indent, ui, idx, changed, false);
        if header.openness > 0.0 {
            show_body(
                &mut child,
                &mut header,
                layout,
                indent,
                ui,
                idx,
                changed,
                false,
            );
        } else {
            header.set_has_inner(has_inner(&child));
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_map(
    map: PeekMap<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
) -> bool {
    let mut got_inner = false;
    for (idx, (key, value)) in map.iter().enumerate() {
        got_inner = true;
        let label = format!("{}", key);
        let mut child = MaybeMut::Not(value);
        let mut header = show_header(&label, &mut child, layout, indent, ui, idx, changed, false);
        if header.openness > 0.0 {
            show_body(
                &mut child,
                &mut header,
                layout,
                indent,
                ui,
                idx,
                changed,
                false,
            );
        } else {
            header.set_has_inner(has_inner(&child));
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_option(
    opt: PeekOption<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
) -> bool {
    if let Some(inner) = opt.value() {
        let mut child = MaybeMut::Not(inner);
        return show_inner_rows(&mut child, layout, indent, ui, changed, false);
    }
    false
}

fn show_inner_rows_peek_tuple(
    tuple: PeekTuple<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
) -> bool {
    let mut got_inner = false;
    for (idx, (_field, value)) in tuple.fields().enumerate() {
        got_inner = true;
        let label = format!("[{idx}]");
        let mut child = MaybeMut::Not(value);
        let mut header = show_header(&label, &mut child, layout, indent, ui, idx, changed, false);
        if header.openness > 0.0 {
            show_body(
                &mut child,
                &mut header,
                layout,
                indent,
                ui,
                idx,
                changed,
                false,
            );
        } else {
            header.set_has_inner(has_inner(&child));
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_pointer(
    ptr: PeekPointer<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    changed: &mut bool,
) -> bool {
    if let Some(inner) = ptr.borrow_inner() {
        let mut child = MaybeMut::Not(inner);
        return show_inner_rows(&mut child, layout, indent, ui, changed, false);
    }
    false
}

// ---------------------------------------------------------------------------
// Inline value rendering (the right-side widget for a row)
// ---------------------------------------------------------------------------

/// Show the inline (right-side) widget for a value. Returns the Response.
fn show_inline_value(
    value: &mut MaybeMut<'_, '_>,
    ui: &mut Ui,
    id: Id,
    force_reborrow: bool,
) -> Response {
    match value {
        MaybeMut::Mut(poke) => show_inline_poke(poke, ui, id, force_reborrow),
        MaybeMut::Not(peek) => show_inline_peek(*peek, ui, id),
    }
}

/// Show inline widget for a mutable value.
fn show_inline_poke(
    poke: &mut Poke<'_, '_>,
    ui: &mut Ui,
    id: Id,
    _force_reborrow: bool,
) -> Response {
    if let Some(scalar_type) = poke.as_peek().scalar_type() {
        return show_inline_poke_scalar(poke, scalar_type, ui);
    }

    // For non-scalar types, show a type summary label
    if poke.is_enum() {
        return show_inline_poke_enum(poke, ui, id);
    }
    if poke.is_struct() {
        return ui.weak(poke.shape().effective_name());
    }
    if let Def::List(list_def) = poke.shape().def {
        return show_inline_poke_list(poke, list_def, ui);
    }
    if let Ok(map) = poke.as_peek().into_map() {
        return ui.weak(format!("[{}]", map.len()));
    }
    if let Ok(opt) = poke.as_peek().into_option() {
        return show_inline_peek_option(opt, ui, id);
    }
    if let Ok(tuple) = poke.as_peek().into_tuple() {
        return ui.weak(format!("({})", tuple.len()));
    }
    if let Ok(ptr) = poke.as_peek().into_pointer()
        && let Some(inner) = ptr.borrow_inner()
    {
        return show_inline_peek(inner, ui, id);
    }

    ui.weak(poke.shape().effective_name())
}

/// Show inline widget for a mutable list: `[len]` with +/- buttons.
fn show_inline_poke_list(poke: &mut Poke<'_, '_>, list_def: ListDef, ui: &mut Ui) -> Response {
    let len = poke
        .as_peek()
        .into_list_like()
        .map(|l| l.len())
        .unwrap_or(0);
    let item_shape = list_def.t();
    let has_default = item_shape.is_default();
    let has_push = list_def.push().is_some();
    let has_set_len = list_def.set_len().is_some();

    let mut changed = false;
    let r = ui.horizontal(|ui| {
        ui.weak(format!("[{len}]"));

        if has_push && has_default && ui.small_button("+").clicked() {
            changed |= try_push_default_to_list(poke, list_def);
        }

        if has_set_len && len > 0 && ui.small_button("-").clicked() {
            changed |= try_pop_from_list(poke, list_def, item_shape);
        }
    });

    let mut r = r.response;
    if changed {
        r.mark_changed();
    }
    r
}

/// Push a default-constructed element to the list using `Partial`.
fn try_push_default_to_list(poke: &mut Poke<'_, '_>, list_def: ListDef) -> bool {
    let item_shape = list_def.t();
    let push_fn = match list_def.push() {
        Some(f) => f,
        None => return false,
    };

    // SAFETY: item_shape comes from the ListDef of this poke's shape.
    let partial = match unsafe { Partial::alloc_shape(item_shape) } {
        Ok(p) => p,
        Err(_) => return false,
    };
    let partial = match partial.set_default() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let heap_value = match partial.build() {
        Ok(v) => v,
        Err(_) => return false,
    };

    // push_fn moves the value out via ptr::read — ownership transfers to the list.
    // SAFETY: heap_value contains an initialized, aligned value of the correct item type.
    unsafe {
        let item_ptr = heap_value.peek().data().as_byte_ptr() as *mut u8;
        push_fn(poke.data_mut(), facet::PtrMut::new(item_ptr));
    }
    // The value has been moved into the list. Prevent HeapValue from dropping
    // the (now-moved) inner value. This leaks the HeapValue's backing allocation
    // but is correct per the ListPushFn contract.
    core::mem::forget(heap_value);

    true
}

/// Pop the last element from the list by moving it out, shrinking the Vec,
/// then dropping the extracted element.
fn try_pop_from_list(
    poke: &mut Poke<'_, '_>,
    list_def: ListDef,
    item_shape: &'static facet::Shape,
) -> bool {
    let set_len_fn = match list_def.set_len() {
        Some(f) => f,
        None => return false,
    };
    let len_fn = list_def.vtable.len;
    let get_mut_fn = match list_def.vtable.get_mut {
        Some(f) => f,
        None => return false,
    };
    // SAFETY: poke points to an initialized, aligned list value (Vec<T>).
    // len_fn and get_mut_fn come from the same ListDef as the poke's shape.
    let len = unsafe { len_fn(poke.data_mut().as_const()) };
    if len == 0 {
        return false;
    }

    // SAFETY: poke.shape() is the list's shape (Vec<T>), which get_mut_fn
    // uses to compute element size from type_params[0]. Index is in bounds.
    let Some(last_ptr) = (unsafe { get_mut_fn(poke.data_mut(), len - 1, poke.shape()) }) else {
        return false;
    };

    // SAFETY: After set_len(len - 1) the Vec no longer considers this slot
    // occupied, but its backing buffer is still allocated — last_ptr remains
    // valid. We drop in place, mirroring Vec::pop semantics (shrink length,
    // then drop the element).
    unsafe { set_len_fn(poke.data_mut(), len - 1) };
    unsafe { item_shape.call_drop_in_place(last_ptr) };

    true
}

/// Show inline widget for a mutable enum: ComboBox to select variant.
fn show_inline_poke_enum(poke: &mut Poke<'_, '_>, ui: &mut Ui, id: Id) -> Response {
    let shape = poke.shape();
    let Type::User(UserType::Enum(enum_type)) = shape.ty else {
        return ui.weak("enum");
    };

    // Get the active variant name (Peek is Copy, variant names are 'static)
    let active_name = poke
        .as_peek()
        .into_enum()
        .ok()
        .and_then(|e| e.active_variant().ok())
        .map(|v| v.effective_name())
        .unwrap_or("?");

    let mut changed = false;
    let r = egui::ComboBox::from_id_salt(id)
        .selected_text(active_name)
        .show_ui(ui, |ui| {
            for (idx, variant) in enum_type.variants.iter().enumerate() {
                let variant_name: &str = variant.effective_name();
                let is_active = variant_name == active_name;
                if ui.selectable_label(is_active, variant_name).clicked()
                    && !is_active
                    && try_change_variant(poke, idx)
                {
                    changed = true;
                }
            }
        });

    let mut r = r.response;
    if changed {
        r.mark_changed();
    }
    r
}

/// Try to change the enum variant by constructing a new value via `Partial`.
///
/// Returns `true` if the variant was successfully changed.
fn try_change_variant(poke: &mut Poke<'_, '_>, variant_idx: usize) -> bool {
    let shape = poke.shape();
    // Build a new enum value with the selected variant using Partial.
    // SAFETY: The shape used is from the provided Poke
    let partial = match unsafe { Partial::alloc_shape(shape) } {
        Ok(p) => p,
        Err(e) => {
            log::debug!("alloc_shape failed: {e}");
            return false;
        }
    };
    // this is the partial of the to be active variant
    let mut partial = match partial.select_nth_variant(variant_idx) {
        Ok(p) => p,
        Err(e) => {
            log::debug!("select_nth_variant failed: {e}");
            return false;
        }
    };

    // Explicitly default each field of the variant.
    // The variant's fields are available from the shape's enum type.
    let Type::User(UserType::Enum(enum_type)) = shape.ty else {
        return false;
    };
    let variant = &enum_type.variants[variant_idx];
    for field_idx in 0..variant.data.fields.len() {
        partial = match partial.set_nth_field_to_default(field_idx) {
            Ok(p) => p,
            Err(e) => {
                log::debug!(
                    "set_nth_field_to_default({field_idx}) failed for variant '{}': {e}",
                    variant.effective_name()
                );
                return false;
            }
        };
    }

    let heap_value = match partial.build() {
        Ok(v) => v,
        Err(e) => {
            log::debug!("build failed: {e}");
            return false;
        }
    };

    let size = shape
        .layout
        .sized_layout()
        .expect("enum must be sized")
        .size();

    // FIXME: replace once <https://github.com/facet-rs/facet/issues/2152> is implemented
    assert_eq!(poke.shape(), heap_value.shape());
    // SAFETY: the Shape is the same and this is the same as core::mem::replace
    // if we had T
    unsafe {
        // Swap the old enum value (in poke) with the new one (in heap_value).
        // After the swap, heap_value holds the old value — its Drop impl will
        // call drop_in_place on it and then free the allocation.
        let dst = poke.data_mut().as_mut_byte_ptr();
        let src = heap_value.peek().data().as_byte_ptr() as *mut u8;
        core::ptr::swap_nonoverlapping(dst, src, size);
    }
    drop(heap_value);

    true
}

fn show_inline_poke_scalar(
    poke: &mut Poke<'_, '_>,
    scalar_type: ScalarType,
    ui: &mut Ui,
) -> Response {
    match scalar_type {
        ScalarType::Bool => {
            if let Ok(v) = poke.get_mut::<bool>() {
                return ui.add(Checkbox::without_text(v));
            }
        }
        ScalarType::U8 => {
            if let Ok(v) = poke.get_mut::<u8>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::U16 => {
            if let Ok(v) = poke.get_mut::<u16>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::U32 => {
            if let Ok(v) = poke.get_mut::<u32>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::U64 => {
            if let Ok(v) = poke.get_mut::<u64>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::U128 => {
            // DragValue doesn't support u128, show as label
            if let Ok(v) = poke.get::<u128>() {
                return ui.label(format!("{v}"));
            }
        }
        ScalarType::USize => {
            if let Ok(v) = poke.get_mut::<usize>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::I8 => {
            if let Ok(v) = poke.get_mut::<i8>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::I16 => {
            if let Ok(v) = poke.get_mut::<i16>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::I32 => {
            if let Ok(v) = poke.get_mut::<i32>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::I64 => {
            if let Ok(v) = poke.get_mut::<i64>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::I128 => {
            if let Ok(v) = poke.get::<i128>() {
                return ui.label(format!("{v}"));
            }
        }
        ScalarType::ISize => {
            if let Ok(v) = poke.get_mut::<isize>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::F32 => {
            if let Ok(v) = poke.get_mut::<f32>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::F64 => {
            if let Ok(v) = poke.get_mut::<f64>() {
                return ui.add(egui::DragValue::new(v));
            }
        }
        ScalarType::String => {
            if let Ok(v) = poke.get_mut::<String>() {
                return ui.add(TextEdit::singleline(v));
            }
        }
        ScalarType::Char => {
            if let Ok(v) = poke.get::<char>() {
                let s = v.to_string();
                return ui.add_enabled(false, TextEdit::singleline(&mut s.as_str()));
            }
        }
        ScalarType::Str => {
            // str is unsized, fall through to display
            if poke.shape().is_display() {
                return ui.label(format!("{}", poke.as_peek()));
            }
        }
        ScalarType::CowStr => {
            if let Ok(v) = poke.get::<Cow<'_, str>>() {
                let mut s = v.clone();
                return ui.add_enabled(false, TextEdit::singleline(&mut s));
            }
        }
        _ if poke.shape().is_display() => {
            return ui.label(format!("{}", poke.as_peek()));
        }
        _ if poke.shape().is_debug() => {
            return ui.label(format!("{:?}", poke.as_peek()));
        }
        _ => {}
    }
    ui.colored_label(
        Color32::YELLOW,
        format!("unsupported scalar: {scalar_type:?}"),
    )
}

/// Show inline widget for a read-only value.
fn show_inline_peek(peek: Peek<'_, '_>, ui: &mut Ui, id: Id) -> Response {
    if let Some(scalar_type) = peek.scalar_type() {
        return show_inline_peek_scalar(peek, scalar_type, ui);
    }

    if let Ok(enu) = peek.into_enum() {
        if let Ok(variant) = enu.active_variant() {
            return ui.weak(variant.effective_name());
        }
        return ui.weak("enum");
    }
    if let Ok(_struc) = peek.into_struct() {
        return ui.weak(peek.shape().effective_name());
    }
    if let Ok(list) = peek.into_list_like() {
        return ui.weak(format!("[{}]", list.len()));
    }
    if let Ok(map) = peek.into_map() {
        return ui.weak(format!("[{}]", map.len()));
    }
    if let Ok(opt) = peek.into_option() {
        return show_inline_peek_option(opt, ui, id);
    }
    if let Ok(tuple) = peek.into_tuple() {
        return ui.weak(format!("({})", tuple.len()));
    }
    if let Ok(ptr) = peek.into_pointer()
        && let Some(inner) = ptr.borrow_inner()
    {
        return show_inline_peek(inner, ui, id.with("ptr"));
    }

    ui.weak(peek.shape().effective_name())
}

fn show_inline_peek_scalar(peek: Peek<'_, '_>, scalar_type: ScalarType, ui: &mut Ui) -> Response {
    match scalar_type {
        ScalarType::Bool => {
            if let Ok(v) = peek.get::<bool>() {
                let mut value = *v;
                return ui.add_enabled(false, Checkbox::without_text(&mut value));
            }
        }
        ScalarType::Char => {
            if let Ok(c) = peek.get::<char>() {
                let s = c.to_string();
                return ui.add_enabled(false, TextEdit::singleline(&mut s.as_str()));
            }
        }
        ScalarType::Str => {
            if let Ok(v) = peek.get::<str>() {
                return ui.add_enabled(false, TextEdit::singleline(&mut &*v));
            }
        }
        ScalarType::CowStr => {
            if let Ok(v) = peek.get::<Cow<'_, str>>() {
                let mut s = v.clone();
                return ui.add_enabled(false, TextEdit::singleline(&mut s));
            }
        }
        ScalarType::String => {
            if let Ok(v) = peek.get::<String>() {
                let mut s: Cow<'_, str> = Cow::Borrowed(v.as_str());
                return ui.add_enabled(false, TextEdit::singleline(&mut s));
            }
        }
        _ if peek.shape().is_display() => {
            return ui.label(format!("{}", peek));
        }
        _ if peek.shape().is_debug() => {
            return ui.label(format!("{:?}", peek));
        }
        _ => {}
    }
    ui.colored_label(
        Color32::YELLOW,
        format!("unsupported scalar: {scalar_type:?}"),
    )
}

fn show_inline_peek_option(opt: PeekOption<'_, '_>, ui: &mut Ui, id: Id) -> Response {
    ui.horizontal(|ui| {
        let is_some = opt.value().is_some();
        let _ = ui.selectable_label(!is_some, "None");
        let _ = ui.selectable_label(is_some, "Some");
        if let Some(inner) = opt.value() {
            show_inline_peek(inner, ui, id.with("some"));
        }
    })
    .response
}
