//! Protect Using Password (Acrobat's simplified dialog, screenshot 13b), with the classic
//! Password Security settings behind "Advanced Options" (compatibility, what to encrypt,
//! printing, changes, copying, screen readers). Execution plan M8.1.

use egui::{Align, Color32, Layout};
use printcraft_engine::{Changes, Edit, Printing, Protection};

use crate::theme::{self, Tokens};
use crate::{PrintCraftApp, widgets};

/// The dialog's state while it is open.
#[derive(Clone, Debug, PartialEq)]
pub struct ProtectDraft {
    /// `true`: password required for Viewing; `false`: for Editing.
    pub viewing: bool,
    pub password: String,
    pub confirm: String,
    pub advanced: bool,
    pub protection: Protection,
}

impl Default for ProtectDraft {
    fn default() -> Self {
        Self { viewing: true, password: String::new(), confirm: String::new(), advanced: false, protection: Protection::default() }
    }
}

impl ProtectDraft {
    /// Why Apply is disabled, if it is.
    pub fn problem(&self) -> Option<&'static str> {
        if self.password.is_empty() {
            Some(crate::i18n::text("ui.type_a_password"))
        } else if self.password != self.confirm {
            Some(crate::i18n::text("ui.the_passwords_don_t_match"))
        } else if self.protection.algorithm != printcraft_engine::Algorithm::Aes256 && !self.password.chars().all(|c| (' '..='~').contains(&c)) {
            Some(crate::i18n::text("ui.this_compatibility_level_supports_only_plain_ascii_passwords"))
        } else {
            None
        }
    }

    /// The edit Apply makes.
    pub fn edit(&self) -> Edit {
        let mut p = self.protection.clone();
        if self.viewing {
            p.open_password = Some(self.password.clone());
            p.permissions_password = None;
        } else {
            p.open_password = None;
            p.permissions_password = Some(self.password.clone());
        }
        Edit::Protect(p)
    }
}

/// A rough password strength, as Acrobat's meter shows (Weak / Medium / Strong).
pub fn strength(pw: &str) -> (&'static str, Color32) {
    let classes = [
        pw.chars().any(|c| c.is_lowercase()),
        pw.chars().any(|c| c.is_uppercase()),
        pw.chars().any(|c| c.is_ascii_digit()),
        pw.chars().any(|c| !c.is_alphanumeric()),
    ]
    .iter()
    .filter(|b| **b)
    .count();
    let n = pw.chars().count();
    if n >= 12 && classes >= 3 {
        (crate::i18n::text("ui.strong"), Color32::from_rgb(0x2D, 0x9D, 0x5B))
    } else if n >= 8 && classes >= 2 {
        (crate::i18n::text("ui.medium"), Color32::from_rgb(0xE8, 0x8A, 0x1A))
    } else {
        (crate::i18n::text("ui.weak"), Color32::from_rgb(0xD3, 0x2F, 0x2F))
    }
}

/// A radio button drawn like Acrobat's: an outlined circle, filled blue with a white dot when on.
fn radio(ui: &mut egui::Ui, t: &Tokens, on: bool, label: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(220.0, 26.0), egui::Sense::click());
    let c = egui::pos2(rect.left() + 9.0, rect.center().y);
    if on {
        ui.painter().circle_filled(c, 8.0, t.accent);
        ui.painter().circle_filled(c, 3.0, Color32::WHITE);
    } else {
        ui.painter().circle_stroke(c, 7.5, egui::Stroke::new(1.5, t.text_muted));
    }
    ui.painter().text(egui::pos2(rect.left() + 26.0, rect.center().y), egui::Align2::LEFT_CENTER, label, theme::regular(14.0), t.text);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Draw the dialog body; returns (apply, cancel).
pub(crate) fn body(ui: &mut egui::Ui, app: &mut PrintCraftApp, t: &Tokens) -> (bool, bool) {
    use printcraft_engine::Algorithm as A;
    let d = &mut app.protect_draft;
    ui.label(egui::RichText::new(crate::i18n::text("ui.protect_using_password_883cf3")).font(theme::semibold(18.0)));
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(crate::i18n::text("ui.requires_user_to_enter_a_password_for"));
    ui.add_space(4.0);
    if radio(ui, t, d.viewing, crate::i18n::text("ui.viewing")).clicked() {
        d.viewing = true;
    }
    if radio(ui, t, !d.viewing, crate::i18n::text("ui.editing")).clicked() {
        d.viewing = false;
    }
    ui.add_space(10.0);
    // Bordered fields (Acrobat: 1 pt #B1B1B1, radius 4).
    let field = |ui: &mut egui::Ui, label: &str, text: &mut String, id: &str| {
        let l = ui.label(label);
        egui::Frame::new()
            .stroke(egui::Stroke::new(1.0, t.border))
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.add(egui::TextEdit::singleline(text).password(true).desired_width(290.0).frame(egui::Frame::NONE).id_salt(id)).labelled_by(l.id)
            })
            .inner
    };
    let r = field(ui, crate::i18n::text("ui.type_password"), &mut d.password, "protect-pw");
    if r.changed() || !d.password.is_empty() {
        let (s, c) = strength(&d.password);
        if !d.password.is_empty() {
            ui.label(egui::RichText::new(crate::msg!(strength_value, s = s)).font(theme::medium(11.5)).color(c));
        }
    }
    ui.add_space(6.0);
    field(ui, crate::i18n::text("ui.re_type_password"), &mut d.confirm, "protect-pw2");
    ui.add_space(6.0);
    let chevron = if d.advanced { "⌃" } else { "⌄" };
    if ui
        .add(egui::Button::new(egui::RichText::new(crate::msg!(advanced_options_value, chevron = chevron)).font(theme::semibold(13.0))).frame(false))
        .clicked()
    {
        d.advanced = !d.advanced;
    }
    if d.advanced {
        let p = &mut d.protection;
        egui::Grid::new("protect-advanced").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(crate::i18n::text("ui.compatibility"));
            let algos = [
                (A::Aes256, crate::i18n::text("ui.acrobat_x_and_later_256_bit_aes")),
                (A::Aes128, crate::i18n::text("ui.acrobat_7_0_and_later_128_bit_aes")),
                (A::Rc4_128, crate::i18n::text("ui.acrobat_5_0_and_later_128_bit_rc4")),
                (A::Rc4_40, crate::i18n::text("ui.acrobat_3_0_and_later_40_bit_rc4")),
            ];
            let cur = algos.iter().find(|(a, _)| *a == p.algorithm).map_or(algos[0].1, |(_, l)| *l);
            egui::ComboBox::from_id_salt("protect-algo").width(300.0).selected_text(cur).show_ui(ui, |ui| {
                for (a, l) in algos {
                    ui.selectable_value(&mut p.algorithm, a, l);
                }
            });
            ui.end_row();
            ui.label(crate::i18n::text("ui.encrypt"));
            ui.vertical(|ui| {
                ui.radio_value(&mut p.encrypt_metadata, true, crate::i18n::text("ui.all_document_contents"));
                // Unencrypted metadata needs crypt filters (Acrobat 6.0 and later).
                ui.add_enabled_ui(p.algorithm != A::Rc4_40 && p.algorithm != A::Rc4_128, |ui| {
                    ui.radio_value(&mut p.encrypt_metadata, false, crate::i18n::text("ui.all_document_contents_except_metadata"));
                });
            });
            ui.end_row();
            if !d.viewing {
                ui.label(crate::i18n::text("ui.printing_allowed"));
                let opts = [
                    (Printing::None, crate::i18n::text("ui.none")),
                    (Printing::Low, crate::i18n::text("ui.low_resolution_150_dpi")),
                    (Printing::High, crate::i18n::text("ui.high_resolution_d95217")),
                ];
                let cur = opts.iter().find(|(o, _)| *o == p.printing).map_or(crate::i18n::text("ui.high_resolution_d95217"), |(_, l)| *l);
                egui::ComboBox::from_id_salt("protect-print").width(300.0).selected_text(cur).show_ui(ui, |ui| {
                    for (o, l) in opts {
                        ui.selectable_value(&mut p.printing, o, l);
                    }
                });
                ui.end_row();
                ui.label(crate::i18n::text("ui.changes_allowed"));
                let opts = [
                    (Changes::None, crate::i18n::text("ui.none")),
                    (Changes::Pages, crate::i18n::text("ui.inserting_deleting_and_rotating_pages")),
                    (Changes::FillSign, crate::i18n::text("ui.filling_in_form_fields_and_signing_existing_signature_fields")),
                    (Changes::CommentFillSign, crate::i18n::text("ui.commenting_filling_in_form_fields_and_signing_existing_signature_fields")),
                    (Changes::AnyExceptExtract, crate::i18n::text("ui.any_except_extracting_pages")),
                ];
                let cur = opts.iter().find(|(o, _)| *o == p.changes).map_or(crate::i18n::text("ui.none"), |(_, l)| *l);
                egui::ComboBox::from_id_salt("protect-changes").width(300.0).selected_text(cur).show_ui(ui, |ui| {
                    for (o, l) in opts {
                        ui.selectable_value(&mut p.changes, o, l);
                    }
                });
                ui.end_row();
                ui.label("");
                ui.vertical(|ui| {
                    ui.checkbox(&mut p.copy, crate::i18n::text("ui.enable_copying_of_text_images_and_other_content"));
                    ui.checkbox(&mut p.accessibility, crate::i18n::text("ui.enable_text_access_for_screen_reader_devices_for_the_visually_impaired"));
                });
                ui.end_row();
            }
        });
        if p.algorithm == A::Rc4_40 || p.algorithm == A::Rc4_128 {
            p.encrypt_metadata = true;
        }
    }
    ui.add_space(8.0);
    if let Some(problem) = d.problem().filter(|_| !d.password.is_empty() || !d.confirm.is_empty()) {
        ui.label(egui::RichText::new(problem).color(t.text_muted).font(theme::regular(12.0)));
    }
    ui.label(
        egui::RichText::new(if d.viewing {
            crate::i18n::text("ui.anyone_opening_the_document_will_need_this_password_it_s_applied_when_you_save")
        } else {
            crate::i18n::text(
                "ui.the_document_opens_without_a_password_this_password_is_needed_to_change_the_restrictions_it_s_applied_when_you_save",
            )
        })
        .color(t.text_faint)
        .font(theme::regular(11.5)),
    );
    ui.add_space(12.0);
    let (mut apply, mut cancel) = (false, false);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let ok = d.problem().is_none();
        if ui.add_enabled_ui(ok, |ui| widgets::pill_button(ui, crate::i18n::text("ui.apply"), true)).inner.clicked() {
            apply = true;
        }
        if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
            cancel = true;
        }
    });
    (apply, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_validate_and_map_to_the_right_password() {
        let _locale = crate::i18n::scope(crate::i18n::Language::En);
        let mut d = ProtectDraft::default();
        assert!(d.problem().is_some());
        d.password = "abc".into();
        d.confirm = "abd".into();
        assert_eq!(d.problem(), Some("The passwords don't match."));
        d.confirm = "abc".into();
        assert_eq!(d.problem(), None);
        match d.edit() {
            Edit::Protect(p) => assert_eq!((p.open_password.as_deref(), p.permissions_password), (Some("abc"), None)),
            other => panic!("{other:?}"),
        }
        d.viewing = false;
        match d.edit() {
            Edit::Protect(p) => assert_eq!((p.open_password, p.permissions_password.as_deref()), (None, Some("abc"))),
            other => panic!("{other:?}"),
        }
        assert_eq!(strength("abc").0, "Weak");
        assert_eq!(strength("Abcdefg1").0, "Medium");
        assert_eq!(strength("Correct-Horse-9").0, "Strong");
    }
}
