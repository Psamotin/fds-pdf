//! Corporate UI hides upstream marketing; engine command compatibility is retained.
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use printcraft_ui_egui::{Dialog, PrintCraftApp};

fn harness(dialog: Option<Dialog>) -> Harness<'static, PrintCraftApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_| {
        let mut app = PrintCraftApp::new();
        app.dialog = dialog;
        app
    });
    h.run_steps(4);
    h
}

#[test]
fn home_and_about_do_not_show_upstream_marketing() {
    for dialog in [None, Some(Dialog::About)] {
        let h = harness(dialog);
        for marketing in ["Discord", "ArtCraft", "Join our Discord", "PrintCraft web page", "ArtCraft website"] {
            assert!(h.query_by_label_contains(marketing).is_none(), "{marketing}");
        }
        assert!(h.query_all_by_label_contains("ФДС ПДФ").count() > 0);
        if dialog.is_some() {
            h.get_by_label_contains("0.2.0-fds.1");
            h.get_by_label_contains("LICENSE, NOTICE");
        }
    }
}

#[test]
fn help_menu_hides_marketing_and_engine_ids_remain_registered() {
    let mut h = harness(None);
    h.get_by_label("Меню").click();
    h.run_steps(2);
    h.get_by_label("Справка ⏵").click();
    h.run_steps(2);
    assert!(h.query_by_label_contains("ArtCraft").is_none());
    assert!(h.query_by_label_contains("Discord").is_none());
    for link in printcraft_engine::links::LINKS {
        assert!(printcraft_engine::commands::command(link.command).is_some());
    }
}
