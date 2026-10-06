//! Add Header and Footer, Add Watermark and Add Background dialogs (Acrobat: Edit a PDF ▸ …;
//! dialog snapshots d13, d14, d12d). Each has the shared Page Range Options and a schematic
//! preview of the current page. "Update" opens the same dialog in replace mode.

use egui::{Align, Color32, CornerRadius, Layout, Rect, Stroke, pos2, vec2};
use printcraft_engine::{Background, Edit, HeaderFooter, MarkKind, Watermark};

use crate::theme::{self, Tokens};
use crate::{PrintCraftApp, widgets};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subset {
    All,
    Even,
    Odd,
}

/// Page Range Options (shared by the three dialogs). Pages are 1-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageRange {
    pub all: bool,
    pub from: usize,
    pub to: usize,
    pub subset: Subset,
}

impl Default for PageRange {
    fn default() -> Self {
        Self { all: true, from: 1, to: 1, subset: Subset::All }
    }
}

impl PageRange {
    /// The pages (0-based) the range selects in a document of `count` pages.
    pub fn pages(&self, count: usize) -> Vec<usize> {
        let (a, b) = if self.all { (1, count) } else { (self.from.max(1), self.to.min(count)) };
        (a..=b)
            .filter(|p| match self.subset {
                Subset::All => true,
                Subset::Even => p % 2 == 0,
                Subset::Odd => p % 2 == 1,
            })
            .map(|p| p - 1)
            .collect()
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, count: usize) {
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.all, true, crate::i18n::text("ui.all_pages"));
            ui.radio_value(&mut self.all, false, crate::i18n::text("ui.pages_from"));
            ui.add_enabled(!self.all, egui::DragValue::new(&mut self.from).range(1..=count.max(1)));
            ui.label(crate::i18n::text("ui.to_663ea1"));
            if self.to == 1 && self.from == 1 {
                self.to = count.max(1);
            }
            ui.add_enabled(!self.all, egui::DragValue::new(&mut self.to).range(1..=count.max(1)));
            ui.label(crate::i18n::text("ui.subset"));
            let label = match self.subset {
                Subset::All => crate::i18n::text("ui.all_pages_in_range"),
                Subset::Even => crate::i18n::text("ui.even_pages_only_6e47e0"),
                Subset::Odd => crate::i18n::text("ui.odd_pages_only_237050"),
            };
            egui::ComboBox::from_id_salt("mark-subset").selected_text(label).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.subset, Subset::All, crate::i18n::text("ui.all_pages_in_range"));
                ui.selectable_value(&mut self.subset, Subset::Even, crate::i18n::text("ui.even_pages_only_6e47e0"));
                ui.selectable_value(&mut self.subset, Subset::Odd, crate::i18n::text("ui.odd_pages_only_237050"));
            });
        });
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarksDraft {
    pub hf: HeaderFooter,
    pub wm: Watermark,
    pub bg: Background,
    pub range: PageRange,
    /// Replace existing marks of this kind (Update).
    pub replace: bool,
    /// Which header/footer box Insert Page Number / Insert Date type into.
    pub focused_box: usize,
    pub page_format: usize,
    pub date_format: usize,
    /// Watermark: size the text to the page.
    pub fit: bool,
    /// Source ▸ File (watermark or background): the file picked, and the page of a PDF (1-based).
    pub use_file: bool,
    pub file: Option<(String, std::sync::Arc<Vec<u8>>)>,
    pub file_page: usize,
}

impl Default for MarksDraft {
    fn default() -> Self {
        Self {
            hf: HeaderFooter::default(),
            wm: Watermark::default(),
            bg: Background::default(),
            range: PageRange::default(),
            replace: false,
            focused_box: 1,
            page_format: 0,
            date_format: 0,
            fit: true,
            use_file: false,
            file: None,
            file_page: 1,
        }
    }
}

const PAGE_FORMATS: [&str; 5] = ["<<1>>", "<<1 of n>>", "<<1/n>>", "<<Page 1>>", "<<Page 1 of n>>"];
const DATE_FORMATS: [&str; 6] = ["<<m/d/yyyy>>", "<<mm/dd/yyyy>>", "<<d/m/yyyy>>", "<<yyyy-mm-dd>>", "<<m/d/yy>>", "<<mmmm d, yyyy>>"];
const BOX_NAMES: [&str; 6] =
    ["Left Header Text", "Center Header Text", "Right Header Text", "Left Footer Text", "Center Footer Text", "Right Footer Text"];

fn color_button(ui: &mut egui::Ui, c: &mut [f64; 3]) {
    // Keep PDF colours as linear RGB; translate only the controls. The egui picker
    // owns hard-coded English labels, so this small desktop popup uses our resources.
    let swatch = Color32::from(egui::Rgba::from_rgb(c[0] as f32, c[1] as f32, c[2] as f32));
    let contrast = if egui::Rgba::from(swatch).intensity() < 0.5 { Color32::WHITE } else { Color32::BLACK };
    let response = ui.add(egui::Button::new(egui::RichText::new(crate::i18n::text("ui.colour")).color(contrast)).fill(swatch));
    egui::Popup::menu(&response).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(230.0);
        for (channel, key) in c.iter_mut().zip(["ui.red", "ui.green", "ui.blue"]) {
            ui.add(egui::Slider::new(channel, 0.0..=1.0).text(crate::i18n::text(key)));
        }
    });
}

/// A schematic of the current page with the marks laid out (no rendering needed).
fn preview(ui: &mut egui::Ui, t: &Tokens, size: (f64, f64), draw: impl FnOnce(&egui::Painter, Rect, f32)) {
    let (w, h) = (size.0.max(1.0), size.1.max(1.0));
    let s = (190.0 / w.max(h)) as f32;
    let (r, _) = ui.allocate_exact_size(vec2(210.0, 210.0), egui::Sense::hover());
    let page = Rect::from_center_size(r.center(), vec2(w as f32 * s, h as f32 * s));
    let painter = ui.painter_at(r);
    painter.rect_filled(page, CornerRadius::ZERO, Color32::WHITE);
    painter.rect_stroke(page, CornerRadius::ZERO, Stroke::new(1.0, t.border), egui::StrokeKind::Outside);
    draw(&painter, page, s);
}

fn rgb32(c: [f64; 3]) -> Color32 {
    let b = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

/// The dialog for `kind`; returns (apply, cancel).
pub(crate) fn body(ui: &mut egui::Ui, app: &mut PrintCraftApp, t: &Tokens, kind: MarkKind) -> (bool, bool) {
    let Some((i, id)) = app.active_ids() else { return (false, true) };
    let current = app.views[i].current;
    let Some(doc) = app.session.get(id) else { return (false, true) };
    let count = doc.info.pages.len();
    let page = doc.info.pages.get(current).map(|p| (p.width as f64, p.height as f64)).unwrap_or((612.0, 792.0));
    let d = &mut app.marks_draft;
    let verb = if d.replace { crate::i18n::text("ui.update") } else { crate::i18n::text("ui.add") };
    let title = match kind {
        MarkKind::HeaderFooter => crate::msg!(value_header_and_footer, verb = verb),
        MarkKind::Watermark => crate::msg!(value_watermark, verb = verb),
        MarkKind::Background => crate::msg!(value_background, verb = verb),
    };
    ui.label(egui::RichText::new(title).font(theme::semibold(18.0)));
    ui.add_space(8.0);
    match kind {
        MarkKind::HeaderFooter => {
            ui.horizontal(|ui| {
                ui.label(crate::i18n::text("ui.font_helvetica"));
                ui.label(crate::i18n::text("ui.size"));
                ui.add(egui::DragValue::new(&mut d.hf.font_size).range(4.0..=72.0).speed(0.5));
                ui.checkbox(&mut d.hf.underline, crate::i18n::text("ui.underline"));
                color_button(ui, &mut d.hf.color);
                ui.add_space(16.0);
                ui.label(crate::i18n::text("ui.margins_in"));
                for (label, k) in [
                    (crate::i18n::text("ui.top"), 0),
                    (crate::i18n::text("ui.bottom"), 1),
                    (crate::i18n::text("ui.left"), 2),
                    (crate::i18n::text("ui.right"), 3),
                ] {
                    ui.label(crate::i18n::current().tr(label));
                    let mut v = d.hf.margins[k] / 72.0;
                    if ui.add(egui::DragValue::new(&mut v).range(0.0..=10.0).speed(0.05).max_decimals(2)).changed() {
                        d.hf.margins[k] = v * 72.0;
                    }
                }
            });
            ui.add_space(8.0);
            egui::Grid::new("hf-boxes").num_columns(3).spacing([10.0, 4.0]).show(ui, |ui| {
                for row in 0..2 {
                    for col in 0..3 {
                        ui.label(egui::RichText::new(BOX_NAMES[row * 3 + col]).small());
                    }
                    ui.end_row();
                    for col in 0..3 {
                        let k = row * 3 + col;
                        let r = ui.add_sized([220.0, 40.0], egui::TextEdit::multiline(&mut d.hf.text[k]).desired_rows(2).id_salt(("hf-box", k)));
                        if r.gained_focus() || r.has_focus() {
                            d.focused_box = k;
                        }
                    }
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("hf-page-format").selected_text(PAGE_FORMATS[d.page_format].trim_matches(['<', '>'])).show_ui(
                    ui,
                    |ui| {
                        for (k, f) in PAGE_FORMATS.iter().enumerate() {
                            ui.selectable_value(&mut d.page_format, k, f.trim_matches(['<', '>']));
                        }
                    },
                );
                if ui.button(crate::i18n::text("ui.insert_page_number")).clicked() {
                    d.hf.text[d.focused_box].push_str(PAGE_FORMATS[d.page_format]);
                }
                ui.add_space(12.0);
                egui::ComboBox::from_id_salt("hf-date-format").selected_text(DATE_FORMATS[d.date_format].trim_matches(['<', '>'])).show_ui(
                    ui,
                    |ui| {
                        for (k, f) in DATE_FORMATS.iter().enumerate() {
                            ui.selectable_value(&mut d.date_format, k, f.trim_matches(['<', '>']));
                        }
                    },
                );
                if ui.button(crate::i18n::text("ui.insert_date")).clicked() {
                    d.hf.text[d.focused_box].push_str(DATE_FORMATS[d.date_format]);
                }
                ui.add_space(12.0);
                ui.label(crate::i18n::text("ui.start_page_number"));
                ui.add(egui::DragValue::new(&mut d.hf.start_number).range(1..=999_999));
            });
        }
        MarkKind::Watermark => {
            ui.horizontal(|ui| {
                ui.label(crate::i18n::text("ui.source"));
                ui.radio_value(&mut d.use_file, false, crate::i18n::text("ui.text"));
                ui.radio_value(&mut d.use_file, true, crate::i18n::text("ui.file"));
            });
            if d.use_file {
                file_source(ui, d, MarkKind::Watermark);
            }
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.add_enabled_ui(!d.use_file, |ui| {
                        ui.label(crate::i18n::text("ui.text"));
                        ui.add(
                            egui::TextEdit::multiline(&mut d.wm.text)
                                .desired_rows(2)
                                .desired_width(320.0)
                                .hint_text("CONFIDENTIAL")
                                .id_salt("wm-text"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(!d.use_file, |ui| ui.checkbox(&mut d.fit, crate::i18n::text("ui.fit_to_page")));
                        ui.add_enabled_ui(!d.fit, |ui| {
                            ui.label(crate::i18n::text("ui.size"));
                            if d.wm.font_size == 0.0 {
                                d.wm.font_size = 72.0;
                            }
                            ui.add(egui::DragValue::new(&mut d.wm.font_size).range(6.0..=300.0));
                        });
                        color_button(ui, &mut d.wm.color);
                    });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::text("ui.rotation"));
                        for r in [-45.0, 0.0, 45.0] {
                            ui.radio_value(&mut d.wm.rotation, r, if r == 0.0 { crate::i18n::text("ui.none").to_string() } else { format!("{r}°") });
                        }
                        ui.add(egui::DragValue::new(&mut d.wm.rotation).range(-180.0..=180.0).suffix("°"));
                    });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::text("ui.opacity"));
                        let mut pct = d.wm.opacity * 100.0;
                        if ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix("%")).changed() {
                            d.wm.opacity = pct / 100.0;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::text("ui.location"));
                        ui.radio_value(&mut d.wm.behind, true, crate::i18n::text("ui.appear_behind_page"));
                        ui.radio_value(&mut d.wm.behind, false, crate::i18n::text("ui.appear_on_top_of_page"));
                    });
                });
            });
        }
        MarkKind::Background => {
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.use_file, false, crate::i18n::text("ui.from_colour"));
                ui.add_enabled_ui(!d.use_file, |ui| color_button(ui, &mut d.bg.color));
                ui.add_space(12.0);
                ui.radio_value(&mut d.use_file, true, crate::i18n::text("ui.file"));
            });
            if d.use_file {
                file_source(ui, d, MarkKind::Background);
            }
            ui.horizontal(|ui| {
                ui.label(crate::i18n::text("ui.opacity"));
                let mut pct = d.bg.opacity * 100.0;
                if ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix("%")).changed() {
                    d.bg.opacity = pct / 100.0;
                }
            });
        }
    }
    ui.add_space(8.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.page_range_options")).font(theme::semibold(12.5)));
    d.range.ui(ui, count);
    ui.add_space(8.0);
    // Preview of the current page.
    let hf = d.hf.clone();
    let (wm, bg, fit) = (d.wm.clone(), d.bg.clone(), d.fit);
    let file_name = d.use_file.then(|| d.file.as_ref().map(|f| f.0.clone())).flatten();
    ui.horizontal(|ui| {
        preview(ui, t, page, |p, r, s| match kind {
            MarkKind::HeaderFooter => {
                for (k, text) in hf.text.iter().enumerate() {
                    if text.trim().is_empty() {
                        continue;
                    }
                    let shown = text.replace("<<", "").replace(">>", "");
                    let y = if k < 3 { r.top() + hf.margins[0] as f32 * s } else { r.bottom() - hf.margins[1] as f32 * s };
                    let (x, align) = match k % 3 {
                        0 => (r.left() + hf.margins[2] as f32 * s, egui::Align2::LEFT_CENTER),
                        1 => (r.center().x, egui::Align2::CENTER_CENTER),
                        _ => (r.right() - hf.margins[3] as f32 * s, egui::Align2::RIGHT_CENTER),
                    };
                    p.text(pos2(x, y), align, shown, egui::FontId::proportional((hf.font_size as f32 * s).max(5.0)), rgb32(hf.color));
                }
            }
            MarkKind::Watermark if file_name.is_some() => {
                let side = r.width().min(r.height()) * wm.scale as f32;
                p.rect_stroke(
                    Rect::from_center_size(r.center(), vec2(side, side)),
                    CornerRadius::ZERO,
                    egui::Stroke::new(1.0, Color32::GRAY),
                    egui::StrokeKind::Inside,
                );
                p.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    file_name.clone().unwrap_or_default(),
                    egui::FontId::proportional(10.0),
                    Color32::GRAY,
                );
            }
            MarkKind::Watermark => {
                let text = if wm.text.trim().is_empty() { "CONFIDENTIAL" } else { wm.text.trim() };
                let size = if fit {
                    (r.width().hypot(r.height()) * 0.5 / (text.len().max(1) as f32 * 0.55)).clamp(6.0, 80.0)
                } else {
                    (wm.font_size as f32 * s).max(5.0)
                };
                let galley = p.layout_no_wrap(text.to_string(), egui::FontId::proportional(size), rgb32(wm.color).gamma_multiply(wm.opacity as f32));
                let angle = -(wm.rotation as f32).to_radians();
                let half = galley.size() / 2.0;
                let (sn, cs) = angle.sin_cos();
                let offset = vec2(half.x * cs - half.y * sn, half.x * sn + half.y * cs);
                p.add(egui::epaint::TextShape::new(r.center() - offset, galley, Color32::BLACK).with_angle(angle));
            }
            MarkKind::Background if file_name.is_some() => {
                p.rect_stroke(r.shrink(6.0), CornerRadius::ZERO, egui::Stroke::new(1.0, Color32::GRAY), egui::StrokeKind::Inside);
                p.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    file_name.clone().unwrap_or_default(),
                    egui::FontId::proportional(10.0),
                    Color32::GRAY,
                );
            }
            MarkKind::Background => {
                p.rect_filled(r, CornerRadius::ZERO, rgb32(bg.color).gamma_multiply(bg.opacity as f32));
            }
        });
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(crate::msg!(preview_page_value_of_value, current + 1, count = count)).small().color(t.text_faint));
            let n = d.range.pages(count).len();
            ui.label(egui::RichText::new(crate::msg!(applies_to_value_page_value, crate::i18n::plural_suffix(n), n = n)).small().color(t.text_faint));
            if d.replace {
                ui.label(egui::RichText::new(crate::i18n::text("ui.replaces_the_existing_one_on_those_pages")).small().color(t.text_faint));
            }
        });
    });
    ui.add_space(10.0);
    let (mut apply, mut cancel) = (false, false);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let ready = match kind {
            MarkKind::HeaderFooter => d.hf.text.iter().any(|x| !x.trim().is_empty()),
            MarkKind::Watermark => {
                if d.use_file {
                    d.file.is_some()
                } else {
                    !d.wm.text.trim().is_empty()
                }
            }
            MarkKind::Background => !d.use_file || d.file.is_some(),
        } && !d.range.pages(count).is_empty();
        if ui.add_enabled_ui(ready, |ui| widgets::pill_button(ui, crate::i18n::text("ui.ok"), true)).inner.clicked() {
            apply = true;
        }
        if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
            cancel = true;
        }
    });
    (apply, cancel)
}

fn mark_file(d: &MarksDraft) -> Option<printcraft_engine::MarkFile> {
    let (name, bytes) = d.file.clone().filter(|_| d.use_file)?;
    Some(printcraft_engine::MarkFile { name, bytes, page: d.file_page.max(1) - 1 })
}

/// Source ▸ File: Browse…, the page of a PDF, and the size relative to the page.
fn file_source(ui: &mut egui::Ui, d: &mut MarksDraft, kind: MarkKind) {
    ui.horizontal(|ui| {
        #[cfg(not(target_arch = "wasm32"))]
        if ui.button(crate::i18n::text("ui.browse")).clicked()
            && let Some(p) = rfd::FileDialog::new()
                .add_filter(crate::i18n::text("ui.pdf_or_image"), &["pdf", "png", "jpg", "jpeg", "tif", "tiff", "gif", "bmp", "jp2", "j2k", "jpx"])
                .pick_file()
        {
            match std::fs::read(&p) {
                Ok(b) => d.file = Some((p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), std::sync::Arc::new(b))),
                Err(_) => d.file = None,
            }
        }
        ui.label(d.file.as_ref().map(|f| f.0.clone()).unwrap_or_else(|| crate::i18n::text("ui.no_file_chosen").into()));
        if d.file.as_ref().is_some_and(|f| f.1.starts_with(b"%PDF")) {
            ui.label(crate::i18n::text("ui.page_number"));
            ui.add(egui::DragValue::new(&mut d.file_page).range(1..=9999));
        }
    });
    ui.horizontal(|ui| {
        ui.label(crate::i18n::text("ui.scale_relative_to_target_page"));
        let scale = if kind == MarkKind::Background { &mut d.bg.scale } else { &mut d.wm.scale };
        let mut pct = *scale * 100.0;
        if ui.add(egui::Slider::new(&mut pct, 5.0..=100.0).suffix("%")).changed() {
            *scale = pct / 100.0;
        }
    });
}

/// The edit the dialog's OK makes.
pub fn edit(d: &MarksDraft, kind: MarkKind, count: usize) -> Edit {
    let pages = d.range.pages(count);
    match kind {
        MarkKind::HeaderFooter => Edit::AddHeaderFooter { pages, settings: d.hf.clone(), replace: d.replace },
        MarkKind::Watermark => {
            let mut wm = d.wm.clone();
            if d.fit {
                wm.font_size = 0.0;
            }
            Edit::AddWatermark { pages, settings: wm, replace: d.replace, file: mark_file(d) }
        }
        MarkKind::Background => Edit::AddBackground { pages, settings: d.bg.clone(), replace: d.replace, file: mark_file(d) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges_with_subsets() {
        let r = PageRange::default();
        assert_eq!(r.pages(4), [0, 1, 2, 3]);
        let r = PageRange { all: false, from: 2, to: 9, subset: Subset::Even };
        assert_eq!(r.pages(6), [1, 3, 5]);
        let r = PageRange { all: true, subset: Subset::Odd, ..PageRange::default() };
        assert_eq!(r.pages(5), [0, 2, 4]);
    }
}

#[cfg(test)]
mod color_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn color_popup_uses_russian_resources_without_changing_rgb() {
        let _locale = crate::i18n::scope(crate::i18n::Language::Ru);
        let original = [0.25, 0.5, 0.75];
        let mut h = egui_kittest::Harness::new_ui_state(color_button, original);
        h.get_by_label("Цвет").click();
        h.run_steps(2);
        for label in ["Красный", "Зелёный", "Синий"] {
            assert!(h.query_all_by_label(label).count() > 0, "{label}");
        }
        assert_eq!(*h.state(), original);
    }
}
