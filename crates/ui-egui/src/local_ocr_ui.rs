//! Existing Scan & OCR desktop presentation backed by the packaged local process adapter.
use crate::local_ocr::{self, Cancellation, Failure, FailureKind, Options, Runtime};
use crate::{PrintCraftApp, widgets};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Validation = Arc<Mutex<Option<Result<(), Failure>>>>;
type JobResult = Result<Vec<PathBuf>, Failure>;
#[derive(Default)]
struct Progress {
    done: usize,
    total: usize,
    result: Option<JobResult>,
}
struct Run {
    cancellation: Cancellation,
    progress: Arc<Mutex<Progress>>,
    started: Instant,
}
pub struct DesktopOcr {
    pub runtime: Runtime,
    pub options: Options,
    validation: Validation,
    run: Option<Run>,
    result: Option<JobResult>,
}
impl DesktopOcr {
    fn new(runtime: Runtime) -> Self {
        let validation: Validation = Arc::new(Mutex::new(None));
        let status = validation.clone();
        let probe = runtime.clone();
        let failed_status = validation.clone();
        if let Err(error) = std::thread::Builder::new().name("fds-ocr-validation".into()).spawn(move || {
            let result = printcraft_engine::guard(|| probe.validate()).unwrap_or_else(|e| Err(Failure::new(FailureKind::Runtime, e)));
            if let Ok(mut state) = status.lock() {
                *state = Some(result);
            }
        }) && let Ok(mut status) = failed_status.lock()
        {
            *status = Some(Err(Failure::new(FailureKind::Runtime, error)));
        }
        Self { runtime, options: Options::default(), validation, run: None, result: None }
    }
    pub fn cancel(&self) {
        if let Some(run) = &self.run {
            run.cancellation.0.store(true, Ordering::Relaxed);
        }
    }
    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }
    fn ready(&self) -> Result<(), Failure> {
        self.validation
            .lock()
            .map_err(|_| Failure::new(FailureKind::Runtime, "Runtime validation lock poisoned"))?
            .clone()
            .ok_or_else(|| Failure::new(FailureKind::Runtime, "Runtime validation is pending"))?
    }
}

fn text(key: &str) -> &str {
    crate::i18n::text(key)
}
pub(crate) fn body(ui: &mut egui::Ui, app: &mut PrintCraftApp) -> (bool, bool) {
    ui.heading(text("ui.recognize_text"));
    let Some((_, id)) = app.active_ids() else {
        ui.label(text("ui.open_a_document_first"));
        return (false, false);
    };
    let Some(document) = app.session.get(id) else {
        return (false, false);
    };
    ui.label(text("ui.ocr_local_file"));
    ui.label(&document.name);
    ui.add_space(8.0);
    ui.label(text("ui.ocr_local_language"));
    ui.label(text("ui.ocr_local_languages"));
    let Some(state) = app.local_ocr.as_mut() else {
        return (false, false);
    };
    ui.add_space(8.0);
    ui.checkbox(&mut state.options.rotate, text("ui.ocr_local_rotate"));
    ui.checkbox(&mut state.options.deskew, text("ui.ocr_local_deskew"));
    ui.add_space(8.0);
    ui.label(text("ui.ocr_local_output"));
    let output = document.path.as_ref().and_then(|p| local_ocr::output_path(Path::new(p)).ok());
    ui.label(output.map(|p| p.display().to_string()).unwrap_or_else(|| local_ocr::output_name(&document.name)));
    ui.label(egui::RichText::new(text("ui.ocr_local_privacy")).small());
    let validation = state.validation.lock().ok().and_then(|s| s.clone());
    match &validation {
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(text("ui.ocr_local_checking"));
            });
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        Some(Err(error)) => {
            show_error(ui, error);
        }
        Some(Ok(())) => {}
    }
    ui.add_space(12.0);
    let mut go = false;
    let mut cancel = false;
    ui.horizontal(|ui| {
        go = ui
            .add_enabled_ui(matches!(validation, Some(Ok(()))) && !state.is_running(), |ui| {
                widgets::pill_button(ui, text("ui.ocr_local_recognize"), true)
            })
            .inner
            .clicked();
        cancel = widgets::pill_button(ui, text("ui.cancel"), false).clicked();
    });
    (go, cancel)
}
fn show_error(ui: &mut egui::Ui, error: &Failure) {
    ui.label(error.message());
    if !error.details.is_empty() {
        ui.collapsing(text("ui.ocr_local_details"), |ui| {
            ui.label(&error.details);
        });
    }
}

impl PrintCraftApp {
    /// Enable the corporate desktop backend. Legacy engine OCR is retained for upstream/web users.
    pub fn enable_local_ocr(&mut self, executable: &Path) -> Result<(), Failure> {
        self.local_ocr = Some(DesktopOcr::new(Runtime::for_executable(executable)?));
        Ok(())
    }
    pub(crate) fn start_local_ocr(&mut self) {
        let Some((_, id)) = self.active_ids() else {
            return;
        };
        let Some(document) = self.session.get(id) else {
            return;
        };
        let output = match &document.path {
            Some(path) => match local_ocr::output_path(Path::new(path)) {
                Ok(path) => Some(path),
                Err(error) => {
                    if let Some(state) = self.local_ocr.as_mut() {
                        state.result = Some(Err(error));
                    }
                    return;
                }
            },
            None => match &self.export_dir_override {
                Some(folder) => Some(Path::new(folder).join(local_ocr::output_name(&document.name))),
                None => rfd::FileDialog::new()
                    .set_title(text("ui.ocr_local_choose_output"))
                    .set_file_name(local_ocr::output_name(&document.name))
                    .add_filter("PDF", &["pdf"])
                    .save_file(),
            },
        };
        let Some(output) = output else {
            return;
        };
        let jobs = vec![(document.bytes.clone(), output)];
        self.spawn_local_ocr(jobs);
    }
    pub(crate) fn start_local_ocr_files(&mut self, files: Vec<(String, Vec<u8>)>) {
        if files.is_empty() {
            return;
        }
        let folder = match &self.export_dir_override {
            Some(folder) => Some(PathBuf::from(folder)),
            None => rfd::FileDialog::new().set_title(text("ui.ocr_local_choose_folder")).pick_folder(),
        };
        let Some(folder) = folder else {
            return;
        };
        let mut outputs = std::collections::HashSet::new();
        let mut jobs = Vec::new();
        for (name, bytes) in files {
            let source = folder.join(Path::new(&name).file_name().unwrap_or_else(|| std::ffi::OsStr::new("document.pdf")));
            match local_ocr::output_path(&source) {
                Ok(mut output) => {
                    // Duplicate filenames in one batch also receive distinct outputs.
                    let mut number = 2;
                    while outputs.contains(&output) || output.exists() {
                        output = folder.join(format!("{}_{}.pdf", local_ocr::output_name(&name).trim_end_matches(".pdf"), number));
                        number += 1;
                    }
                    outputs.insert(output.clone());
                    jobs.push((Arc::new(bytes), output));
                }
                Err(error) => {
                    if let Some(state) = self.local_ocr.as_mut() {
                        state.result = Some(Err(error));
                    }
                    return;
                }
            }
        }
        self.spawn_local_ocr(jobs);
    }
    fn spawn_local_ocr(&mut self, jobs: Vec<(Arc<Vec<u8>>, PathBuf)>) {
        let Some(state) = self.local_ocr.as_mut() else {
            return;
        };
        if state.is_running() {
            self.notify(text("ui.text_recognition_is_already_running"));
            return;
        }
        if let Err(error) = state.ready() {
            state.result = Some(Err(error));
            return;
        }
        state.result = None;
        let cancellation = Cancellation::default();
        let cancelled = cancellation.0.clone();
        let progress = Arc::new(Mutex::new(Progress { total: jobs.len(), ..Default::default() }));
        let shared = progress.clone();
        let runtime = state.runtime.clone();
        let options = state.options;
        let worker = crate::i18n::in_locale(move || {
            let outcome = printcraft_engine::guard(|| {
                let mut results = Vec::new();
                for (bytes, output) in jobs {
                    results.push(local_ocr::recognize(&runtime, &bytes, &output, options, &cancelled)?);
                    if let Ok(mut progress) = shared.lock() {
                        progress.done += 1;
                    }
                }
                Ok(results)
            })
            .unwrap_or_else(|error| Err(Failure::new(FailureKind::Recognition, error)));
            if let Ok(mut progress) = shared.lock() {
                progress.result = Some(outcome);
            }
        });
        match std::thread::Builder::new().name("fds-local-ocr".into()).spawn(worker) {
            Ok(_) => state.run = Some(Run { cancellation, progress, started: Instant::now() }),
            Err(error) => state.result = Some(Err(Failure::new(FailureKind::Launch, error))),
        }
    }
    pub(crate) fn poll_local_ocr(&mut self) {
        let Some(state) = self.local_ocr.as_mut() else {
            return;
        };
        if let Some(run) = &state.run {
            let result = match run.progress.lock() {
                Ok(mut progress) => progress.result.take(),
                Err(_) => Some(Err(Failure::new(FailureKind::Recognition, "OCR progress lock poisoned"))),
            };
            if let Some(result) = result {
                state.run = None;
                state.result = Some(result);
            }
        }
        if let Some(run) = &state.run {
            let mut cancel = false;
            if let Some(ctx) = &self.ctx {
                egui::Modal::new(egui::Id::new("fds-local-ocr-progress")).show(ctx, |ui| {
                    ui.heading(text("ui.ocr_local_running"));
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(text("ui.ocr_local_progress_notice"));
                    });
                    if let Ok(progress) = run.progress.lock() {
                        ui.label(format!("{} / {}", (progress.done + 1).min(progress.total), progress.total));
                    }
                    ui.label(format!("{}: {}", text("ui.ocr_local_elapsed"), run.started.elapsed().as_secs()));
                    cancel = widgets::pill_button(ui, text("ui.cancel"), false).clicked();
                });
            }
            if cancel {
                state.cancel();
            }
            if let Some(ctx) = &self.ctx {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        let Some(result) = state.result.as_ref() else {
            return;
        };
        let Some(ctx) = self.ctx.clone() else {
            return;
        };
        let mut close = false;
        let mut open = None;
        egui::Modal::new(egui::Id::new("fds-local-ocr-result")).show(&ctx, |ui| {
            match result {
                Ok(paths) => {
                    ui.heading(text("ui.ocr_local_complete"));
                    for path in paths {
                        ui.label(path.display().to_string());
                        if ui.push_id(path, |ui| widgets::pill_button(ui, text("ui.ocr_local_open_result"), true)).inner.clicked() {
                            open = Some(path.clone());
                        }
                    }
                }
                Err(error) => show_error(ui, error),
            }
            close = widgets::pill_button(ui, text("ui.close"), false).clicked();
        });
        if close || open.is_some() {
            state.result = None;
        }
        if let Some(path) = open {
            self.open_path(&path.to_string_lossy());
        }
    }
}
