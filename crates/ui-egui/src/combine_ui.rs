//! Combine files: the files to combine, in order, each with the pages to take (all, or a range
//! such as "1-3, 6"); reorder, remove or add files, then Combine.

use std::sync::Arc;

use egui::{Align, Layout};

use crate::theme::{self, Tokens};
use crate::{Dialog, PrintCraftApp, icons, widgets};

#[derive(Clone, Debug)]
pub struct CombineFile {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    /// The pages to take ("" = all).
    pub range: String,
}

enum RowAction {
    Up(usize),
    Down(usize),
    Remove(usize),
}

pub(crate) fn body(ui: &mut egui::Ui, app: &mut PrintCraftApp, t: &Tokens) -> (bool, bool) {
    ui.label(egui::RichText::new(crate::i18n::text("ui.combine_files")).font(theme::semibold(18.0)));
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(crate::i18n::text("ui.files_are_combined_in_this_order_leave_pages_empty_to_take_every_page"))
            .small()
            .color(t.text_faint),
    );
    ui.add_space(8.0);
    let mut action = None;
    let n = app.combine_draft.len();
    egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("combine-files").num_columns(4).min_col_width(40.0).spacing([10.0, 6.0]).striped(true).show(ui, |ui| {
            ui.label(egui::RichText::new(crate::i18n::text("ui.file")).color(t.text_muted));
            ui.label(egui::RichText::new(crate::i18n::text("ui.pages")).color(t.text_muted));
            ui.label("");
            ui.label("");
            ui.end_row();
            for (i, f) in app.combine_draft.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(icons::image("file-text", 16.0, t.icon));
                    ui.add(egui::Label::new(&f.name).truncate());
                    ui.label(
                        egui::RichText::new(crate::msg!(value_page_value, f.pages, crate::i18n::plural_suffix(f.pages))).small().color(t.text_faint),
                    );
                });
                ui.add_sized([120.0, 22.0], egui::TextEdit::singleline(&mut f.range).hint_text(crate::i18n::text("ui.all_pages")))
                    .on_hover_text(crate::msg!(pages_of_value_to_combine_e_g_1_3_6, f.name));
                ui.horizontal(|ui| {
                    if ui.add_enabled_ui(i > 0, |ui| icons::button(ui, "chevron-up", 24.0, false, crate::i18n::text("ui.move_up"))).inner.clicked() {
                        action = Some(RowAction::Up(i));
                    }
                    if ui
                        .add_enabled_ui(i + 1 < n, |ui| icons::button(ui, "chevron-down", 24.0, false, crate::i18n::text("ui.move_down")))
                        .inner
                        .clicked()
                    {
                        action = Some(RowAction::Down(i));
                    }
                });
                if icons::button(ui, "trash-2", 24.0, false, &crate::msg!(remove_value, f.name)).clicked() {
                    action = Some(RowAction::Remove(i));
                }
                ui.end_row();
            }
        });
    });
    match action {
        Some(RowAction::Up(i)) => app.combine_draft.swap(i, i - 1),
        Some(RowAction::Down(i)) => app.combine_draft.swap(i, i + 1),
        Some(RowAction::Remove(i)) => {
            app.combine_draft.remove(i);
        }
        None => {}
    }
    ui.add_space(10.0);
    let (mut go, mut cancel) = (false, false);
    ui.horizontal(|ui| {
        if widgets::pill_button(ui, crate::i18n::text("ui.add_files"), false).clicked() {
            app.combine_dialog();
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let ok = app.combine_draft.len() >= 2;
            if ui
                .add_enabled_ui(ok, |ui| widgets::pill_button(ui, crate::i18n::text("ui.combine"), true))
                .inner
                .on_disabled_hover_text(crate::i18n::text("ui.add_at_least_two_files"))
                .clicked()
            {
                go = true;
            }
            if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                cancel = true;
            }
        });
    });
    (go, cancel)
}

impl PrintCraftApp {
    /// Add picked files to the Combine files list (and show it).
    pub(crate) fn stage_combine(&mut self, files: Vec<(String, Vec<u8>)>) {
        let _locale = crate::i18n::scope(self.language);
        for (name, bytes) in files {
            let bytes = Arc::new(bytes);
            match printcraft_render::inspect(bytes.clone(), None) {
                Ok(info) => self.combine_draft.push(CombineFile { name, bytes, pages: info.pages.len(), range: String::new() }),
                Err(e) => self.notify(crate::msg!(couldn_t_add_value_value, name = name, e = e)),
            }
        }
        self.dialog = Some(Dialog::Combine);
    }

    /// Combine the listed files into a new, unsaved document.
    pub fn combine_staged(&mut self) {
        let _locale = crate::i18n::scope(self.language);
        let files = std::mem::take(&mut self.combine_draft);
        if files.is_empty() {
            return;
        }
        let count = files.len();
        let sources: Vec<(String, Arc<Vec<u8>>, Option<String>)> = files
            .into_iter()
            .map(|f| (crate::files::strip_pdf(&f.name).to_string(), f.bytes, Some(f.range).filter(|r| !r.trim().is_empty())))
            .collect();
        match self.session.combine_ranges(&sources) {
            Ok(bytes) => self.open_created("Combined.pdf", bytes, &crate::msg!(combined_value_files, count = count)),
            Err(e) => self.notify(crate::msg!(couldn_t_combine_files_value, e = e)),
        }
    }
}
