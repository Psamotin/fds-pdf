//! Standards ▸ PDF/A: what the document declares, verifying it against PDF/A-2b or 3b, and
//! fixing what can be fixed (Save as PDF/A).

use egui::{Align, Layout};
use printcraft_engine::pdfa::{Issue, Level};

use crate::theme::{self, Tokens};
use crate::{PrintCraftApp, widgets};

/// The dialog's state: the chosen level and the last result.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfaState {
    pub level: Level,
    pub issues: Option<Vec<Issue>>,
    pub fixed: Vec<String>,
}

impl Default for PdfaState {
    fn default() -> Self {
        PdfaState { level: Level::A2b, issues: None, fixed: Vec::new() }
    }
}

impl PrintCraftApp {
    pub fn pdfa_verify(&mut self) {
        let _locale = crate::i18n::scope(self.language);
        let Some((_, id)) = self.active_ids() else { return };
        let level = self.pdfa.level;
        self.pdfa.issues = self.session.get(id).map(|d| d.pdfa_verify(level));
        self.pdfa.fixed.clear();
    }

    pub fn pdfa_convert(&mut self) {
        let _locale = crate::i18n::scope(self.language);
        let level = self.pdfa.level;
        if self.apply_edit(printcraft_engine::Edit::ConvertPdfA { level }) {
            self.pdfa_verify();
            let left = self.pdfa.issues.as_ref().map_or(0, Vec::len);
            self.notify(if left == 0 {
                crate::msg!(the_document_now_conforms_to_value_save_it_to_keep_the_changes, level.label())
            } else {
                crate::msg!(fixed_what_could_be_fixed_value_problem_value_remain, crate::i18n::plural_suffix(left), left = left)
            });
        }
    }
}

/// Returns `true` to close.
pub(crate) fn body(ui: &mut egui::Ui, app: &mut PrintCraftApp, t: &Tokens) -> bool {
    ui.label(egui::RichText::new("PDF/A").font(theme::semibold(18.0)));
    ui.add_space(6.0);
    let declared = app.active_ids().and_then(|(_, id)| app.session.get(id)).map(|d| d.standards()).unwrap_or_default();
    let shown = declared.pdfa.as_ref().map_or("none".to_string(), |(p, c)| format!("PDF/A-{p}{}", c.to_lowercase()));
    ui.label(crate::msg!(declared_conformance_value, shown = shown));
    if !declared.output_intents.is_empty() {
        ui.label(egui::RichText::new(crate::msg!(output_intent_value, declared.output_intents.join(", "))).color(t.text_muted));
    }
    ui.add_space(6.0);
    let before = app.pdfa.level;
    ui.horizontal(|ui| {
        ui.label(crate::i18n::text("ui.conformance_level"));
        egui::ComboBox::from_id_salt("pdfa-level").selected_text(app.pdfa.level.label()).show_ui(ui, |ui| {
            for l in [Level::A2b, Level::A3b] {
                ui.selectable_value(&mut app.pdfa.level, l, l.label());
            }
        });
    });
    if app.pdfa.level != before {
        app.pdfa.issues = None;
    }
    ui.add_space(6.0);
    egui::Frame::new().fill(t.hover).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            ui.set_min_height(120.0);
            match &app.pdfa.issues {
                None => {
                    ui.label(egui::RichText::new(crate::i18n::text("ui.verify_to_see_what_the_document_needs")).color(t.text_muted));
                }
                Some(v) if v.is_empty() => {
                    ui.label(crate::msg!(no_problems_found_the_document_conforms_to_value, app.pdfa.level.label()));
                }
                Some(v) => {
                    for i in v {
                        let page = i.page.map(|p| crate::msg!(page_value, p + 1)).unwrap_or_default();
                        let fix = if i.fixable { "" } else { crate::i18n::text("ui.not_fixable_here") };
                        ui.label(format!("{}{page}", i.message));
                        ui.label(egui::RichText::new(format!("ISO 19005 {}{fix}", i.clause)).small().color(t.text_muted));
                    }
                }
            }
        });
    });
    ui.add_space(10.0);
    let mut close = false;
    let mut action = None;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, crate::i18n::text("ui.save_as_pdf_a"), true).clicked() {
                action = Some(true);
            }
            if widgets::pill_button(ui, crate::i18n::text("ui.verify"), false).clicked() {
                action = Some(false);
            }
            if widgets::pill_button(ui, crate::i18n::text("ui.close"), false).clicked() {
                close = true;
            }
        });
    });
    match action {
        Some(true) => app.pdfa_convert(),
        Some(false) => app.pdfa_verify(),
        None => {}
    }
    close
}
