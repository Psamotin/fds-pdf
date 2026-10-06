//! Window chrome: tab strip (with the integrated macOS title bar), mode bar, right rail.

use egui::{Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, vec2};

use crate::canvas::{Fit, PageLayout};
use crate::theme::{self, ThemeKind, Tokens};
use crate::{Dialog, Mode, PrintCraftApp, PropsTab, RightPanel, icons, widgets};

pub fn tab_strip(app: &mut PrintCraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let left = if cfg!(target_os = "macos") && app.integrated_titlebar { 80 } else { 8 };
    egui::Panel::top("tab_strip")
        .exact_size(38.0)
        .frame(egui::Frame::NONE.fill(t.titlebar).inner_margin(egui::Margin { left, right: 10, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            let full = ui.max_rect();
            let drag = ui.interact(full, ui.id().with("titledrag"), Sense::click_and_drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if icons::button(ui, "house", 28.0, app.active.is_none(), crate::i18n::text("ui.home")).clicked() {
                    app.active = None;
                }
                let mut close = None;
                for i in 0..app.views.len() {
                    let Some(doc) = app.session.get(app.views[i].id) else { continue };
                    let (name, dirty) = (doc.display_name(), doc.dirty);
                    if tab(ui, &t, &name, dirty, app.active == Some(i), &mut close, i).clicked() {
                        app.active = Some(i);
                    }
                }
                if let Some(i) = close {
                    app.request_close_tab(i);
                }
                ui.add_space(4.0);
                if widgets::ghost_button(ui, "plus", crate::i18n::text("ui.open")).on_hover_text(crate::i18n::text("ui.open_a_pdf_o")).clicked() {
                    app.open_dialog();
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (icon, next, tip) = match app.theme {
                        ThemeKind::Light => ("moon", ThemeKind::Dark, crate::i18n::text("ui.dark_gray_theme")),
                        ThemeKind::Dark => ("sun", ThemeKind::Light, crate::i18n::text("ui.light_theme")),
                    };
                    if icons::button(ui, icon, 28.0, false, tip).clicked() {
                        let ctx = ui.ctx().clone();
                        app.set_theme(&ctx, next);
                    }
                    if icons::button(ui, "circle-help", 28.0, false, crate::i18n::text("ui.keyboard_shortcuts")).clicked() {
                        app.dialog = Some(Dialog::Shortcuts);
                    }
                });
            });
        });
}

fn tab(ui: &mut egui::Ui, t: &Tokens, name: &str, dirty: bool, active: bool, close: &mut Option<usize>, index: usize) -> egui::Response {
    let font = theme::regular(13.0);
    let label: String = if name.chars().count() > 28 { format!("{}…", name.chars().take(27).collect::<String>()) } else { name.to_string() };
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), t.text).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(text_w + 64.0, 30.0), Sense::click());
    let a11y = if dirty { crate::msg!(value_edited, name = name) } else { name.to_string() };
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, &a11y));
    let bg = if active {
        t.chrome
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 }, bg);
    icons::paint(
        ui,
        Rect::from_min_size(rect.min + vec2(6.0, 7.0), vec2(16.0, 16.0)),
        "file-text",
        15.0,
        if active { t.accent } else { t.text_muted },
    );
    ui.painter().text(rect.min + vec2(28.0, rect.height() / 2.0), Align2::LEFT_CENTER, label, font, if active { t.text } else { t.text_muted });
    let x_rect = Rect::from_center_size(rect.right_center() - vec2(16.0, 0.0), vec2(20.0, 20.0));
    let x = ui.interact(x_rect, ui.id().with(("tabclose", index)), Sense::click());
    if x.hovered() {
        ui.painter().rect_filled(x_rect, CornerRadius::same(4), t.pressed);
    }
    // Unsaved changes: a dot where the close button sits, until the tab is hovered.
    if dirty && !resp.hovered() && !x.hovered() {
        ui.painter().circle_filled(x_rect.center(), 4.0, if active { t.text } else { t.text_muted });
    } else if active || resp.hovered() || x.hovered() {
        icons::paint(ui, x_rect, "x", 13.0, t.text_muted);
    }
    if x.clicked() {
        *close = Some(index);
    }
    resp.on_hover_text(if dirty { crate::msg!(value_unsaved_changes, name = name) } else { name.to_string() })
}

pub fn mode_bar(app: &mut PrintCraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("mode_bar")
        .exact_size(48.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(10, 0)).stroke(Stroke::new(1.0, t.divider)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                main_menu(app, ui);
                ui.add_space(6.0);
                ui.painter().vline(ui.cursor().left(), ui.max_rect().y_range().shrink(12.0), Stroke::new(1.0, t.divider));
                ui.add_space(10.0);
                for (mode, label) in [
                    (Mode::AllTools, crate::i18n::text("ui.all_tools")),
                    (Mode::Read, crate::i18n::text("ui.read")),
                    (Mode::Edit, crate::i18n::text("ui.edit")),
                    (Mode::Convert, crate::i18n::text("ui.convert")),
                    (Mode::Sign, crate::i18n::text("ui.e_sign")),
                ] {
                    if widgets::mode_tab(ui, label, app.mode == mode).clicked() {
                        app.mode = mode;
                        app.left_open = true;
                        app.left = match mode {
                            Mode::Edit => crate::LeftPanel::Tool("edit"),
                            Mode::Convert => crate::LeftPanel::Tool("export"),
                            Mode::Sign => crate::LeftPanel::Tool("fill_sign"),
                            _ => crate::LeftPanel::AllTools,
                        };
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let has_doc = app.active.is_some();
                    ui.add_enabled_ui(has_doc, |ui| {
                        if icons::button(ui, "printer", 32.0, false, crate::i18n::text("ui.print_p")).clicked() {
                            app.run_command("print.dialog");
                        }
                        if icons::button(ui, "save", 32.0, false, crate::i18n::text("ui.save_m4")).clicked() {
                            app.run_command("file.save");
                        }
                        if icons::button(ui, "info", 32.0, false, crate::i18n::text("ui.document_properties_d")).clicked() {
                            app.dialog = Some(Dialog::Properties(PropsTab::Description));
                        }
                    });
                    ui.add_space(8.0);
                    if widgets::search_box(ui, crate::i18n::text("ui.find_tools_and_commands"), 260.0).clicked() {
                        app.palette_open = true;
                    }
                });
            });
        });
}

fn main_menu(app: &mut PrintCraftApp, ui: &mut egui::Ui) {
    let language = app.language;
    let t = Tokens::get(ui.ctx());
    let resp = widgets::ghost_button(ui, "panel-left", language.tr(crate::i18n::text("ui.menu")));
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(230.0);
        ui.menu_button(language.tr(crate::i18n::text("ui.file")), |ui| crate::commands::registry_menu(app, ui, crate::i18n::text("ui.file")));
        ui.menu_button(language.tr(crate::i18n::text("ui.edit")), |ui| crate::commands::registry_menu(app, ui, crate::i18n::text("ui.edit")));
        ui.menu_button(language.tr(crate::i18n::text("ui.pages")), |ui| crate::commands::registry_menu(app, ui, crate::i18n::text("ui.pages")));
        ui.menu_button(language.tr(crate::i18n::text("ui.view")), |ui| {
            if let Some(i) = app.active {
                let v = &mut app.views[i];
                ui.label(egui::RichText::new(crate::i18n::text("ui.zoom")).color(t.text_faint).small());
                if widgets::menu_item(ui, language.tr(crate::i18n::text("ui.actual_size")), "⌘1").clicked() {
                    v.set_zoom(1.0);
                }
                if widgets::menu_item(ui, language.tr(crate::i18n::text("ui.zoom_to_page_level")), "⌘0").clicked() {
                    v.fit = Fit::Page;
                }
                if widgets::menu_item(ui, language.tr(crate::i18n::text("ui.fit_to_width")), "⌘2").clicked() {
                    v.fit = Fit::Width;
                }
                if widgets::menu_item(ui, crate::i18n::text("ui.fit_to_height"), "").clicked() {
                    v.fit = Fit::Height;
                    v.goto = Some((v.current, 0.0));
                }
                if widgets::menu_item(ui, crate::i18n::text("ui.fit_visible"), "⌘3").clicked() {
                    ui.close();
                    app.execute("view.fit_visible");
                    return;
                }
                if widgets::menu_item(ui, crate::i18n::text("ui.zoom_in"), "⌘+").clicked() {
                    v.zoom_step(true);
                }
                if widgets::menu_item(ui, crate::i18n::text("ui.zoom_out"), "⌘−").clicked() {
                    v.zoom_step(false);
                }
                if widgets::menu_item(ui, crate::i18n::text("ui.rotate_view_clockwise"), "⇧⌘+").clicked() {
                    v.rotate_view(true);
                }
                if widgets::menu_item(ui, crate::i18n::text("ui.rotate_view_counterclockwise"), "⇧⌘−").clicked() {
                    v.rotate_view(false);
                }
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::text("ui.page_navigation")).color(t.text_faint).small());
                if ui.add_enabled(!v.back.is_empty(), egui::Button::new(crate::i18n::text("ui.previous_view")).shortcut_text("⌘[")).clicked() {
                    v.view_history(false);
                }
                if ui.add_enabled(!v.forward.is_empty(), egui::Button::new(crate::i18n::text("ui.next_view")).shortcut_text("⌘]")).clicked() {
                    v.view_history(true);
                }
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::text("ui.page_display")).color(t.text_faint).small());
                for (l, label) in [
                    (PageLayout::Continuous, crate::i18n::text("ui.continuous_scrolling")),
                    (PageLayout::TwoUp, crate::i18n::text("ui.two_page_view")),
                    (PageLayout::Single, crate::i18n::text("ui.single_page")),
                ] {
                    if ui.radio(v.layout == l, label).clicked() {
                        v.layout = l;
                        v.goto = Some((v.current, 0.0));
                    }
                }
                if ui
                    .add_enabled(
                        v.layout == PageLayout::TwoUp,
                        egui::Checkbox::new(&mut v.cover, crate::i18n::text("ui.show_cover_page_in_two_page_view")),
                    )
                    .changed()
                {
                    v.goto = Some((v.current, 0.0));
                }
                ui.separator();
            }
            crate::commands::registry_menu(app, ui, crate::i18n::text("ui.view"));
            ui.menu_button(language.tr(crate::i18n::text("ui.display_theme")), |ui| {
                let ctx = ui.ctx().clone();
                if ui.radio(app.follow_system_theme, crate::i18n::text("ui.use_system_setting")).clicked() {
                    app.follow_system_theme = true;
                }
                if ui.radio(!app.follow_system_theme && app.theme == ThemeKind::Light, crate::i18n::text("ui.light_gray")).clicked() {
                    app.follow_system_theme = false;
                    app.set_theme(&ctx, ThemeKind::Light);
                }
                if ui.radio(!app.follow_system_theme && app.theme == ThemeKind::Dark, crate::i18n::text("ui.dark_gray")).clicked() {
                    app.follow_system_theme = false;
                    app.set_theme(&ctx, ThemeKind::Dark);
                }
            });
            ui.menu_button(language.tr(crate::i18n::text("ui.side_panels")), |ui| {
                for (p, label) in [
                    (RightPanel::Comments, crate::i18n::text("ui.comments")),
                    (RightPanel::Bookmarks, crate::i18n::text("ui.bookmarks")),
                    (RightPanel::Pages, crate::i18n::text("ui.pages")),
                    (RightPanel::Fields, crate::i18n::text("ui.fields")),
                    (RightPanel::Layers, crate::i18n::text("ui.layers")),
                    (RightPanel::Attachments, crate::i18n::text("ui.attachments")),
                    (RightPanel::Signatures, crate::i18n::text("ui.signatures")),
                    (RightPanel::Accessibility, crate::i18n::text("ui.accessibility_checker")),
                    (RightPanel::Search, crate::i18n::text("ui.search")),
                    (RightPanel::Compare, crate::i18n::text("ui.compare")),
                ] {
                    if ui.radio(app.right == Some(p), label).clicked() {
                        app.right = Some(p);
                    }
                }
            });
        });
        ui.menu_button(language.tr(crate::i18n::text("ui.help")), |ui| crate::commands::registry_menu(app, ui, crate::i18n::text("ui.help")));
    });
}

pub fn right_rail(app: &mut PrintCraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some((index, id)) = app.active_ids() else { return };
    let Some(doc) = app.session.get(id) else { return };
    let has_signatures = doc.is_signed();
    let has_check = app.a11y.report.as_ref().is_some_and(|(d, _)| *d == id);
    let (has_comments, has_outline, has_fields, has_layers, has_files) = (
        !doc.info.annotations.is_empty(),
        !doc.info.outline.is_empty(),
        !doc.info.fields.is_empty(),
        !doc.info.layers.is_empty(),
        !doc.info.attachments.is_empty(),
    );
    let page_count = doc.info.pages.len();
    let labels: Vec<String> = doc.info.pages.iter().map(|p| p.label.clone()).collect();
    egui::Panel::right("rail")
        .resizable(false)
        .exact_size(48.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(6, 8)).stroke(Stroke::new(1.0, t.divider)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let mut rail_button = |ui: &mut egui::Ui, panel: RightPanel, icon: &str, tip: &str, has: bool| {
                let selected = app.right == Some(panel);
                let r = icons::button(ui, icon, 34.0, selected, tip);
                if has && !selected {
                    let c = r.rect.right_top() + vec2(-8.0, 8.0);
                    ui.painter().circle_filled(c, 3.0, t.accent);
                }
                if r.clicked() {
                    app.right = if selected { None } else { Some(panel) };
                }
            };
            rail_button(ui, RightPanel::Comments, "message-square-text", crate::i18n::text("ui.comments"), has_comments);
            rail_button(ui, RightPanel::Bookmarks, "bookmark", crate::i18n::text("ui.bookmarks"), has_outline);
            rail_button(ui, RightPanel::Pages, "files", crate::i18n::text("ui.page_thumbnails"), false);
            rail_button(ui, RightPanel::Fields, "text-cursor-input", crate::i18n::text("ui.form_fields"), has_fields);
            rail_button(ui, RightPanel::Layers, "layers", crate::i18n::text("ui.layers"), has_layers);
            rail_button(ui, RightPanel::Attachments, "paperclip", crate::i18n::text("ui.attachments"), has_files);
            rail_button(ui, RightPanel::Signatures, "signature", crate::i18n::text("ui.signatures"), has_signatures);
            if has_check {
                rail_button(ui, RightPanel::Accessibility, "accessibility", crate::i18n::text("ui.accessibility_checker"), false);
            }

            // Page navigation cluster at the bottom (as in Acrobat's rail).
            let view = &mut app.views[index];
            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if icons::button(ui, "zoom-out", 32.0, false, crate::i18n::text("ui.zoom_out_fa0feb")).clicked() {
                    view.zoom_step(false);
                }
                if icons::button(ui, "zoom-in", 32.0, false, crate::i18n::text("ui.zoom_in_fed0f2")).clicked() {
                    view.zoom_step(true);
                }
                if icons::button(ui, "rotate-cw", 32.0, false, crate::i18n::text("ui.rotate_view_clockwise_cefa3a")).clicked() {
                    view.rotate_view(true);
                }
                let fit_icon = if view.fit == Fit::Width { "maximize" } else { "columns-2" };
                if icons::button(ui, fit_icon, 32.0, false, crate::i18n::text("ui.toggle_fit_page_fit_width")).clicked() {
                    view.fit = if view.fit == Fit::Width { Fit::Page } else { Fit::Width };
                    view.goto = Some((view.current, 0.0));
                }
                ui.label(egui::RichText::new(format!("{:.0}%", view.zoom * 100.0)).font(theme::regular(10.5)).color(t.text_faint));
                ui.add_space(6.0);
                if icons::button(ui, "chevron-down", 30.0, false, crate::i18n::text("ui.next_page")).clicked() {
                    view.go_to_page(view.current + 1);
                }
                if icons::button(ui, "chevron-up", 30.0, false, crate::i18n::text("ui.previous_page")).clicked() {
                    view.go_to_page(view.current.saturating_sub(1));
                }
                ui.label(egui::RichText::new(page_count.to_string()).font(theme::regular(11.0)).color(t.text_muted));
                let edit = egui::TextEdit::singleline(&mut view.page_input)
                    .id(egui::Id::new("page-input"))
                    .desired_width(34.0)
                    .horizontal_align(Align::Center)
                    .font(theme::medium(12.0))
                    .margin(vec2(2.0, 4.0));
                let r = egui::Frame::NONE
                    .fill(t.field)
                    .stroke(Stroke::new(1.0, t.border))
                    .corner_radius(CornerRadius::same(5))
                    .show(ui, |ui| ui.add(edit))
                    .inner
                    .on_hover_text(crate::i18n::text("ui.current_page_type_a_page_number_or_label_such_as_iv_and_press_enter"));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    // A page label first (logical page numbers, as Acrobat), then a number.
                    let typed = view.page_input.clone();
                    if !view.go_to_typed(&typed, &labels) {
                        view.page_input = (view.current + 1).to_string();
                    }
                }
            });
        });
}
