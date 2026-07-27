#![allow(clippy::too_many_arguments)]

use alloc::{
    borrow::{Cow, ToOwned},
    string::{String, ToString},
};
use core::fmt::Debug;

use derive_more::{Deref, DerefMut as DeriveDerefMut, From};
use egui::{
    Align, Checkbox, Color32, Id, Layout, Response, TextBuffer, TextEdit, Ui, UiBuilder, WidgetText,
};
use facet::{Def, Facet, ListDef, MapDef, OptionDef, ScalarType, SetDef, Type, UserType};
use facet_reflect::{
    HasFields, Partial, Peek, PeekEnum, PeekListLike, PeekMap, PeekOption, PeekPointer, PeekSet,
    PeekStruct, PeekTuple, Poke, PokeEnum, PokeList, PokeStruct,
};

use crate::{
    MaybeMut,
    layout::{ProbeHeader, ProbeLayout, swap_probe_header_state},
    maybe_mut::{Guard, MakeLockErrorKind},
};

/// Returns `true` if the given attributes slice contains `Attr::Skip`.
fn has_egui_skip(attributes: &[facet::Attr]) -> bool {
    attributes
        .iter()
        .any(|a| matches!((a.ns, a.key), (Some("egui"), "skip")))
}

/// Returns `true` if the given attributes slice contains `Attr::AsDisplay`.
fn has_egui_as_display(attributes: &[facet::Attr]) -> bool {
    attributes
        .iter()
        .any(|a| matches!((a.ns, a.key), (Some("egui"), "as_display")))
}

/// Returns the `Attr::Rename` value from the attributes, if present.
fn egui_rename(attributes: &[facet::Attr]) -> Option<&'static str> {
    attributes.iter().find_map(|a| {
        if matches!((a.ns, a.key), (Some("egui"), "rename")) {
            a.get_as::<&'static str>().copied()
        } else {
            None
        }
    })
}

/// Returns the display name for a field: uses `egui::rename` if present,
/// otherwise falls back to `effective_name()`.
fn field_display_name(field: &facet::Field) -> String {
    egui_rename(field.attributes)
        .unwrap_or_else(|| field.effective_name())
        .to_owned()
}

/// Returns the display name for a shape: uses `egui::rename` if present,
/// otherwise falls back to `effective_name()`.
fn shape_display_name(shape: &facet::Shape) -> &str {
    egui_rename(shape.attributes).unwrap_or_else(|| shape.effective_name())
}

fn text_line_count(s: &str) -> usize {
    1 + s.chars().filter(|&c| c == '\n').count()
}

fn shift_enter_pressed(ui: &Ui) -> bool {
    ui.input(|i| {
        i.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Enter,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.shift
            )
        })
    })
}

fn should_render_as_display(shape: &facet::Shape, attributes: &[facet::Attr]) -> bool {
    has_egui_as_display(attributes) || has_egui_as_display(shape.attributes)
}

/// The container that stores a [`MaybeMut`] of the type `T` that should be shown
/// in the [`Ui`](egui::Ui)
#[must_use = "use [`FacetProbe::show`] to display the probe in the [`Ui`]"]
#[derive(Deref, DeriveDerefMut)]
pub struct FacetProbe<'mem, 'facet> {
    header: Option<WidgetText>,
    id: Option<Id>,
    read_only: bool,
    expand_all: bool,
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
    pub fn readonly(self, readonly: bool) -> Self {
        Self {
            read_only: readonly,
            ..self
        }
    }

    pub fn expand_all(self, expand_all: bool) -> Self {
        Self { expand_all, ..self }
    }

    pub fn with_header(mut self, label: impl Into<WidgetText>) -> Self {
        self.header = Some(label.into());
        self
    }

    /// Set a stable egui id source for this probe.
    ///
    /// Use this when the probe can move in the UI hierarchy (e.g. draggable tabs),
    /// so collapse/expand state remains stable.
    pub fn with_id_source(mut self, id_source: impl core::hash::Hash + Debug) -> Self {
        self.id = Some(Id::new(id_source));
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
            id: None,
            read_only: true,
            expand_all: false,
            force_reborrow: false,
            inner: MaybeMut::Not(value),
        }
    }

    pub fn new_poke(value: Poke<'mem, 'facet>) -> Self {
        Self {
            header: None,
            id: None,
            read_only: false,
            expand_all: false,
            force_reborrow: false,
            inner: MaybeMut::Mut(value),
        }
    }

    pub fn new<T>(value: impl Into<MaybeMutT<'mem, T>>) -> Self
    where
        T: Facet<'facet> + 'mem,
        'facet: 'mem,
    {
        let v: MaybeMutT<'mem, T> = value.into();
        let inner: MaybeMut = match v {
            MaybeMutT::Mut(v) => Poke::new(v).into(),
            MaybeMutT::Not(v) => Peek::new(v).into(),
        };
        Self {
            header: None,
            id: None,
            read_only: false,
            expand_all: false,
            force_reborrow: false,
            inner,
        }
    }

    pub fn show<'lock>(self, ui: &mut Ui) -> Response
    where
        'mem: 'lock,
        'facet: 'mem,
    {
        // Container-level skip: hide the entire probe
        if has_egui_skip(self.shape().attributes) {
            return ui.label("");
        }

        let mut changed = false;

        let shape_attrs = self.shape().attributes;
        let readonly = shape_attrs
            .iter()
            .any(|a| matches!((a.ns, a.key), (Some("egui"), "readonly")))
            || self.read_only;

        // Check for expand_all attribute (or use self.expand_all)
        let expand_all = self.expand_all
            || shape_attrs
                .iter()
                .any(|a| matches!((a.ns, a.key), (Some("egui"), "expand_all")));

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
                        return ui.colored_label(Color32::RED, "Fallback Read Failure");
                    };
                    read
                }
                Err(e) if matches!(e.kind, MakeLockErrorKind::LockFailure) => {
                    return ui.colored_label(Color32::RED, "Lock Failure");
                }
                Err(e) => {
                    return ui.colored_label(Color32::RED, alloc::format!("Error: {e}"));
                }
            }
        };

        let maybe_mut = &mut guard.as_maybe();
        // Anchor persistent widget state to a probe root id that does not depend
        // on current Ui ancestry, so tab moves don't reset collapsed sections.
        let ptr_salt = maybe_mut.as_peek().data().as_byte_ptr() as usize;
        let probe_id = self
            .id
            .unwrap_or_else(|| Id::new(("facet_egui::probe", ptr_salt)));
        let mut r = ui
            .push_id(probe_id, |ui| {
                let child_ui = &mut ui.new_child(
                    UiBuilder::new()
                        .max_rect(ui.max_rect())
                        .layout(Layout::top_down(Align::Min)),
                );

                let mut layout = ProbeLayout::load(child_ui.ctx(), probe_id.with("layout"));
                let root_as_display = should_render_as_display(maybe_mut.shape(), &[]);

                if let Some(label) = self.header {
                    // Show with a top-level header (like Probe::new(x).with_header("name"))
                    let mut header = show_header(
                        label,
                        maybe_mut,
                        &mut layout,
                        0,
                        child_ui,
                        probe_id.with("root"),
                        &mut changed,
                        self.force_reborrow,
                        expand_all,
                        root_as_display,
                    );

                    if header.openness > 0.0 && !root_as_display {
                        show_body(
                            maybe_mut,
                            &mut header,
                            &mut layout,
                            0,
                            child_ui,
                            probe_id.with("root"),
                            &mut changed,
                            self.force_reborrow,
                            expand_all,
                            root_as_display,
                        );
                    }

                    header.store(child_ui.ctx());
                } else {
                    // Show directly without a top-level header (table of fields)
                    show_body_direct(
                        maybe_mut,
                        &mut layout,
                        0,
                        child_ui,
                        probe_id.with("root"),
                        &mut changed,
                        self.force_reborrow,
                        expand_all,
                        root_as_display,
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
    // Option/Result must be checked before enum, since they also match into_enum().
    if let Ok(opt) = peek.into_option()
        && let Some(inner) = opt.value()
    {
        return has_inner(&MaybeMut::Not(inner));
    }
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
    id: Id,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
    as_display: bool,
) -> ProbeHeader {
    show_header_with_prefix(
        |_| {},
        true,
        label,
        value,
        layout,
        indent,
        ui,
        id,
        changed,
        force_reborrow,
        expand_all,
        as_display,
    )
}

/// Like `show_header` but renders `prefix` widgets in the label column before
/// the collapse button. Used by the list row to inject ▲/▼ swap buttons.
#[expect(clippy::too_many_arguments)]
fn show_header_with_prefix(
    prefix: impl FnOnce(&mut Ui),
    animate: bool,
    label: impl Into<WidgetText>,
    value: &mut MaybeMut<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    ui: &mut Ui,
    id: Id,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
    as_display: bool,
) -> ProbeHeader {
    let mut header = if animate {
        ProbeHeader::load(ui.ctx(), id)
    } else {
        ProbeHeader::load_no_animation(ui.ctx(), id)
    };
    let row_has_inner = !as_display && has_inner(value);
    header.set_has_inner(row_has_inner);
    if !row_has_inner {
        header.set_open(false);
    }

    if expand_all && row_has_inner {
        header.set_open(true);
    }

    ui.horizontal(|ui| {
        let label_response = layout.inner_label_ui(indent, id.with("label"), ui, |ui| {
            prefix(ui);
            if header.has_inner() {
                header.collapse_button(ui);
            }
            ui.label(label)
        });

        layout.inner_value_ui(id.with("value"), ui, |ui| {
            *changed |= show_inline_value(value, ui, id, force_reborrow, as_display)
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
    id: Id,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
    as_display: bool,
) {
    if as_display {
        header.set_has_inner(false);
        header.set_open(false);
        return;
    }

    let cursor = ui.cursor();
    let table_rect = egui::Rect::from_min_max(
        egui::pos2(cursor.min.x, cursor.min.y - header.body_shift()),
        ui.max_rect().max,
    );

    let mut table_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(table_rect)
            .layout(Layout::top_down(Align::Min))
            .id_salt(id.with("body")),
    );
    table_ui.set_clip_rect(
        ui.clip_rect()
            .intersect(egui::Rect::everything_below(ui.min_rect().max.y)),
    );

    let got_inner = show_inner_rows(
        value,
        layout,
        indent + 1,
        id,
        &mut table_ui,
        changed,
        force_reborrow,
        expand_all,
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
    id: Id,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
    as_display: bool,
) {
    if as_display {
        *changed |= show_inline_value(value, ui, id, force_reborrow, true).changed();
        return;
    }

    let cursor = ui.cursor();
    let table_rect =
        egui::Rect::from_min_max(egui::pos2(cursor.min.x, cursor.min.y), ui.max_rect().max);

    let mut table_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(table_rect)
            .layout(Layout::top_down(Align::Min))
            .id_salt(id.with("body")),
    );
    table_ui.set_clip_rect(
        ui.clip_rect()
            .intersect(egui::Rect::everything_below(ui.min_rect().max.y)),
    );

    show_inner_rows(
        value,
        layout,
        indent + 1,
        id,
        &mut table_ui,
        changed,
        force_reborrow,
        expand_all,
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
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    match value {
        MaybeMut::Mut(poke) => show_inner_rows_poke(
            poke,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        ),
        MaybeMut::Not(peek) => show_inner_rows_peek(
            *peek,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        ),
    }
}

/// Attempt to write-lock a child [`MaybeMut`]. If the child is already
/// [`MaybeMut::Mut`] or wraps a lockable pointer (e.g. `RwLock`), this
/// returns a [`Guard`] with mutable access. Otherwise it falls back to
/// a read lock.
fn lock_child<'mem, 'facet>(child: MaybeMut<'mem, 'facet>) -> Option<Guard<'mem, 'facet>>
where
    'facet: 'mem,
{
    match child.write() {
        Ok(guard) => Some(guard),
        Err(e) if matches!(e.kind, MakeLockErrorKind::NotLockable) => {
            MaybeMut::Not(e.unchanged).read().ok()
        }
        Err(_) => None,
    }
}

fn show_inner_rows_poke(
    poke: &mut Poke<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    // Option/Result have Def::Option/Def::Result but Type::User(UserType::Enum),
    // so check Def before is_enum() to avoid misrouting.
    if let Def::Option(option_def) = poke.shape().def {
        return show_inner_rows_poke_option(
            poke,
            option_def,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        );
    }

    // For enums, we can use into_enum directly without reborrowing,
    // since PokeEnum.field() takes &mut self.
    if poke.is_enum() {
        let enu_poke = match poke.try_reborrow() {
            Some(rb) => rb,
            None if force_reborrow => unsafe {
                Poke::from_raw_parts(poke.data_mut(), poke.shape())
            },
            None => {
                return show_inner_rows_peek(
                    poke.as_peek(),
                    layout,
                    indent,
                    id,
                    ui,
                    changed,
                    force_reborrow,
                    expand_all,
                );
            }
        };
        if let Ok(enu) = enu_poke.into_enum() {
            return show_inner_rows_poke_enum(
                enu,
                layout,
                indent,
                id,
                ui,
                changed,
                force_reborrow,
                expand_all,
            );
        }
        return show_inner_rows_peek(
            poke.as_peek(),
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        );
    }

    // For structs, reborrow to get mutable field access
    if poke.is_struct() {
        let reborrow = match poke.try_reborrow() {
            Some(rb) => rb,
            None if force_reborrow => unsafe {
                Poke::from_raw_parts(poke.data_mut(), poke.shape())
            },
            None => {
                return show_inner_rows_peek(
                    poke.as_peek(),
                    layout,
                    indent,
                    id,
                    ui,
                    changed,
                    force_reborrow,
                    expand_all,
                );
            }
        };
        if let Ok(struc) = reborrow.into_struct() {
            return show_inner_rows_poke_struct(
                struc,
                layout,
                indent,
                id,
                ui,
                changed,
                force_reborrow,
                expand_all,
            );
        }
    }

    // Maps and Sets: PokeMap/PokeSet don't expose mutable values, so render
    // entries via the read-only peek path (the inline `+` button is shown by
    // `show_inline_poke_map`/`show_inline_poke_set`).
    if matches!(poke.shape().def, Def::Map(_) | Def::Set(_)) {
        return show_inner_rows_peek(
            poke.as_peek(),
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        );
    }

    let data_mut = poke.data_mut();
    let shape = poke.shape();
    let poke = match poke.try_reborrow() {
        Some(rb) => rb,
        None if force_reborrow => unsafe { Poke::from_raw_parts(poke.data_mut(), poke.shape()) },
        None => {
            return show_inner_rows_peek(
                poke.as_peek(),
                layout,
                indent,
                id,
                ui,
                changed,
                force_reborrow,
                expand_all,
            );
        }
    };
    if let Ok(poke_list) = poke.into_list() {
        show_inner_rows_poke_list(
            poke_list,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    } else {
        // restore old poke
        // SAFETY: this is ok because there still is only one access to poke due to the if
        // branch not being reached
        let poke = unsafe { Poke::from_raw_parts(data_mut, shape) };
        // For tuple, option, pointer — fall through to peek
        show_inner_rows_peek(
            poke.as_peek(),
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    }
}

fn show_inner_rows_poke_list(
    mut list: PokeList<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let len = list.len();
    if len == 0 {
        return false;
    }
    let can_swap = list.def().vtable.swap.is_some();
    // Swaps can't be performed while we hold a `get_mut` borrow on a row, so
    // collect the requested swap and apply it after the iteration.
    let mut pending_swap: Option<(usize, usize)> = None;
    for idx in 0..len {
        let label = alloc::format!("[{idx}]");
        if let Some(field_poke) = list.get_mut(idx) {
            let row_id = id.with(("list", idx));
            let Some(mut guard) = lock_child(MaybeMut::Mut(field_poke)) else {
                continue;
            };
            let child = &mut guard.as_maybe();
            let as_display = should_render_as_display(child.shape(), &[]);
            let prefix = |ui: &mut Ui| {
                if !can_swap {
                    return;
                }
                ui.scope(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(2.0, 0.0);
                    let up = ui
                        .add_enabled(idx > 0, egui::Button::new("⬆").small())
                        .on_hover_text("move up");
                    if up.clicked() {
                        pending_swap = Some((idx, idx - 1));
                    }
                    let down = ui
                        .add_enabled(idx + 1 < len, egui::Button::new("⬇").small())
                        .on_hover_text("move down");
                    if down.clicked() {
                        pending_swap = Some((idx, idx + 1));
                    }
                });
            };
            let mut header = show_header_with_prefix(
                prefix,
                false,
                &label,
                child,
                layout,
                indent,
                ui,
                row_id,
                changed,
                force_reborrow,
                expand_all,
                as_display,
            );
            if header.openness > 0.0 && !as_display {
                show_body(
                    child,
                    &mut header,
                    layout,
                    indent,
                    ui,
                    row_id,
                    changed,
                    force_reborrow,
                    expand_all,
                    as_display,
                );
            }
            header.store(ui.ctx());
        }
    }
    if let Some((a, b)) = pending_swap
        && list.swap(a, b).is_ok()
    {
        // Move the persisted collapse state along with the item, otherwise the
        // (now-different) item that ends up at the original index would inherit
        // the open/closed state of the moved row.
        swap_probe_header_state(ui.ctx(), id.with(("list", a)), id.with(("list", b)));
        *changed = true;
    }
    true
}

fn show_inner_rows_poke_struct(
    mut struc: PokeStruct<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let count = struc.field_count();
    if count == 0 {
        return false;
    }
    let mut got_inner = false;
    for idx in 0..count {
        let field = &struc.ty().fields[idx];
        if has_egui_skip(field.attributes) {
            continue;
        }
        if let Ok(field_poke) = struc.field(idx) {
            let row_id = id.with(("struct", idx));
            let Some(mut guard) = lock_child(MaybeMut::Mut(field_poke)) else {
                continue;
            };
            let child = &mut guard.as_maybe();
            if field.is_flattened() {
                ui.push_id(row_id, |ui| {
                    got_inner |= show_inner_rows(
                        child,
                        layout,
                        indent,
                        row_id,
                        ui,
                        changed,
                        force_reborrow,
                        expand_all,
                    );
                });
                continue;
            }
            got_inner = true;
            let field_name = field_display_name(field);
            let as_display = should_render_as_display(child.shape(), field.attributes);
            let mut header = show_header(
                &field_name,
                child,
                layout,
                indent,
                ui,
                row_id,
                changed,
                force_reborrow,
                expand_all,
                as_display,
            );
            if header.openness > 0.0 && !as_display {
                show_body(
                    child,
                    &mut header,
                    layout,
                    indent,
                    ui,
                    row_id,
                    changed,
                    force_reborrow,
                    expand_all,
                    as_display,
                );
            }
            header.store(ui.ctx());
        }
    }
    got_inner
}

fn show_inner_rows_poke_enum(
    mut enu: PokeEnum<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let variant = match enu.active_variant() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let field_count = variant.data.fields.len();
    if field_count == 0 {
        return false;
    }
    let mut got_inner = false;
    for idx in 0..field_count {
        let field = &variant.data.fields[idx];
        if has_egui_skip(field.attributes) {
            continue;
        }
        if let Ok(Some(field_poke)) = enu.field(idx) {
            let row_id = id.with(("enum", idx));
            let Some(mut guard) = lock_child(MaybeMut::Mut(field_poke)) else {
                continue;
            };
            let child = &mut guard.as_maybe();
            if field.is_flattened() {
                ui.push_id(row_id, |ui| {
                    got_inner |= show_inner_rows(
                        child,
                        layout,
                        indent,
                        row_id,
                        ui,
                        changed,
                        force_reborrow,
                        expand_all,
                    );
                });
                continue;
            }
            let field_name = field_display_name(field);
            let as_display = should_render_as_display(child.shape(), field.attributes);
            let mut header = show_header(
                &field_name,
                child,
                layout,
                indent,
                ui,
                row_id,
                changed,
                force_reborrow,
                expand_all,
                as_display,
            );
            if header.openness > 0.0 && !as_display {
                show_body(
                    child,
                    &mut header,
                    layout,
                    indent,
                    ui,
                    row_id,
                    changed,
                    force_reborrow,
                    expand_all,
                    as_display,
                );
            }
            header.store(ui.ctx());
        }
    }
    got_inner
}

fn show_inner_rows_peek(
    peek: Peek<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    if let Ok(opt) = peek.into_option() {
        show_inner_rows_peek_option(
            opt,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    } else if let Ok(struc) = peek.into_struct() {
        show_inner_rows_peek_struct(
            struc,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    } else if let Ok(enu) = peek.into_enum() {
        show_inner_rows_peek_enum(
            enu,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    } else if let Ok(list) = peek.into_list_like() {
        show_inner_rows_peek_list(
            list,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    } else if let Ok(map) = peek.into_map() {
        show_inner_rows_peek_map(
            map,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    } else if let Ok(set) = peek.into_set() {
        show_inner_rows_peek_set(
            set,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    } else if let Ok(tuple) = peek.into_tuple() {
        show_inner_rows_peek_tuple(
            tuple,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    } else if let Ok(ptr) = peek.into_pointer() {
        show_inner_rows_peek_pointer(
            ptr,
            layout,
            indent,
            id,
            ui,
            changed,
            force_reborrow,
            expand_all,
        )
    } else {
        false
    }
}

fn show_inner_rows_peek_struct(
    struc: PeekStruct<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let mut got_inner = false;
    for (idx, (field, value)) in struc.fields().enumerate() {
        let row_id = id.with(("struct", idx));
        if has_egui_skip(field.attributes) {
            continue;
        }
        let Some(mut guard) = lock_child(MaybeMut::Not(value)) else {
            continue;
        };
        let child = &mut guard.as_maybe();
        if field.is_flattened() {
            ui.push_id(row_id, |ui| {
                got_inner |= show_inner_rows(
                    child,
                    layout,
                    indent,
                    row_id,
                    ui,
                    changed,
                    force_reborrow,
                    expand_all,
                );
            });
            continue;
        }
        got_inner = true;
        let field_name = field_display_name(&field);
        let as_display = should_render_as_display(child.shape(), field.attributes);
        let mut header = show_header(
            &field_name,
            child,
            layout,
            indent,
            ui,
            row_id,
            changed,
            force_reborrow,
            expand_all,
            as_display,
        );
        if header.openness > 0.0 && !as_display {
            show_body(
                child,
                &mut header,
                layout,
                indent,
                ui,
                row_id,
                changed,
                force_reborrow,
                expand_all,
                as_display,
            );
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_enum(
    enu: PeekEnum<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let mut got_inner = false;
    for (idx, (field, value)) in enu.fields().enumerate() {
        let row_id = id.with(("enum", idx));
        if has_egui_skip(field.attributes) {
            continue;
        }
        let Some(mut guard) = lock_child(MaybeMut::Not(value)) else {
            continue;
        };
        let child = &mut guard.as_maybe();
        if field.is_flattened() {
            ui.push_id(row_id, |ui| {
                got_inner |= show_inner_rows(
                    child,
                    layout,
                    indent,
                    row_id,
                    ui,
                    changed,
                    force_reborrow,
                    expand_all,
                );
            });
            continue;
        }
        got_inner = true;
        let field_name = field_display_name(&field);
        let as_display = should_render_as_display(child.shape(), field.attributes);
        let mut header = show_header(
            &field_name,
            child,
            layout,
            indent,
            ui,
            row_id,
            changed,
            force_reborrow,
            expand_all,
            as_display,
        );
        if header.openness > 0.0 && !as_display {
            show_body(
                child,
                &mut header,
                layout,
                indent,
                ui,
                row_id,
                changed,
                force_reborrow,
                expand_all,
                as_display,
            );
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_list(
    list: PeekListLike<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let mut got_inner = false;
    for (idx, item) in list.iter().enumerate() {
        let row_id = id.with(("list", idx));
        got_inner = true;
        let label = alloc::format!("[{idx}]");
        let Some(mut guard) = lock_child(MaybeMut::Not(item)) else {
            continue;
        };
        let child = &mut guard.as_maybe();
        let as_display = should_render_as_display(child.shape(), &[]);
        let mut header = show_header(
            &label,
            child,
            layout,
            indent,
            ui,
            row_id,
            changed,
            force_reborrow,
            expand_all,
            as_display,
        );
        if header.openness > 0.0 && !as_display {
            show_body(
                child,
                &mut header,
                layout,
                indent,
                ui,
                row_id,
                changed,
                force_reborrow,
                expand_all,
                as_display,
            );
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_map(
    map: PeekMap<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let mut got_inner = false;
    for (idx, (key, value)) in map.iter().enumerate() {
        let row_id = id.with(("map", idx));
        got_inner = true;
        let label = alloc::format!("{}", key);
        let Some(mut guard) = lock_child(MaybeMut::Not(value)) else {
            continue;
        };
        let child = &mut guard.as_maybe();
        let as_display = should_render_as_display(child.shape(), &[]);
        let mut header = show_header(
            &label,
            child,
            layout,
            indent,
            ui,
            row_id,
            changed,
            force_reborrow,
            expand_all,
            as_display,
        );
        if header.openness > 0.0 && !as_display {
            show_body(
                child,
                &mut header,
                layout,
                indent,
                ui,
                row_id,
                changed,
                force_reborrow,
                expand_all,
                as_display,
            );
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_set(
    set: PeekSet<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let mut got_inner = false;
    for (idx, value) in set.iter().enumerate() {
        let row_id = id.with(("set", idx));
        got_inner = true;
        let label = alloc::format!("[{idx}]");
        let Some(mut guard) = lock_child(MaybeMut::Not(value)) else {
            continue;
        };
        let child = &mut guard.as_maybe();
        let as_display = should_render_as_display(child.shape(), &[]);
        let mut header = show_header(
            &label,
            child,
            layout,
            indent,
            ui,
            row_id,
            changed,
            force_reborrow,
            expand_all,
            as_display,
        );
        if header.openness > 0.0 && !as_display {
            show_body(
                child,
                &mut header,
                layout,
                indent,
                ui,
                row_id,
                changed,
                force_reborrow,
                expand_all,
                as_display,
            );
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_option(
    opt: PeekOption<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    if let Some(inner) = opt.value() {
        let Some(mut guard) = lock_child(MaybeMut::Not(inner)) else {
            return false;
        };
        let child = &mut guard.as_maybe();
        return show_inner_rows(
            child,
            layout,
            indent,
            id.with("option"),
            ui,
            changed,
            force_reborrow,
            expand_all,
        );
    }
    false
}

fn show_inner_rows_peek_tuple(
    tuple: PeekTuple<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let mut got_inner = false;
    for (idx, (_field, value)) in tuple.fields().enumerate() {
        let row_id = id.with(("tuple", idx));
        got_inner = true;
        let label = alloc::format!("[{idx}]");
        let Some(mut guard) = lock_child(MaybeMut::Not(value)) else {
            continue;
        };
        let child = &mut guard.as_maybe();
        let as_display = should_render_as_display(child.shape(), &[]);
        let mut header = show_header(
            &label,
            child,
            layout,
            indent,
            ui,
            row_id,
            changed,
            force_reborrow,
            expand_all,
            as_display,
        );
        if header.openness > 0.0 && !as_display {
            show_body(
                child,
                &mut header,
                layout,
                indent,
                ui,
                row_id,
                changed,
                force_reborrow,
                expand_all,
                as_display,
            );
        }
        header.store(ui.ctx());
    }
    got_inner
}

fn show_inner_rows_peek_pointer(
    ptr: PeekPointer<'_, '_>,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    if let Some(inner) = ptr.borrow_inner() {
        let Some(mut guard) = lock_child(MaybeMut::Not(inner)) else {
            return false;
        };
        let child = &mut guard.as_maybe();
        return show_inner_rows(
            child,
            layout,
            indent,
            id.with("ptr"),
            ui,
            changed,
            force_reborrow,
            expand_all,
        );
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
    as_display: bool,
) -> Response {
    match value {
        MaybeMut::Mut(poke) => show_inline_poke(poke, ui, id, force_reborrow, as_display),
        MaybeMut::Not(peek) => show_inline_peek(*peek, ui, id, as_display),
    }
}

/// Show inline widget for a mutable value.
fn show_inline_poke(
    poke: &mut Poke<'_, '_>,
    ui: &mut Ui,
    id: Id,
    force_reborrow: bool,
    as_display: bool,
) -> Response {
    if as_display || should_render_as_display(poke.shape(), &[]) {
        return ui.label(alloc::format!("{}", poke.as_peek()));
    }

    if let Some(scalar_type) = poke.as_peek().scalar_type() {
        return show_inline_poke_scalar(poke, scalar_type, ui);
    }

    // For non-scalar types, show a type summary label
    // Option/Result have Def::Option/Def::Result but Type::User(UserType::Enum),
    // so check Def before is_enum() to avoid misrouting.
    if let Def::Option(option_def) = poke.shape().def {
        return show_inline_poke_option(poke, option_def, ui, id, force_reborrow);
    }
    if poke.is_enum() {
        return show_inline_poke_enum(poke, ui, id);
    }
    if poke.is_struct() {
        return ui.weak(shape_display_name(poke.shape()));
    }
    if let Def::List(list_def) = poke.shape().def {
        return show_inline_poke_list(poke, list_def, ui);
    }
    if let Def::Map(map_def) = poke.shape().def {
        return show_inline_poke_map(poke, map_def, ui);
    }
    if let Def::Set(set_def) = poke.shape().def {
        return show_inline_poke_set(poke, set_def, ui);
    }
    if let Ok(tuple) = poke.as_peek().into_tuple() {
        return ui.weak(alloc::format!("({})", tuple.len()));
    }
    if let Ok(ptr) = poke.as_peek().into_pointer()
        && let Some(inner) = ptr.borrow_inner()
    {
        return show_inline_peek(inner, ui, id, false);
    }

    ui.weak(shape_display_name(poke.shape()))
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
    let has_pop = list_def.pop().is_some();

    let mut changed = false;
    let r = ui.horizontal(|ui| {
        ui.weak(alloc::format!("[{len}]"));

        if has_push && has_default && ui.small_button("+").clicked() {
            changed |= try_push_default_to_list(poke, list_def);
        }

        if has_pop && len > 0 && ui.small_button("-").clicked() {
            changed |= try_pop_from_list(poke);
        }
    });

    let mut r = r.response;
    if changed {
        r.mark_changed();
    }
    r
}

/// Push a default-constructed element to the list via `PokeList::push_from_heap`.
fn try_push_default_to_list(poke: &mut Poke<'_, '_>, list_def: ListDef) -> bool {
    let item_shape = list_def.t();

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

    let Some(reborrow) = poke.try_reborrow() else {
        return false;
    };
    let Ok(mut list) = reborrow.into_list() else {
        return false;
    };
    list.push_from_heap(heap_value).is_ok()
}

/// Pop the last element from the list via `PokeList::pop`. The returned
/// `HeapValue` drops the popped element when it goes out of scope.
fn try_pop_from_list(poke: &mut Poke<'_, '_>) -> bool {
    let Some(reborrow) = poke.try_reborrow() else {
        return false;
    };
    let Ok(mut list) = reborrow.into_list() else {
        return false;
    };
    matches!(list.pop(), Ok(Some(_)))
}

/// Show inline widget for a mutable map: `[len]` with a `+` button to insert a
/// default-built (key, value) entry. Map removal is not exposed by `PokeMap`,
/// so there is no `-` button.
fn show_inline_poke_map(poke: &mut Poke<'_, '_>, map_def: MapDef, ui: &mut Ui) -> Response {
    let len = poke.as_peek().into_map().map(|m| m.len()).unwrap_or(0);
    let key_shape = map_def.k();
    let value_shape = map_def.v();
    let can_insert = key_shape.is_default() && value_shape.is_default();

    let mut changed = false;
    let r = ui.horizontal(|ui| {
        ui.weak(alloc::format!("[{len}]"));

        if can_insert
            && ui
                .small_button("+")
                .on_hover_text("insert default key/value")
                .clicked()
        {
            changed |= try_insert_default_into_map(poke, map_def);
        }
    });

    let mut r = r.response;
    if changed {
        r.mark_changed();
    }
    r
}

/// Insert a (default key, default value) entry into the map via
/// `PokeMap::insert_from_heap`.
fn try_insert_default_into_map(poke: &mut Poke<'_, '_>, map_def: MapDef) -> bool {
    let key = match build_default_heap_value(map_def.k()) {
        Some(v) => v,
        None => return false,
    };
    let value = match build_default_heap_value(map_def.v()) {
        Some(v) => v,
        None => return false,
    };

    let Some(reborrow) = poke.try_reborrow() else {
        return false;
    };
    let Ok(mut map) = reborrow.into_map() else {
        return false;
    };
    map.insert_from_heap(key, value).is_ok()
}

/// Show inline widget for a mutable set: `[len]` with a `+` button to insert a
/// default-built element. Set removal is not exposed by `PokeSet`, so there is
/// no `-` button.
fn show_inline_poke_set(poke: &mut Poke<'_, '_>, set_def: SetDef, ui: &mut Ui) -> Response {
    let len = poke.as_peek().into_set().map(|s| s.len()).unwrap_or(0);
    let elem_shape = set_def.t();
    let can_insert = elem_shape.is_default();

    let mut changed = false;
    let r = ui.horizontal(|ui| {
        ui.weak(alloc::format!("[{len}]"));

        if can_insert
            && ui
                .small_button("+")
                .on_hover_text("insert default value")
                .clicked()
        {
            changed |= try_insert_default_into_set(poke, set_def);
        }
    });

    let mut r = r.response;
    if changed {
        r.mark_changed();
    }
    r
}

/// Insert a default-constructed element into the set via
/// `PokeSet::insert_from_heap`.
fn try_insert_default_into_set(poke: &mut Poke<'_, '_>, set_def: SetDef) -> bool {
    let value = match build_default_heap_value(set_def.t()) {
        Some(v) => v,
        None => return false,
    };

    let Some(reborrow) = poke.try_reborrow() else {
        return false;
    };
    let Ok(mut set) = reborrow.into_set() else {
        return false;
    };
    set.insert_from_heap(value).is_ok()
}

/// Build a default-constructed `HeapValue` for the given shape, or return
/// `None` if allocation, defaulting, or building fails.
fn build_default_heap_value(
    shape: &'static facet::Shape,
) -> Option<facet_reflect::HeapValue<'static, true>> {
    // SAFETY: `shape` is provided by the caller and assumed to be a real, registered shape.
    let partial = unsafe { Partial::alloc_shape(shape) }.ok()?;
    let partial = partial.set_default().ok()?;
    partial.build().ok()
}

/// Show inline widget for a mutable option: toggle between None and Some.
fn show_inline_poke_option(
    poke: &mut Poke<'_, '_>,
    option_def: OptionDef,
    ui: &mut Ui,
    id: Id,
    force_reborrow: bool,
) -> Response {
    let is_some = unsafe { (option_def.vtable.is_some)(poke.data_mut().as_const()) };

    let mut changed = false;
    let r = ui.horizontal(|ui| {
        if ui.selectable_label(!is_some, "None").clicked()
            && is_some
            && let Some(reborrow) = poke.try_reborrow()
            && let Ok(mut opt) = reborrow.into_option()
        {
            opt.set_none();
            changed = true;
        }
        if ui.selectable_label(is_some, "Some").clicked() && !is_some {
            // Switch from None to Some(default)
            changed = try_set_option_to_some_default(poke, option_def);
        }
        if is_some {
            let inner_ptr = unsafe { (option_def.vtable.get_value)(poke.data_mut().as_const()) };
            if !inner_ptr.is_null() {
                // SAFETY: We have unique mutable access through poke, and the inner value
                // is stored within the Option's memory. The shape matches the inner type.
                let mut inner_poke = unsafe {
                    Poke::from_raw_parts(facet::PtrMut::new(inner_ptr as *mut u8), option_def.t())
                };
                show_inline_poke(&mut inner_poke, ui, id.with("some"), force_reborrow, false);
            }
        }
    });

    let mut r = r.response;
    if changed {
        r.mark_changed();
    }
    r
}

/// Set an Option from None to Some(T::default()) via `PokeOption::set_some_from_heap`.
fn try_set_option_to_some_default(poke: &mut Poke<'_, '_>, option_def: OptionDef) -> bool {
    let inner_shape = option_def.t();

    // Allocate and default-construct the inner value
    // SAFETY: inner_shape comes from the OptionDef of this poke's shape.
    let partial = match unsafe { Partial::alloc_shape(inner_shape) } {
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

    let Some(reborrow) = poke.try_reborrow() else {
        return false;
    };
    let Ok(mut opt) = reborrow.into_option() else {
        return false;
    };
    opt.set_some_from_heap(heap_value).is_ok()
}

/// Show inner rows for a mutable Option. When Some, shows the inner value mutably.
fn show_inner_rows_poke_option(
    poke: &mut Poke<'_, '_>,
    option_def: OptionDef,
    layout: &mut ProbeLayout,
    indent: usize,
    id: Id,
    ui: &mut Ui,
    changed: &mut bool,
    force_reborrow: bool,
    expand_all: bool,
) -> bool {
    let is_some = unsafe { (option_def.vtable.is_some)(poke.data_mut().as_const()) };
    if !is_some {
        return false;
    }

    let inner_ptr = unsafe { (option_def.vtable.get_value)(poke.data_mut().as_const()) };
    if inner_ptr.is_null() {
        return false;
    }

    // SAFETY: We have unique mutable access through poke, and the inner value
    // is stored within the Option's memory. The shape matches the inner type.
    let inner_poke =
        unsafe { Poke::from_raw_parts(facet::PtrMut::new(inner_ptr as *mut u8), option_def.t()) };

    let mut child = MaybeMut::Mut(inner_poke);
    show_inner_rows(
        &mut child,
        layout,
        indent,
        id.with("option"),
        ui,
        changed,
        force_reborrow,
        expand_all,
    )
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

/// An [`egui::TextBuffer`] wrapper around a single [`char`].
///
/// Editing behaves as a replace: any inserted text sets the char to the first
/// character of that text, so typing or pasting swaps the value in place.
/// Deletion is a no-op because a `char` must always hold a value.
struct CharBuffer {
    value: char,
    buf: [u8; 4],
    len: usize,
}

impl CharBuffer {
    fn new(value: char) -> Self {
        let mut this = Self {
            value,
            buf: [0; 4],
            len: 0,
        };
        this.set(value);
        this
    }

    fn set(&mut self, value: char) {
        self.value = value;
        self.len = value.encode_utf8(&mut self.buf).len();
    }
}

impl TextBuffer for CharBuffer {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    fn insert_text(&mut self, text: &str, _char_index: egui::text::CharIndex) -> usize {
        match text.chars().next() {
            Some(c) => {
                self.set(c);
                1
            }
            None => 0,
        }
    }

    fn delete_char_range(&mut self, _char_range: core::ops::Range<egui::text::CharIndex>) {}

    fn type_id(&self) -> core::any::TypeId {
        core::any::TypeId::of::<Self>()
    }
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
                return ui.label(alloc::format!("{v}"));
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
                return ui.label(alloc::format!("{v}"));
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
                if v.contains('\n') {
                    let rows = text_line_count(v);
                    return ui.add(TextEdit::multiline(v).desired_rows(rows));
                }
                let mut r = ui.add(TextEdit::singleline(v));
                if (r.has_focus() || r.lost_focus()) && shift_enter_pressed(ui) {
                    v.push('\n');
                    r.mark_changed();
                    r.request_focus();
                }
                return r;
            }
        }
        ScalarType::Char => {
            if let Ok(v) = poke.get_mut::<char>() {
                let mut buf = CharBuffer::new(*v);
                let r = ui.add(TextEdit::singleline(&mut buf));
                if r.changed() {
                    *v = buf.value;
                }
                return r;
            }
        }
        // str is unsized, fall through to display
        ScalarType::Str if poke.shape().is_display() => {
            return ui.label(alloc::format!("{}", poke.as_peek()));
        }
        ScalarType::CowStr => {
            if let Ok(v) = poke.get::<Cow<'_, str>>() {
                let mut s = v.clone();
                if s.contains('\n') {
                    let rows = text_line_count(&s);
                    return ui.add_enabled(false, TextEdit::multiline(&mut s).desired_rows(rows));
                }
                return ui.add_enabled(false, TextEdit::singleline(&mut s));
            }
        }
        _ if poke.shape().is_display() => {
            return ui.label(alloc::format!("{}", poke.as_peek()));
        }
        _ if poke.shape().is_debug() => {
            return ui.label(alloc::format!("{:?}", poke.as_peek()));
        }
        _ => {}
    }
    ui.colored_label(
        Color32::YELLOW,
        alloc::format!("unsupported scalar: {scalar_type:?}"),
    )
}

fn show_inline_peek(peek: Peek<'_, '_>, ui: &mut Ui, id: Id, as_display: bool) -> Response {
    if as_display || should_render_as_display(peek.shape(), &[]) {
        return ui.label(alloc::format!("{}", peek));
    }

    if let Some(scalar_type) = peek.scalar_type() {
        return show_inline_peek_scalar(peek, scalar_type, ui);
    }

    // Option/Result have Def::Option/Def::Result but Type::User(UserType::Enum),
    // so check into_option before into_enum to avoid misrouting.
    if let Ok(opt) = peek.into_option() {
        return show_inline_peek_option(opt, ui, id);
    }
    if let Ok(enu) = peek.into_enum() {
        if let Ok(variant) = enu.active_variant() {
            return ui.weak(variant.effective_name());
        }
        return ui.weak("enum");
    }
    if let Ok(_struc) = peek.into_struct() {
        return ui.weak(shape_display_name(peek.shape()));
    }
    if let Ok(list) = peek.into_list_like() {
        return ui.weak(alloc::format!("[{}]", list.len()));
    }
    if let Ok(map) = peek.into_map() {
        return ui.weak(alloc::format!("[{}]", map.len()));
    }
    if let Ok(tuple) = peek.into_tuple() {
        return ui.weak(alloc::format!("({})", tuple.len()));
    }
    if let Ok(ptr) = peek.into_pointer()
        && let Some(inner) = ptr.borrow_inner()
    {
        return show_inline_peek(inner, ui, id.with("ptr"), false);
    }

    ui.weak(shape_display_name(peek.shape()))
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
                let mut s = v;
                if s.contains('\n') {
                    let rows = text_line_count(s);
                    return ui.add_enabled(false, TextEdit::multiline(&mut s).desired_rows(rows));
                }
                return ui.add_enabled(false, TextEdit::singleline(&mut s));
            }
        }
        ScalarType::CowStr => {
            if let Ok(v) = peek.get::<Cow<'_, str>>() {
                let mut s = v.clone();
                if s.contains('\n') {
                    let rows = text_line_count(&s);
                    return ui.add_enabled(false, TextEdit::multiline(&mut s).desired_rows(rows));
                }
                return ui.add_enabled(false, TextEdit::singleline(&mut s));
            }
        }
        ScalarType::String => {
            if let Ok(v) = peek.get::<String>() {
                let mut s: Cow<'_, str> = Cow::Borrowed(v.as_str());
                if s.contains('\n') {
                    let rows = text_line_count(&s);
                    return ui.add_enabled(false, TextEdit::multiline(&mut s).desired_rows(rows));
                }
                return ui.add_enabled(false, TextEdit::singleline(&mut s));
            }
        }
        _ if peek.shape().is_display() => {
            return ui.label(alloc::format!("{}", peek));
        }
        _ if peek.shape().is_debug() => {
            return ui.label(alloc::format!("{:?}", peek));
        }
        _ => {}
    }
    ui.colored_label(
        Color32::YELLOW,
        alloc::format!("unsupported scalar: {scalar_type:?}"),
    )
}

fn show_inline_peek_option(opt: PeekOption<'_, '_>, ui: &mut Ui, id: Id) -> Response {
    ui.horizontal(|ui| {
        let is_some = opt.value().is_some();
        let _ = ui.selectable_label(!is_some, "None");
        let _ = ui.selectable_label(is_some, "Some");
        if let Some(inner) = opt.value() {
            show_inline_peek(inner, ui, id.with("some"), false);
        }
    })
    .response
}
