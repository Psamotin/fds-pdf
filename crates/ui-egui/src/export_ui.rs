//! Export a PDF ▸ Image (PNG, JPEG, TIFF) and Text (Acrobat's Export a PDF tool, first formats).
//!
//! On the desktop the export runs on a worker thread and reports progress in the notice bar;
//! on the web it runs in place and downloads the files.

use std::sync::{Arc, Mutex};

use egui::{Align, Layout};
use printcraft_engine::export::{ExportSource, Exporter, ImageFormat};

use crate::marks_ui::PageRange;
use crate::theme::{self, Tokens};
use crate::{PrintCraftApp, widgets};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Image,
    Text,
    /// Export all images: the images pages use, as files.
    AllImages,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportDraft {
    pub dpi: f64,
    pub range: PageRange,
    pub format: ImageFormat,
    /// Export all images ▸ skip images with fewer pixels than this on their shorter side.
    pub min_side: u32,
}

impl Default for ExportDraft {
    fn default() -> Self {
        Self { dpi: 150.0, range: PageRange::default(), format: ImageFormat::Png, min_side: 0 }
    }
}

/// Progress of a background export: (done, total, final message once finished).
pub type ExportStatus = Arc<Mutex<Option<(usize, usize, Option<String>)>>>;

pub(crate) fn body(ui: &mut egui::Ui, app: &mut PrintCraftApp, t: &Tokens, kind: ExportKind) -> (bool, bool) {
    let count = app.active_ids().and_then(|(_, id)| app.session.get(id)).map(|d| d.info.pages.len()).unwrap_or(0);
    let d = &mut app.export_draft;
    ui.label(
        egui::RichText::new(match kind {
            ExportKind::Image => crate::i18n::text("ui.export_to_image"),
            ExportKind::Text => crate::i18n::text("ui.export_to_text"),
            ExportKind::AllImages => crate::i18n::text("ui.export_all_images"),
        })
        .font(theme::semibold(18.0)),
    );
    ui.add_space(8.0);
    if kind == ExportKind::Image {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::text("ui.resolution"));
            egui::ComboBox::from_id_salt("export-dpi").selected_text(crate::msg!(value_pixels_inch, d.dpi)).show_ui(ui, |ui| {
                for dpi in [72.0, 96.0, 150.0, 300.0, 600.0] {
                    ui.selectable_value(&mut d.dpi, dpi, crate::msg!(value_pixels_inch_b5a63a, dpi = dpi));
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::text("ui.format"));
            let mut quality = match d.format {
                ImageFormat::Jpeg { quality } => quality,
                _ => 85,
            };
            egui::ComboBox::from_id_salt("export-format").selected_text(d.format.label()).show_ui(ui, |ui| {
                for f in [ImageFormat::Png, ImageFormat::Jpeg { quality }, ImageFormat::Tiff] {
                    let on = std::mem::discriminant(&d.format) == std::mem::discriminant(&f);
                    if ui.selectable_label(on, f.label()).clicked() {
                        d.format = f;
                    }
                }
            });
            if let ImageFormat::Jpeg { .. } = d.format {
                ui.label(crate::i18n::text("ui.quality"));
                if ui.add(egui::Slider::new(&mut quality, 10..=100)).changed() {
                    d.format = ImageFormat::Jpeg { quality };
                }
            }
        });
        ui.label(egui::RichText::new(crate::msg!(one_value_file_per_page_named_after_the_document, d.format.label())).small().color(t.text_faint));
    } else if kind == ExportKind::AllImages {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::text("ui.exclude_images_smaller_than"));
            let label = |n: u32| if n == 0 { crate::i18n::text("ui.no_limit").to_string() } else { crate::msg!(value_pixels, n = n) };
            egui::ComboBox::from_id_salt("export-min").selected_text(label(d.min_side)).show_ui(ui, |ui| {
                for n in [0, 16, 32, 64, 128, 256] {
                    ui.selectable_value(&mut d.min_side, n, label(n));
                }
            });
        });
        ui.label(
            egui::RichText::new(crate::i18n::text(
                "ui.each_image_once_named_after_the_document_and_page_jpeg_images_are_saved_unchanged_others_as_png",
            ))
            .small()
            .color(t.text_faint),
        );
    } else {
        ui.label(
            egui::RichText::new(crate::i18n::text("ui.plain_text_in_reading_order_pages_are_separated_by_form_feeds")).small().color(t.text_faint),
        );
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.pages")).font(theme::semibold(12.5)));
    d.range.ui(ui, count);
    ui.add_space(12.0);
    let (mut apply, mut cancel) = (false, false);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let ok = !d.range.pages(count).is_empty();
        if ui.add_enabled_ui(ok, |ui| widgets::pill_button(ui, crate::i18n::text("ui.export"), true)).inner.clicked() {
            apply = true;
        }
        if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
            cancel = true;
        }
    });
    (apply, cancel)
}

/// Write the files (`name`, bytes) produced for `pages`, reporting progress.
#[allow(clippy::too_many_arguments)]
fn run(
    src: ExportSource,
    kind: ExportKind,
    dpi: f64,
    format: ImageFormat,
    min_side: u32,
    pages: Vec<usize>,
    stem: String,
    mut sink: impl FnMut(&str, Vec<u8>) -> Result<(), String>,
    status: &ExportStatus,
) -> String {
    let total = pages.len();
    let set = |done: usize, msg: Option<String>| {
        if let Ok(mut s) = status.lock() {
            *s = Some((done, total, msg));
        }
    };
    if kind == ExportKind::AllImages {
        set(0, None);
        let out = match printcraft_engine::export::extract_images(&src, &pages, min_side) {
            Ok(o) => o,
            Err(e) => return crate::msg!(export_stopped_value, e = e),
        };
        for (k, img) in out.images.iter().enumerate() {
            if let Err(e) = sink(&printcraft_engine::export::image_file_name(&stem, img, k + 1), img.data.clone()) {
                return crate::msg!(export_stopped_value, e = e);
            }
        }
        let n = out.images.len();
        let mut msg = crate::msg!(exported_value_image_value, crate::i18n::plural_suffix(n), n = n);
        if !out.skipped.is_empty() {
            msg.push_str(&crate::msg!(value_not_exported_value, out.skipped.len(), out.skipped[0].2));
        }
        return msg;
    }
    let mut ex = Exporter::from_source(src);
    match kind {
        // Handled above.
        ExportKind::AllImages => String::new(),
        ExportKind::Image => {
            for (k, p) in pages.iter().enumerate() {
                set(k, None);
                let result = ex.image(*p, dpi, format).and_then(|img| sink(&format!("{stem}_page_{}.{}", p + 1, format.extension()), img));
                if let Err(e) = result {
                    return crate::msg!(export_stopped_value, e = e);
                }
            }
            crate::msg!(exported_value_image_value_bddf51, crate::i18n::plural_suffix(total), total = total)
        }
        ExportKind::Text => {
            set(0, None);
            match ex.text_of(&pages).and_then(|text| sink(&format!("{stem}.txt"), text.into_bytes())) {
                Ok(()) => crate::msg!(exported_the_text_of_value_page_value, crate::i18n::plural_suffix(total), total = total),
                Err(e) => crate::msg!(export_stopped_value, e = e),
            }
        }
    }
}

impl PrintCraftApp {
    /// Start exporting the active document with the dialog's settings.
    pub(crate) fn start_export(&mut self, kind: ExportKind) {
        let _locale = crate::i18n::scope(self.language);
        let Some((_, id)) = self.active_ids() else { return };
        let Some(doc) = self.session.get(id) else { return };
        let src = doc.export_source();
        let stem = doc.name.trim_end_matches(".pdf").trim_end_matches(".PDF").to_string();
        let pages = self.export_draft.range.pages(src.pages);
        let dpi = self.export_draft.dpi;
        let format = self.export_draft.format;
        let min_side = self.export_draft.min_side;
        let status: ExportStatus = Arc::new(Mutex::new(Some((0, pages.len(), None))));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = match &self.export_dir_override {
                Some(d) => Some(std::path::PathBuf::from(d)),
                None => rfd::FileDialog::new().set_title(crate::i18n::text("ui.choose_a_folder_for_the_exported_files")).pick_folder(),
            };
            let Some(dir) = dir else { return };
            let st = status.clone();
            let shown = dir.display().to_string();
            let work = move || {
                let sink = |name: &str, bytes: Vec<u8>| {
                    crate::editing::write_atomically(&dir.join(name).to_string_lossy(), &bytes).map_err(|e| format!("{name}: {e}"))
                };
                let msg = run(src, kind, dpi, format, min_side, pages, stem, sink, &st);
                if let Ok(mut s) = st.lock() {
                    let (done, total) = s.as_ref().map_or((0, 0), |(d, t, _)| (*d, *t));
                    *s = Some((done.max(total), total, Some(crate::msg!(value_to_value, msg = msg, shown = shown))));
                }
            };
            if self.export_dir_override.is_some() {
                work(); // tests and automation: synchronous
            } else {
                std::thread::Builder::new().name("printcraft-export".into()).spawn(crate::i18n::in_locale(work)).ok();
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let msg = run(src, kind, dpi, format, min_side, pages, stem, |name, bytes| crate::editing::download(name, &bytes), &status);
            if let Ok(mut s) = status.lock() {
                *s = Some((0, 0, Some(msg)));
            }
        }
        self.export_status = Some(status);
    }

    /// Show export progress, and the result once it is done.
    pub(crate) fn poll_export(&mut self) {
        let _locale = crate::i18n::scope(self.language);
        let Some(st) = self.export_status.clone() else { return };
        let snapshot = st.lock().ok().and_then(|s| s.clone());
        match snapshot {
            Some((_, _, Some(msg))) => {
                self.export_status = None;
                self.notify(msg);
            }
            Some((done, total, None)) if total > 1 => self.notify(crate::msg!(exporting_value_of_value, done = done, total = total)),
            _ => {}
        }
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}
