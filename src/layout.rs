use std::hash::Hash;

#[derive(Clone, Copy)]
pub struct ProbeLayoutState {
    labels_width: f32,
}

pub struct ProbeLayout {
    id: egui::Id,
    state: ProbeLayoutState,
    min_labels_width: f32,
}

impl ProbeLayout {
    fn load(cx: &egui::Context, id: egui::Id) -> ProbeLayout {
        let state = cx.data_mut(|d| *d.get_temp_mut_or(id, ProbeLayoutState { labels_width: 0.0 }));
        ProbeLayout {
            id,
            state,
            min_labels_width: 0.0,
        }
    }

    fn store(mut self, cx: &egui::Context) {
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
