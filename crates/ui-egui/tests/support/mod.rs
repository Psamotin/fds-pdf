//! Upstream behavior tests explicitly exercise the English fallback.
//! Corporate Russian rendering is covered by localization.rs.
pub fn english_app() -> printcraft_ui_egui::PrintCraftApp {
    let mut app = printcraft_ui_egui::PrintCraftApp::new();
    app.set_option("language", "en").expect("English fallback is supported");
    app
}
