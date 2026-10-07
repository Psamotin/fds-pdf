//! Redact a PDF (execution plan M8.5–M8.6): mark text and areas with the Redact tool (drag across
//! text to mark it, drag elsewhere to mark a box), mark whole pages, find text or patterns and
//! mark every match, set the default box colour and overlay text, and apply all marks (with a
//! confirmation, as Acrobat asks) or clear them.

use egui::{Color32, CornerRadius, Pos2, Rect, Stroke};
use printcraft_engine::{Edit, NewAnnotation, REDACT_PATTERNS, RedactPattern, Rgb, Shape, Style, rect_quad};
use printcraft_render::DocInfo;

use crate::canvas::{DocView, PageXform};
use crate::theme::Tokens;
use crate::{PrintCraftApp, widgets};

const MARK_RED: Color32 = Color32::from_rgb(0xE3, 0x22, 0x22);

/// Redaction Tool Properties: what new marks look like once applied.
#[derive(Clone, Debug, PartialEq)]
pub struct RedactPrefs {
    /// Box colour (`None` = no box, the content just disappears).
    pub fill: Option<Rgb>,
    pub use_overlay: bool,
    pub overlay: String,
    /// Overlay text font, size (0 = auto), colour, alignment and repetition.
    pub look: printcraft_engine::OverlayLook,
}

impl Default for RedactPrefs {
    fn default() -> Self {
        Self { fill: Some([0.0, 0.0, 0.0]), use_overlay: false, overlay: String::new(), look: Default::default() }
    }
}

impl RedactPrefs {
    pub fn mark(&self, page: usize, quads: Vec<[f64; 8]>, author: &str) -> Edit {
        let shape = Shape::Redact { quads, overlay: if self.use_overlay { self.overlay.clone() } else { String::new() }, look: self.look };
        let mut style = Style::default_for(&shape);
        style.fill = self.fill;
        Edit::AddAnnotation(NewAnnotation { page, shape, style, contents: String::new(), author: author.to_string() })
    }
}

/// Redact pages: which pages to mark.
#[derive(Clone, Debug, PartialEq)]
pub struct PagesDraft {
    pub current: bool,
    pub from: usize,
    pub to: usize,
}

impl Default for PagesDraft {
    fn default() -> Self {
        Self { current: true, from: 1, to: 1 }
    }
}

/// Find text and redact.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchDraft {
    pub patterns: bool,
    pub text: String,
    pub pattern: RedactPattern,
    /// The result of the last search (matches marked), shown in the dialog.
    pub found: Option<usize>,
}

impl Default for SearchDraft {
    fn default() -> Self {
        Self { patterns: false, text: String::new(), pattern: RedactPattern::Phone, found: None }
    }
}

/// A box being drawn with the Redact tool: (page, start).
pub type AreaDrag = Option<(usize, Pos2)>;

fn to_user(xf: &PageXform, info: &DocInfo, page: usize, p: Pos2) -> [f64; 2] {
    let (vx, vy) = xf.screen_to_view(p);
    let u = info.pages[page].view_to_user(vx, vy);
    [u[0] as f64, u[1] as f64]
}

/// Redact tool input on a page. Presses on text are left to text selection (the selection is
/// marked when it ends, see [`after_text`]); presses elsewhere draw a box. Returns `true` when
/// the gesture is a box.
pub(crate) fn page_input(
    ui: &egui::Ui,
    resp: &egui::Response,
    xf: &PageXform,
    page: usize,
    info: &DocInfo,
    over_text: impl Fn(Pos2) -> bool,
    view: &mut DocView,
) -> bool {
    let pointer = ui.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));
    if let Some((dp, start)) = view.redact_drag
        && dp == page
    {
        if resp.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
            view.redact_drag = None;
            let end = pointer.unwrap_or(start);
            let r = Rect::from_two_pos(start, end).intersect(xf.rect);
            if r.width() >= 3.0 && r.height() >= 3.0 {
                let (a, b) = (to_user(xf, info, page, r.min), to_user(xf, info, page, r.max));
                view.pending_redaction = Some((page, vec![rect_quad([a[0], a[1], b[0], b[1]])]));
            }
        }
        return true;
    }
    let Some(p) = pointer.filter(|p| xf.rect.contains(*p)) else { return false };
    if !over_text(p) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
        if !over_text(origin) {
            view.redact_drag = Some((page, origin));
            return true;
        }
    }
    false
}

/// A finished text selection with the Redact tool becomes a mark.
pub(crate) fn after_text(resp: &egui::Response, page: usize, info: &DocInfo, view: &mut DocView) {
    if !(resp.drag_stopped() || resp.double_clicked()) || view.redact_drag.is_some() {
        return;
    }
    if let Some((p, quads)) = view.selection_quads(info).filter(|(p, _)| *p == page) {
        view.clear_selection();
        view.pending_redaction = Some((p, quads));
    }
}

/// The box being drawn.
pub(crate) fn paint(ui: &egui::Ui, painter: &egui::Painter, page: usize, view: &DocView) {
    if let (Some((dp, start)), Some(p)) = (view.redact_drag, ui.input(|i| i.pointer.hover_pos()))
        && dp == page
    {
        let r = Rect::from_two_pos(start, p);
        painter.rect_filled(r, CornerRadius::ZERO, MARK_RED.gamma_multiply(0.12));
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.5, MARK_RED), egui::StrokeKind::Inside);
    }
}

impl PrintCraftApp {
    /// Mark every match of the search draft on every page; returns how many were marked.
    pub fn redact_search(&mut self) -> usize {
        let _locale = crate::i18n::scope(self.language);
        let Some((_, id)) = self.active_ids() else { return 0 };
        let Some(doc) = self.session.get(id) else { return 0 };
        let d = self.redact_search.clone();
        if !d.patterns && d.text.trim().is_empty() {
            return 0;
        }
        let config = printcraft_render::RenderConfig { password: doc.password.as_deref().map(std::sync::Arc::from), ..Default::default() };
        let mut r = printcraft_render::PageRenderer::new(doc.bytes.clone(), config);
        let mut edits = Vec::new();
        let author = self.comment_prefs.author.clone();
        for page in 0..doc.info.pages.len() {
            let out =
                r.render(printcraft_render::RenderRequest { page, kind: printcraft_render::RequestKind::Text, scale: 1.0, ..Default::default() });
            let Some(text) = out.text else { continue };
            let hits = if d.patterns { text.find_with(|c| printcraft_engine::find_pattern(d.pattern, c)) } else { text.find(&d.text) };
            for h in hits {
                let quads: Vec<[f64; 8]> = text.line_rects(h).into_iter().map(|r| doc.info.pages[page].view_rect_to_quad(r)).collect();
                if !quads.is_empty() {
                    edits.push(self.redact_prefs.mark(page, quads, &author));
                }
            }
        }
        let n = edits.len();
        if n > 0 {
            self.apply_edit(Edit::Batch { label: "Mark for redaction".into(), edits });
        }
        n
    }

    /// Mark the pages of the Redact pages draft.
    pub fn redact_pages(&mut self) {
        let _locale = crate::i18n::scope(self.language);
        let Some((i, id)) = self.active_ids() else { return };
        let Some(doc) = self.session.get(id) else { return };
        let n = doc.info.pages.len();
        let d = self.redact_pages_draft.clone();
        let pages: Vec<usize> = if d.current { vec![self.views[i].current] } else { (d.from.max(1) - 1..d.to.min(n)).collect() };
        let author = self.comment_prefs.author.clone();
        let edits: Vec<Edit> = pages
            .iter()
            .map(|&p| {
                let c = doc.info.pages[p].crop;
                self.redact_prefs.mark(p, vec![rect_quad([c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64])], &author)
            })
            .collect();
        match <[Edit; 1]>::try_from(edits) {
            Ok([one]) => {
                self.apply_edit(one);
            }
            Err(edits) if edits.is_empty() => {}
            Err(edits) => {
                self.apply_edit(Edit::Batch { label: "Mark pages for redaction".into(), edits });
            }
        }
    }
}

/// Redact pages ("Mark Page Range"). Returns (apply, cancel).
pub(crate) fn pages_body(ui: &mut egui::Ui, d: &mut PagesDraft, pages: usize, _t: &Tokens) -> (bool, bool) {
    ui.label(egui::RichText::new(crate::i18n::text("ui.mark_page_range")).font(crate::theme::semibold(18.0)));
    ui.add_space(8.0);
    ui.radio_value(&mut d.current, true, crate::i18n::text("ui.current_page"));
    ui.horizontal(|ui| {
        ui.radio_value(&mut d.current, false, crate::i18n::text("ui.pages_from"));
        ui.add_enabled(!d.current, egui::DragValue::new(&mut d.from).range(1..=pages));
        ui.label(crate::i18n::text("ui.to_663ea1"));
        ui.add_enabled(!d.current, egui::DragValue::new(&mut d.to).range(1..=pages));
        ui.label(crate::msg!(of_value, pages = pages));
    });
    d.to = d.to.max(d.from);
    ui.add_space(12.0);
    buttons(ui, crate::i18n::text("ui.ok"), true)
}

/// Find text and redact. Returns (search, cancel).
pub(crate) fn search_body(ui: &mut egui::Ui, d: &mut SearchDraft, t: &Tokens) -> (bool, bool) {
    ui.set_width(420.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.find_text_and_redact")).font(crate::theme::semibold(18.0)));
    ui.add_space(8.0);
    ui.radio_value(&mut d.patterns, false, crate::i18n::text("ui.single_word_or_phrase"));
    let mut enter = false;
    ui.add_enabled_ui(!d.patterns, |ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut d.text).hint_text(crate::i18n::text("ui.text_to_find")).desired_width(380.0));
        enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    });
    ui.add_space(6.0);
    ui.radio_value(&mut d.patterns, true, crate::i18n::text("ui.patterns"));
    ui.add_enabled_ui(d.patterns, |ui| {
        egui::ComboBox::from_id_salt("redact-pattern").selected_text(crate::i18n::current().tr(d.pattern.label())).width(240.0).show_ui(ui, |ui| {
            for p in REDACT_PATTERNS {
                ui.selectable_value(&mut d.pattern, p, crate::i18n::current().tr(p.label()));
            }
        });
    });
    ui.add_space(6.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.text_in_images_isn_t_found_recognise_text_ocr_first")).small().color(t.text_faint));
    if let Some(n) = d.found {
        ui.label(
            egui::RichText::new(if n == 0 {
                crate::i18n::text("ui.no_matches").to_string()
            } else {
                crate::msg!(value_match_es_marked_for_redaction, n = n)
            })
            .color(t.text_muted),
        );
    }
    ui.add_space(12.0);
    let (go, cancel) = buttons(ui, crate::i18n::text("ui.mark_all"), true);
    (go || enter, cancel)
}

/// Redaction Tool Properties. Returns (apply, cancel).
pub(crate) fn props_body(ui: &mut egui::Ui, d: &mut RedactPrefs, _t: &Tokens) -> (bool, bool) {
    ui.set_width(380.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.redaction_tool_properties")).font(crate::theme::semibold(18.0)));
    ui.add_space(8.0);
    egui::Grid::new("redact-props").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
        ui.label(crate::i18n::text("ui.redacted_area_fill_colour"));
        ui.horizontal(|ui| {
            let mut none = d.fill.is_none();
            if ui.checkbox(&mut none, crate::i18n::text("ui.no_colour")).changed() {
                d.fill = if none { None } else { Some([0.0, 0.0, 0.0]) };
            }
        });
        ui.end_row();
        ui.label("");
        if let Some(c) = crate::comments::swatch_grid(ui, d.fill) {
            d.fill = Some(c);
        }
        ui.end_row();
        ui.label("");
        ui.checkbox(&mut d.use_overlay, crate::i18n::text("ui.use_overlay_text"));
        ui.end_row();
        let l = ui.label(crate::i18n::text("ui.custom_text"));
        ui.add_enabled(d.use_overlay, egui::TextEdit::singleline(&mut d.overlay).desired_width(220.0)).labelled_by(l.id);
        ui.end_row();
        let on = d.use_overlay;
        let look = &mut d.look;
        ui.label(crate::i18n::text("ui.font"));
        ui.add_enabled_ui(on, |ui| {
            egui::ComboBox::from_id_salt("overlay-font").selected_text(crate::i18n::current().tr(look.font.name())).show_ui(ui, |ui| {
                for f in printcraft_engine::OverlayFont::ALL {
                    ui.selectable_value(&mut look.font, f, crate::i18n::current().tr(f.name()));
                }
            });
        });
        ui.end_row();
        ui.label(crate::i18n::text("ui.font_size_9b3242"));
        ui.add_enabled_ui(on, |ui| {
            ui.horizontal(|ui| {
                let mut auto = look.size <= 0.0;
                if ui.checkbox(&mut auto, crate::i18n::text("ui.auto_size_text_to_fit_redaction_region")).changed() {
                    look.size = if auto { 0.0 } else { 10.0 };
                }
                if !auto {
                    ui.add(egui::DragValue::new(&mut look.size).range(2.0..=144.0).suffix(crate::i18n::text("ui.pt")));
                }
            });
        });
        ui.end_row();
        ui.label(crate::i18n::text("ui.font_colour"));
        ui.add_enabled_ui(on, |ui| {
            if let Some(c) = crate::comments::swatch_grid(ui, Some(look.color)) {
                look.color = c;
            }
        });
        ui.end_row();
        ui.label("");
        ui.add_enabled(on, egui::Checkbox::new(&mut look.repeat, crate::i18n::text("ui.repeat_overlay_text")));
        ui.end_row();
        ui.label(crate::i18n::text("ui.text_alignment"));
        ui.add_enabled_ui(on, |ui| {
            ui.horizontal(|ui| {
                for (a, label) in [(0u8, crate::i18n::text("ui.left")), (1, crate::i18n::text("ui.center")), (2, crate::i18n::text("ui.right"))] {
                    ui.radio_value(&mut look.align, a, label);
                }
            });
        });
        ui.end_row();
    });
    ui.add_space(12.0);
    buttons(ui, crate::i18n::text("ui.ok"), true)
}

/// Apply redactions confirmation. Returns (apply, cancel).
pub(crate) fn apply_body(ui: &mut egui::Ui, marks: usize, t: &Tokens) -> (bool, bool) {
    ui.set_width(420.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.apply_redactions")).font(crate::theme::semibold(18.0)));
    ui.add_space(8.0);
    ui.label(crate::msg!(you_are_about_to_apply_value_redaction_mark_value_text_images_and_drawings_under_the_marks_and_comments_and_form_fields_that_overlap_them_are_removed_permanently,
        crate::i18n::plural_suffix(marks)
    , marks = marks));
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(crate::i18n::text(
            "ui.save_the_document_afterwards_saving_rewrites_the_whole_file_so_no_trace_of_the_removed_content_stays_in_it",
        ))
        .small()
        .color(t.text_muted),
    );
    ui.add_space(12.0);
    buttons(ui, crate::i18n::text("ui.apply"), true)
}

fn buttons(ui: &mut egui::Ui, ok: &str, primary: bool) -> (bool, bool) {
    let (mut a, mut c) = (false, false);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if widgets::pill_button(ui, ok, primary).clicked() {
            a = true;
        }
        if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
            c = true;
        }
    });
    (a, c)
}

/// Remove Hidden Information: each category with what was found, checked by default when
/// something was.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HiddenDraft {
    pub found: Vec<(printcraft_engine::Hidden, usize, bool)>,
}

impl PrintCraftApp {
    pub fn open_remove_hidden(&mut self) {
        let _locale = crate::i18n::scope(self.language);
        let Some((_, id)) = self.active_ids() else { return };
        let Some(doc) = self.session.get(id) else { return };
        self.hidden_draft = HiddenDraft { found: doc.hidden_info().into_iter().map(|(h, n)| (h, n, n > 0)).collect() };
        self.dialog = Some(crate::Dialog::RemoveHidden);
    }
}

/// Returns (remove, cancel).
pub(crate) fn hidden_body(ui: &mut egui::Ui, d: &mut HiddenDraft, t: &Tokens) -> (bool, bool) {
    ui.set_width(440.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.remove_hidden_information")).font(crate::theme::semibold(18.0)));
    ui.add_space(4.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.select_the_items_to_remove_from_this_document")).color(t.text_muted));
    ui.add_space(8.0);
    let total: usize = d.found.iter().map(|f| f.1).sum();
    for (h, n, on) in d.found.iter_mut() {
        ui.add_enabled_ui(*n > 0, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(on, crate::i18n::current().tr(h.label()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(if *n == 0 { crate::i18n::text("ui.none_found").to_string() } else { n.to_string() }).color(t.text_muted),
                    );
                });
            });
        });
    }
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(crate::i18n::text("ui.form_fields_are_flattened_their_values_stay_visible_saving_rewrites_the_whole_file"))
            .small()
            .color(t.text_faint),
    );
    ui.add_space(12.0);
    let any = d.found.iter().any(|f| f.2 && f.1 > 0);
    let (mut a, mut c) = (false, false);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.add_enabled_ui(any && total > 0, |ui| widgets::pill_button(ui, crate::i18n::text("ui.remove"), true)).inner.clicked() {
            a = true;
        }
        if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
            c = true;
        }
    });
    (a, c)
}

/// Sanitize Document confirmation. Returns (sanitize, cancel).
pub(crate) fn sanitize_body(ui: &mut egui::Ui, t: &Tokens) -> (bool, bool) {
    ui.set_width(440.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.sanitize_document")).font(crate::theme::semibold(18.0)));
    ui.add_space(8.0);
    ui.label(crate::i18n::text("ui.sanitizing_removes_hidden_information_from_the_document_metadata_file_attachments_comments_form_fields_flattened_hidden_text_and_layers_bookmarks_links_actions_and_scripts_and_private_application_data"));
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(crate::i18n::text("ui.save_the_document_afterwards_saving_rewrites_the_whole_file_so_nothing_removed_stays_in_it"))
            .small()
            .color(t.text_muted),
    );
    ui.add_space(12.0);
    buttons(ui, crate::i18n::text("ui.sanitize"), true)
}
