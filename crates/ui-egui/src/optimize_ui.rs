//! Optimize PDF ▸ Advanced optimization (Acrobat's PDF Optimizer): Images, Discard Objects,
//! Discard User Data and Clean Up panels. The result is saved as a copy, like Reduce File Size.

use egui::{Align, Layout};
use printcraft_engine::Hidden;
use printcraft_engine::optimize::{Compression, ImageSettings, QUALITIES, Settings};

use crate::theme::{self, Tokens};
use crate::{PrintCraftApp, widgets};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizeTab {
    Images,
    DiscardObjects,
    DiscardUserData,
    CleanUp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OptimizeDraft {
    pub tab: OptimizeTab,
    pub settings: Settings,
    /// Remove Hidden Information categories to discard.
    pub discard: Vec<Hidden>,
    /// "Audit space usage…" was pressed.
    pub audit: bool,
}

impl Default for OptimizeDraft {
    fn default() -> Self {
        Self { tab: OptimizeTab::Images, settings: Settings::default(), discard: Vec::new(), audit: false }
    }
}

fn image_row(ui: &mut egui::Ui, id: &str, title: &str, s: &mut ImageSettings) {
    ui.label(egui::RichText::new(title).font(theme::semibold(13.0)));
    ui.horizontal(|ui| {
        ui.checkbox(&mut s.downsample, crate::i18n::text("ui.bicubic_downsampling_to"));
        ui.add_enabled(s.downsample, egui::DragValue::new(&mut s.target_ppi).range(9.0..=2400.0).suffix(" ppi"));
        ui.label(crate::i18n::text("ui.for_images_above"));
        ui.add_enabled(s.downsample, egui::DragValue::new(&mut s.above_ppi).range(9.0..=2400.0).suffix(" ppi"));
    });
    s.above_ppi = s.above_ppi.max(s.target_ppi);
    ui.horizontal(|ui| {
        ui.label(crate::i18n::text("ui.compression"));
        let label = match s.compression {
            Compression::Jpeg(_) => "JPEG",
            Compression::Flate => "ZIP",
            Compression::Retain => crate::i18n::text("ui.retain_existing"),
        };
        egui::ComboBox::from_id_salt((id, "compression")).selected_text(label).show_ui(ui, |ui| {
            let q = match s.compression {
                Compression::Jpeg(q) => q,
                _ => 60,
            };
            ui.selectable_value(&mut s.compression, Compression::Jpeg(q), "JPEG");
            ui.selectable_value(&mut s.compression, Compression::Flate, "ZIP");
            ui.selectable_value(&mut s.compression, Compression::Retain, crate::i18n::text("ui.retain_existing"));
        });
        if let Compression::Jpeg(q) = &mut s.compression {
            ui.label(crate::i18n::text("ui.quality"));
            let name = QUALITIES.iter().min_by_key(|(_, v)| (*v as i32 - *q as i32).abs()).map_or(crate::i18n::text("ui.medium"), |(n, _)| n);
            egui::ComboBox::from_id_salt((id, "quality")).selected_text(name).show_ui(ui, |ui| {
                for (n, v) in QUALITIES {
                    ui.selectable_value(q, v, n);
                }
            });
        }
    });
    ui.add_space(8.0);
}

fn discard_box(ui: &mut egui::Ui, list: &mut Vec<Hidden>, h: Hidden, label: &str) {
    let mut on = list.contains(&h);
    if ui.checkbox(&mut on, label).changed() {
        if on {
            list.push(h);
        } else {
            list.retain(|x| *x != h);
        }
    }
}

/// Draw the dialog; returns (ok, cancel).
pub(crate) fn body(ui: &mut egui::Ui, d: &mut OptimizeDraft, t: &Tokens) -> (bool, bool) {
    ui.label(egui::RichText::new(crate::i18n::text("ui.pdf_optimizer")).font(theme::semibold(18.0)));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for (tab, label) in [
            (OptimizeTab::Images, crate::i18n::text("ui.images")),
            (OptimizeTab::DiscardObjects, crate::i18n::text("ui.discard_objects")),
            (OptimizeTab::DiscardUserData, crate::i18n::text("ui.discard_user_data")),
            (OptimizeTab::CleanUp, crate::i18n::text("ui.clean_up")),
        ] {
            if widgets::mode_tab(ui, label, d.tab == tab).clicked() {
                d.tab = tab;
            }
        }
    });
    ui.separator();
    ui.add_space(6.0);
    let s = &mut d.settings;
    match d.tab {
        OptimizeTab::Images => {
            image_row(ui, "color", crate::i18n::text("ui.color_images"), &mut s.color);
            image_row(ui, "gray", crate::i18n::text("ui.grayscale_images"), &mut s.gray);
            ui.label(
                egui::RichText::new(crate::i18n::text(
                    "ui.each_image_is_measured_where_pages_draw_it_an_image_is_replaced_only_if_the_result_is_smaller",
                ))
                .small()
                .color(t.text_muted),
            );
        }
        OptimizeTab::DiscardObjects => {
            discard_box(ui, &mut d.discard, Hidden::LinksActionsScripts, crate::i18n::text("ui.discard_all_links_actions_and_javascript"));
            ui.checkbox(&mut s.discard_alternate_images, crate::i18n::text("ui.discard_alternate_images"));
            ui.checkbox(&mut s.discard_thumbnails, crate::i18n::text("ui.discard_embedded_page_thumbnails"));
            ui.checkbox(&mut s.discard_tags, crate::i18n::text("ui.discard_document_tags"));
            ui.checkbox(&mut s.discard_print_settings, crate::i18n::text("ui.discard_embedded_print_settings"));
            discard_box(ui, &mut d.discard, Hidden::Bookmarks, crate::i18n::text("ui.discard_bookmarks"));
            discard_box(ui, &mut d.discard, Hidden::FormFields, crate::i18n::text("ui.flatten_form_fields"));
            discard_box(ui, &mut d.discard, Hidden::HiddenLayers, crate::i18n::text("ui.discard_hidden_layer_content"));
        }
        OptimizeTab::DiscardUserData => {
            discard_box(ui, &mut d.discard, Hidden::Comments, crate::i18n::text("ui.discard_all_comments_forms_and_multimedia"));
            discard_box(ui, &mut d.discard, Hidden::Metadata, crate::i18n::text("ui.discard_document_information_and_metadata"));
            discard_box(ui, &mut d.discard, Hidden::Attachments, crate::i18n::text("ui.discard_all_object_data_file_attachments"));
            discard_box(ui, &mut d.discard, Hidden::PrivateData, crate::i18n::text("ui.discard_private_data_of_other_applications"));
            discard_box(ui, &mut d.discard, Hidden::HiddenText, crate::i18n::text("ui.discard_hidden_text"));
        }
        OptimizeTab::CleanUp => {
            ui.checkbox(&mut s.flate_unencoded, crate::i18n::text("ui.use_flate_to_encode_streams_that_are_not_encoded"));
            ui.checkbox(&mut s.remove_invalid_links, crate::i18n::text("ui.remove_invalid_links_and_bookmarks"));
            ui.checkbox(&mut s.remove_unreferenced_dests, crate::i18n::text("ui.remove_unreferenced_named_destinations"));
            ui.add_enabled(false, egui::Checkbox::new(&mut true, crate::i18n::text("ui.compress_document_structure_object_streams")));
            ui.add_enabled(false, egui::Checkbox::new(&mut true, crate::i18n::text("ui.remove_unused_objects_and_merge_identical_ones")));
        }
    }
    ui.add_space(12.0);
    let (mut ok, mut cancel) = (false, false);
    ui.horizontal(|ui| {
        if widgets::pill_button(ui, crate::i18n::text("ui.audit_space_usage"), false).clicked() {
            d.audit = true;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, crate::i18n::text("ui.ok"), true).clicked() {
                ok = true;
            }
            if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                cancel = true;
            }
        });
    });
    (ok, cancel)
}

/// Audit Space Usage: bytes and share of the file per kind of content.
pub(crate) fn audit_body(ui: &mut egui::Ui, rows: &[printcraft_engine::optimize::SpaceUse], t: &Tokens) -> bool {
    ui.label(egui::RichText::new(crate::i18n::text("ui.space_audit")).font(crate::theme::semibold(18.0)));
    ui.add_space(8.0);
    egui::Grid::new("space-audit").num_columns(3).striped(true).spacing([24.0, 4.0]).show(ui, |ui| {
        for h in [crate::i18n::text("ui.description"), crate::i18n::text("ui.bytes"), crate::i18n::text("ui.percentage")] {
            ui.label(egui::RichText::new(h).color(t.text_muted));
        }
        ui.end_row();
        let total: u64 = rows.iter().map(|r| r.bytes).sum();
        for r in rows.iter().filter(|r| r.bytes > 0) {
            ui.label(r.category.label());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(r.bytes.to_string()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(format!("{:.2}%", r.percent)));
            ui.end_row();
        }
        ui.label(egui::RichText::new(crate::i18n::text("ui.total")).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(egui::RichText::new(total.to_string()).strong()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(egui::RichText::new("100.00%").strong()));
        ui.end_row();
    });
    ui.add_space(12.0);
    let mut ok = false;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| ok = widgets::pill_button(ui, crate::i18n::text("ui.ok"), true).clicked())
    });
    ok
}

impl PrintCraftApp {
    /// Optimize PDF with the dialog's choices and save the copy.
    pub fn optimize_with_draft(&mut self) {
        let _locale = crate::i18n::scope(self.language);
        let Some((_, id)) = self.active_ids() else { return };
        let d = self.optimize_draft.clone();
        let result = self.session.optimized_bytes(id, &d.settings, &d.discard).map(|(b, r)| {
            let o = &r.optimize;
            let mut parts = Vec::new();
            if o.images_resampled + o.images_recompressed > 0 {
                parts.push(crate::msg!(
                    value_image_value_optimized,
                    o.images_resampled + o.images_recompressed,
                    if o.images_resampled + o.images_recompressed == 1 { "" } else { "s" }
                ));
            }
            let discarded: usize = r.discarded.iter().map(|(_, n)| n).sum();
            if discarded > 0 {
                parts.push(crate::msg!(value_item_value_discarded, crate::i18n::plural_suffix(discarded), discarded = discarded));
            }
            (b, if parts.is_empty() { String::new() } else { format!("; {}", parts.join(", ")) })
        });
        self.save_optimized(id, "optimized", result);
    }
}
