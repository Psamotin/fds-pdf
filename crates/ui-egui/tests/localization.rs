//! R1: actual Russian UI, stable engine command ids, and English fallback.
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use printcraft_ui_egui::{
    PrintCraftApp,
    i18n::{Language, PRODUCT_NAME, PRODUCT_VERSION},
};

fn harness(options: &[(&str, &str)]) -> Harness<'static, PrintCraftApp> {
    let options: Vec<(String, String)> = options.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect();
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_eframe(move |_| {
        let mut app = PrintCraftApp::new();
        for (key, value) in &options {
            app.set_option(key, value).unwrap();
        }
        app
    });
    h.run_steps(4);
    h
}

#[test]
fn russian_is_default_and_home_chrome_and_tools_are_localized() {
    let mut h = harness(&[]);
    assert_eq!(h.state().language, Language::Ru);
    assert_eq!(PRODUCT_NAME, "ФДС ПДФ");
    assert_eq!(PRODUCT_VERSION, "0.2.0-fds.1");
    for label in ["Открыть файл", "Недавние документы", "КОНФИДЕНЦИАЛЬНОСТЬ", "Чтение", "Редактирование", "Конвертация", "Подпись"]
    {
        h.get_by_label(label);
    }
    for label in ["Все инструменты", "Упорядочить страницы", "Комментарии", "Защитить PDF", "Распознать текст"]
    {
        assert!(h.query_all_by_label(label).count() > 0, "{label}");
    }
    h.get_by_label("Меню").click();
    h.run_steps(2);
    for label in ["Файл", "Редактирование", "Страницы", "Вид", "Справка"] {
        assert!(h.query_all_by_label_contains(label).count() > 0, "{label}");
    }
}

#[test]
fn russian_palette_finds_tools_and_keeps_english_ids() {
    let h = harness(&[("palette", "распознать")]);
    assert!(h.query_all_by_label_contains("Распознать текст").count() > 0);
    let h = harness(&[("palette", "file.open")]);
    h.get_by_label("Открыть…");
    assert!(printcraft_engine::commands::command("file.open").is_some());
}

#[test]
fn english_fallback_and_locale_preferences_survive_restart() {
    let h = harness(&[("language", "en")]);
    h.get_by_label("Open file");
    let mut app = PrintCraftApp::new();
    app.restore(&h.state().persist());
    assert_eq!(app.language, Language::En);
    app.set_option("language", "ru").unwrap();
    assert_eq!(Language::Ru.command_label("Undo Rotate page"), "Отменить Повернуть страницу");
    assert_eq!(Language::Ru.tr("custom-document.pdf"), "custom-document.pdf");
}

#[test]
fn russian_preferences_about_and_missing_document_errors() {
    let mut h = harness(&[("dialog", "preferences")]);
    h.get_by_label("Язык интерфейса");
    h.get_by_role(egui::accesskit::Role::ComboBox).click();
    h.run_steps(2);
    h.get_by_label("Русский");
    h.get_by_label("Имя автора новых комментариев");
    let mut h = harness(&[]);
    h.state_mut().execute("file.save");
    h.run_steps(2);
    h.get_by_label("Сначала откройте документ");
    assert!(h.state().views.is_empty());
}

#[test]
fn russian_ui_never_translates_pdf_named_actions_or_history_identifiers() {
    use printcraft_ui_egui::prepare::{ActionDraft, Arrange};
    let _locale = printcraft_ui_egui::i18n::scope(Language::Ru);
    let draft = ActionDraft { kind: 3, ..Default::default() };
    assert_eq!(draft.action().unwrap(), printcraft_engine::FieldAction::Named("Print".into()));
    assert_eq!(Arrange::AlignTop.label(), "Align fields");
}
