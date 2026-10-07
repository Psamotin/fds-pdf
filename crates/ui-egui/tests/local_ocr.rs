//! Corporate backend in the existing Scan & OCR dialog; no installed OCR required.
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use printcraft_ui_egui::PrintCraftApp;
use std::time::{Duration, Instant};

#[test]
fn missing_portable_runtime_has_a_russian_error_and_preserves_the_document() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("FDS-PDF.exe");
    let session = printcraft_engine::Session::new();
    let source = session.create_from_text("document", "Original content").unwrap().to_vec();
    let original = source.clone();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = PrintCraftApp::default();
        app.open_bytes("договор.pdf", None, source).unwrap();
        app.enable_local_ocr(&executable).unwrap();
        app
    });
    h.run_steps(4);
    assert!(h.state_mut().execute("ocr.recognize"));
    let deadline = Instant::now() + Duration::from_secs(3);
    let error = "Встроенный модуль распознавания отсутствует или повреждён. Повторно распакуйте полный архив приложения.";
    while h.query_by_label(error).is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
        h.run_steps(2);
    }
    h.get_by_label(error);
    h.get_by_label("Русский + Английский");
    h.get_by_label("Распознать");
    if let Some(path) = std::env::var_os("FDS_OCR_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    let app = h.state();
    let document = app.session.get(app.active_ids().unwrap().1).unwrap();
    assert_eq!(document.bytes.as_slice(), original);
    assert!(!app.local_ocr.as_ref().unwrap().is_running());
}
