use std::hash::Hash;

// -- ProbeHeader: collapsible section state, ported from egui-probe --

#[derive(Clone, Copy)]
struct ProbeHeaderState {
    has_inner: bool,
    open: bool,
    body_height: f32,
}

pub(crate) struct ProbeHeader {
    id: egui::Id,
    state: ProbeHeaderState,
    dirty: bool,
    pub openness: f32,
}

impl ProbeHeader {
    pub fn load(cx: &egui::Context, id: egui::Id) -> ProbeHeader {
        let state = cx.data_mut(|d| d.get_temp(id)).unwrap_or(ProbeHeaderState {
            has_inner: false,
            open: false,
            body_height: 0.0,
        });
        let openness = cx.animate_bool(id, state.open);
        ProbeHeader {
            id,
            state,
            dirty: false,
            openness,
        }
    }

    pub fn load_no_animation(cx: &egui::Context, id: egui::Id) -> ProbeHeader {
        let state = cx.data_mut(|d| d.get_temp(id)).unwrap_or(ProbeHeaderState {
            has_inner: false,
            open: false,
            body_height: 0.0,
        });
        let openness = if state.open { 1.0 } else { 0.0 };
        ProbeHeader {
            id,
            state,
            dirty: false,
            openness,
        }
    }

    pub fn store(self, cx: &egui::Context) {
        if self.dirty {
            cx.data_mut(|d| d.insert_temp(self.id, self.state));
            cx.request_repaint();
        }
    }

    pub fn has_inner(&self) -> bool {
        self.state.has_inner
    }

    pub fn set_has_inner(&mut self, has_inner: bool) {
        if self.state.has_inner != has_inner {
            self.state.has_inner = has_inner;
            self.dirty = true;
        }
    }

    pub fn toggle(&mut self) {
        self.state.open = !self.state.open;
        self.dirty = true;
    }

    pub fn set_open(&mut self, open: bool) {
        if self.state.open != open {
            self.state.open = open;
            self.dirty = true;
        }
    }

    pub fn set_body_height(&mut self, height: f32) {
        if (self.state.body_height - height).abs() > 0.001 {
            self.state.body_height = height;
            self.dirty = true;
        }
    }

    pub fn body_shift(&self) -> f32 {
        (1.0 - self.openness) * self.state.body_height
    }

    pub fn collapse_button(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = ui.spacing().icon_width_inner;
        let response =
            ui.allocate_response(egui::vec2(desired_size, desired_size), egui::Sense::click());
        if response.clicked() {
            self.toggle();
        }
        egui::collapsing_header::paint_default_icon(ui, self.openness, &response);
        response
    }
}

/// Swap the persisted `ProbeHeader` state between two ids. Used when reordering
/// list items so the open/closed state follows the moved item rather than the
/// index slot.
pub(crate) fn swap_probe_header_state(cx: &egui::Context, a: egui::Id, b: egui::Id) {
    if a == b {
        return;
    }
    cx.data_mut(|d| {
        let sa = d.get_temp::<ProbeHeaderState>(a);
        let sb = d.get_temp::<ProbeHeaderState>(b);
        match (sa, sb) {
            (Some(sa), Some(sb)) => {
                d.insert_temp(a, sb);
                d.insert_temp(b, sa);
            }
            (Some(sa), None) => {
                d.remove::<ProbeHeaderState>(a);
                d.insert_temp(b, sa);
            }
            (None, Some(sb)) => {
                d.insert_temp(a, sb);
                d.remove::<ProbeHeaderState>(b);
            }
            (None, None) => {}
        }
    });
    cx.request_repaint();
}

// -- ProbeLayout: two-column label/value layout, ported from egui-probe --

#[derive(Clone, Copy)]
pub(crate) struct ProbeLayoutState {
    labels_width: f32,
}

pub(crate) struct ProbeLayout {
    id: egui::Id,
    state: ProbeLayoutState,
    min_labels_width: f32,
}

impl ProbeLayout {
    pub fn load(cx: &egui::Context, id: egui::Id) -> ProbeLayout {
        let state = cx.data_mut(|d| *d.get_temp_mut_or(id, ProbeLayoutState { labels_width: 0.0 }));
        ProbeLayout {
            id,
            state,
            min_labels_width: 0.0,
        }
    }

    pub fn store(mut self, cx: &egui::Context) {
        if self.state.labels_width != self.min_labels_width {
            self.state.labels_width = self.min_labels_width;
            cx.data_mut(|d| d.insert_temp(self.id, self.state));
            cx.request_repaint();
        }
    }

    fn bump_labels_width(&mut self, width: f32) {
        if self.min_labels_width < width {
            self.min_labels_width = width;
        }
    }

    pub fn inner_label_ui(
        &mut self,
        indent: usize,
        id_salt: impl Hash,
        ui: &mut egui::Ui,
        add_content: impl FnOnce(&mut egui::Ui) -> egui::Response,
    ) -> egui::Response {
        let labels_width = self.state.labels_width;
        let cursor = ui.cursor();

        let max = egui::pos2(cursor.max.x.min(cursor.min.x + labels_width), cursor.max.y);
        let min = egui::pos2(cursor.min.x, cursor.min.y);
        let rect = egui::Rect::from_min_max(min, max);

        let mut label_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.intersect(ui.max_rect()))
                .layout(*ui.layout())
                .id_salt(id_salt),
        );
        label_ui.set_clip_rect(
            ui.clip_rect()
                .intersect(egui::Rect::everything_left_of(max.x)),
        );

        for _ in 0..indent {
            label_ui.separator();
        }

        let label_response = add_content(&mut label_ui);
        let mut final_rect = label_ui.min_rect();

        self.bump_labels_width(final_rect.width());

        final_rect.max.x = final_rect.min.x + labels_width;

        ui.advance_cursor_after_rect(final_rect);
        label_response
    }

    pub fn inner_value_ui(
        &mut self,
        id_salt: impl Hash,
        ui: &mut egui::Ui,
        add_content: impl FnOnce(&mut egui::Ui),
    ) {
        let mut value_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(ui.cursor().intersect(ui.max_rect()))
                .layout(*ui.layout())
                .id_salt(id_salt),
        );

        add_content(&mut value_ui);
        let final_rect = value_ui.min_rect();
        ui.advance_cursor_after_rect(final_rect);
    }
}
