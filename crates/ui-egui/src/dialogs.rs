//! Modal dialogs: Document Properties, Keyboard Shortcuts, About.

use egui::{Align, Layout};

use crate::theme::{self, Tokens};
use printcraft_engine::Edit;

use crate::{CloseRequest, Dialog, PrintCraftApp, PropsTab, panels::human_size, widgets};

const INFO_KEYS: [&str; 4] = ["Title", "Author", "Subject", "Keywords"];

pub fn show(app: &mut PrintCraftApp, ctx: &egui::Context) {
    password(app, ctx);
    save_prompt(app, ctx);
    crate::updates::dialog(app, ctx);
    let Some(dialog) = app.dialog else {
        app.props_draft = None;
        app.view_draft = None;
        return;
    };
    // Seed the editable Description fields from the document when the dialog opens.
    if let (Dialog::Properties(_), Some((_, id))) = (dialog, app.active_ids())
        && app.props_draft.as_ref().is_none_or(|(d, _)| *d != id)
        && let Some(doc) = app.session.get(id)
    {
        app.props_draft = Some((id, INFO_KEYS.map(|k| doc.info_value(k).unwrap_or_default())));
        app.view_draft = Some((id, doc.initial_view()));
    }
    let mut apply = false;
    let mut split_now: Option<crate::SplitPlan> = None;
    let mut split_ready: Option<crate::SplitPlan> = None;
    let mut extract_now = false;
    let mut rotate_now = false;
    let mut link_now: Option<Edit> = None;
    let mut recover: Option<bool> = None;
    let mut number_now: Option<Edit> = None;
    let mut apply_number = false;
    let mut link_command: Option<&'static str> = None;
    let mut protect_now = false;
    let mut boxes_now = false;
    let mut marks_now = false;
    let mut export_now = false;
    let mut props_now = false;
    let mut field_props_now = false;
    let mut redact_now: Option<Dialog> = None;
    let mut print_go = false;
    let mut revert_now = false;
    let mut summarize_now = false;
    let mut optimize_now = false;
    let mut duplicate_now: Option<Edit> = None;
    let mut replace_now = false;
    let mut open_revision: Option<usize> = None;
    let mut a11y_now = false;
    let mut ocr_now = false;
    let mut compare_now = false;
    let mut combine_now = false;
    let mut stamp_now = false;
    let mut alt_now = false;
    let t = Tokens::get(ctx);
    let mut close = false;
    let mut next = dialog;
    let modal = egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
        ui.set_width(match dialog {
            Dialog::Properties(_) => 640.0,
            Dialog::Print => 820.0,
            Dialog::FieldProps => 600.0,
            _ => 520.0,
        });
        // Dialog controls are outlined (radio buttons, check boxes, combo boxes and number fields
        // would otherwise blend into the dialog, whose fill matches the theme's field colour).
        let w = &mut ui.visuals_mut().widgets;
        w.inactive.bg_stroke = egui::Stroke::new(1.0, t.border);
        w.inactive.weak_bg_fill = t.field;
        // Slider rails and check-box interiors use the plain fill.
        w.inactive.bg_fill = t.hover;
        w.hovered.bg_stroke = egui::Stroke::new(1.0, t.text_muted);
        match dialog {
            Dialog::Properties(tab) => {
                ui.label(egui::RichText::new(crate::i18n::text("ui.document_properties")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for (tb, label) in [
                        (PropsTab::Description, crate::i18n::text("ui.description")),
                        (PropsTab::InitialView, crate::i18n::text("ui.initial_view")),
                        (PropsTab::Security, crate::i18n::text("ui.security")),
                        (PropsTab::Fonts, crate::i18n::text("ui.fonts")),
                        (PropsTab::Advanced, crate::i18n::text("ui.advanced")),
                    ] {
                        if widgets::mode_tab(ui, label, tab == tb).clicked() {
                            next = Dialog::Properties(tb);
                        }
                    }
                });
                ui.separator();
                let Some((_, id)) = app.active_ids() else { return };
                let Some(doc) = app.session.get(id) else { return };
                let i = &doc.info;
                let row = |ui: &mut egui::Ui, k: &str, v: String| {
                    ui.label(egui::RichText::new(k).color(t.text_muted));
                    ui.label(if v.is_empty() { egui::RichText::new("—").color(t.text_faint) } else { egui::RichText::new(v) });
                    ui.end_row();
                };
                egui::ScrollArea::vertical().max_height(460.0).auto_shrink([false, true]).show(ui, |ui| {
                    egui::Grid::new("props").num_columns(2).spacing([18.0, 8.0]).min_col_width(140.0).show(ui, |ui| match tab {
                        PropsTab::Description => {
                            row(ui, crate::i18n::text("ui.file"), doc.name.clone());
                            match app.props_draft.as_mut() {
                                Some((_, draft)) if doc.allows_modification() => {
                                    for (k, v) in INFO_KEYS.iter().zip(draft.iter_mut()) {
                                        let l = ui.label(egui::RichText::new(app.language.tr(k)).color(t.text_muted));
                                        ui.add(
                                            egui::TextEdit::singleline(v)
                                                .desired_width(420.0)
                                                .background_color(t.field)
                                                .margin(egui::Margin::symmetric(6, 4))
                                                .id_salt(("info", *k)),
                                        )
                                        .labelled_by(l.id);
                                        ui.end_row();
                                    }
                                }
                                _ => {
                                    row(ui, crate::i18n::text("ui.title"), i.title.clone().unwrap_or_default());
                                    row(ui, crate::i18n::text("ui.author"), i.author.clone().unwrap_or_default());
                                    row(ui, crate::i18n::text("ui.subject"), i.subject.clone().unwrap_or_default());
                                    row(ui, crate::i18n::text("ui.keywords"), i.keywords.clone().unwrap_or_default());
                                }
                            }
                            row(ui, crate::i18n::text("ui.application"), i.creator.clone().unwrap_or_default());
                            row(ui, crate::i18n::text("ui.pdf_producer"), i.producer.clone().unwrap_or_default());
                        }
                        PropsTab::InitialView => {
                            use printcraft_engine::{InitialLayout as L, Magnification as M, Navigation as N};
                            let editable = doc.allows_modification();
                            let pages = i.pages.len();
                            let Some((_, v)) = app.view_draft.as_mut() else { return };
                            ui.label(egui::RichText::new(crate::i18n::text("ui.layout_and_magnification")).font(theme::semibold(12.5)));
                            ui.end_row();
                            ui.label(crate::i18n::text("ui.navigation_tab"));
                            ui.add_enabled_ui(editable, |ui| {
                                egui::ComboBox::from_id_salt("iv-nav")
                                    .selected_text(
                                        format!("{:?}", v.navigation)
                                            .replace("PageOnly", crate::i18n::text("ui.page_only"))
                                            .replace(crate::i18n::text("ui.pages"), crate::i18n::text("ui.pages_panel_and_page")),
                                    )
                                    .show_ui(ui, |ui| {
                                        for (n, l) in [
                                            (N::PageOnly, crate::i18n::text("ui.page_only")),
                                            (N::Bookmarks, crate::i18n::text("ui.bookmarks_panel_and_page")),
                                            (N::Pages, crate::i18n::text("ui.pages_panel_and_page")),
                                            (N::Attachments, crate::i18n::text("ui.attachments_panel_and_page")),
                                            (N::Layers, crate::i18n::text("ui.layers_panel_and_page")),
                                        ] {
                                            ui.selectable_value(&mut v.navigation, n, l);
                                        }
                                    });
                            });
                            ui.end_row();
                            ui.label(crate::i18n::text("ui.page_layout"));
                            ui.add_enabled_ui(editable, |ui| {
                                let names = [
                                    (L::Default, crate::i18n::text("ui.default")),
                                    (L::SinglePage, crate::i18n::text("ui.single_page_f9ebd4")),
                                    (L::SinglePageContinuous, crate::i18n::text("ui.single_page_continuous")),
                                    (L::TwoUp, crate::i18n::text("ui.two_up_facing")),
                                    (L::TwoUpContinuous, crate::i18n::text("ui.two_up_continuous_facing")),
                                    (L::TwoUpCoverPage, crate::i18n::text("ui.two_up_cover_page")),
                                    (L::TwoUpContinuousCoverPage, crate::i18n::text("ui.two_up_continuous_cover_page")),
                                ];
                                let shown = names.iter().find(|(l, _)| *l == v.layout).map_or(crate::i18n::text("ui.default"), |(_, n)| n);
                                egui::ComboBox::from_id_salt("iv-layout").selected_text(shown).show_ui(ui, |ui| {
                                    for (l, n) in names {
                                        ui.selectable_value(&mut v.layout, l, n);
                                    }
                                });
                            });
                            ui.end_row();
                            ui.label(crate::i18n::text("ui.magnification"));
                            ui.add_enabled_ui(editable, |ui| {
                                ui.horizontal(|ui| {
                                    let names = [
                                        (M::Default, crate::i18n::text("ui.default")),
                                        (M::ActualSize, crate::i18n::text("ui.actual_size_ef37f4")),
                                        (M::FitPage, crate::i18n::text("ui.fit_page")),
                                        (M::FitWidth, crate::i18n::text("ui.fit_width")),
                                        (M::FitHeight, crate::i18n::text("ui.fit_height")),
                                        (M::FitVisible, crate::i18n::text("ui.fit_visible_ba976e")),
                                    ];
                                    let shown = match v.magnification {
                                        M::Percent(p) => format!("{p:.0}%"),
                                        m => names.iter().find(|(x, _)| *x == m).map_or(crate::i18n::text("ui.default"), |(_, n)| n).to_string(),
                                    };
                                    egui::ComboBox::from_id_salt("iv-mag").selected_text(shown).show_ui(ui, |ui| {
                                        for (m, n) in names {
                                            ui.selectable_value(&mut v.magnification, m, n);
                                        }
                                        for p in [50.0, 75.0, 125.0, 150.0, 200.0] {
                                            ui.selectable_value(&mut v.magnification, M::Percent(p), format!("{p:.0}%"));
                                        }
                                    });
                                });
                            });
                            ui.end_row();
                            ui.label(crate::i18n::text("ui.open_to_page"));
                            ui.add_enabled_ui(editable, |ui| {
                                let mut p = v.page + 1;
                                if ui.add(egui::DragValue::new(&mut p).range(1..=pages.max(1))).changed() {
                                    v.page = p - 1;
                                }
                                ui.label(crate::msg!(of_value, pages = pages));
                            });
                            ui.end_row();
                            ui.label(egui::RichText::new(crate::i18n::text("ui.window_options")).font(theme::semibold(12.5)));
                            ui.end_row();
                            ui.label("");
                            ui.add_enabled_ui(editable, |ui| {
                                ui.vertical(|ui| {
                                    ui.checkbox(&mut v.fit_window, crate::i18n::text("ui.resize_window_to_initial_page"));
                                    ui.checkbox(&mut v.center_window, crate::i18n::text("ui.center_window_on_screen"));
                                    ui.checkbox(&mut v.full_screen, crate::i18n::text("ui.open_in_full_screen_mode"));
                                    ui.horizontal(|ui| {
                                        ui.label(crate::i18n::text("ui.show"));
                                        ui.radio_value(&mut v.display_title, false, crate::i18n::text("ui.file_name"));
                                        ui.radio_value(&mut v.display_title, true, crate::i18n::text("ui.document_title"));
                                    });
                                });
                            });
                            ui.end_row();
                            ui.label(egui::RichText::new(crate::i18n::text("ui.user_interface_options")).font(theme::semibold(12.5)));
                            ui.end_row();
                            ui.label("");
                            ui.add_enabled_ui(editable, |ui| {
                                ui.vertical(|ui| {
                                    ui.checkbox(&mut v.hide_menubar, crate::i18n::text("ui.hide_menu_bar"));
                                    ui.checkbox(&mut v.hide_toolbar, crate::i18n::text("ui.hide_toolbars"));
                                    ui.checkbox(&mut v.hide_window_ui, crate::i18n::text("ui.hide_window_controls"));
                                });
                            });
                            ui.end_row();
                        }
                        PropsTab::Security => match doc.security_summary() {
                            None => {
                                row(ui, crate::i18n::text("ui.security_method"), crate::i18n::text("ui.no_security").into());
                                row(ui, crate::i18n::text("ui.restrictions"), crate::i18n::text("ui.none_everything_is_allowed").into());
                                if doc.allows_security_change() && ui.button(crate::i18n::text("ui.protect_using_password")).clicked() {
                                    link_command = Some("protect.password");
                                }
                            }
                            Some(sec) => {
                                row(ui, crate::i18n::text("ui.security_method"), crate::i18n::text("ui.password_security").into());
                                row(ui, crate::i18n::text("ui.encryption"), sec.method.clone());
                                row(
                                    ui,
                                    crate::i18n::text("ui.opened_with"),
                                    if sec.pending {
                                        crate::i18n::text("ui.protection_is_applied_when_you_save").into()
                                    } else if sec.owner {
                                        crate::i18n::text("ui.owner_password_no_restrictions_apply").into()
                                    } else {
                                        crate::i18n::text("ui.user_password").into()
                                    },
                                );
                                if doc.allows_security_change() {
                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::text("ui.change_settings")).clicked() {
                                            link_command = Some("protect.password");
                                        }
                                        if ui.button(crate::i18n::text("ui.remove_security")).clicked() {
                                            link_command = Some("protect.remove");
                                        }
                                    });
                                }
                                let p = sec.permissions;
                                let yes = |b: bool| {
                                    if b { crate::i18n::text("ui.allowed").to_string() } else { crate::i18n::text("ui.not_allowed").to_string() }
                                };
                                row(
                                    ui,
                                    crate::i18n::text("ui.printing"),
                                    if !p.print() {
                                        crate::i18n::text("ui.not_allowed").into()
                                    } else if p.print_high_quality() {
                                        crate::i18n::text("ui.high_resolution").into()
                                    } else {
                                        crate::i18n::text("ui.low_resolution").into()
                                    },
                                );
                                row(ui, crate::i18n::text("ui.changing_the_document"), yes(p.modify()));
                                row(ui, crate::i18n::text("ui.document_assembly"), yes(p.assemble()));
                                row(ui, crate::i18n::text("ui.content_copying"), yes(p.copy()));
                                row(ui, crate::i18n::text("ui.content_copying_for_accessibility"), yes(p.extract_for_accessibility()));
                                row(ui, crate::i18n::text("ui.commenting"), yes(p.annotate()));
                                row(ui, crate::i18n::text("ui.filling_of_form_fields"), yes(p.fill_forms()));
                            }
                        },
                        PropsTab::Fonts => {
                            if i.fonts.is_empty() {
                                row(ui, crate::i18n::text("ui.fonts"), crate::i18n::text("ui.no_fonts_are_referenced_by_the_pages").into());
                            }
                            for f in &i.fonts {
                                let mut detail = f.kind.clone();
                                if let Some(e) = &f.encoding {
                                    detail.push_str(&format!(" · {e}"));
                                }
                                detail.push_str(if f.subset {
                                    crate::i18n::text("ui.embedded_subset")
                                } else if f.embedded {
                                    crate::i18n::text("ui.embedded")
                                } else {
                                    crate::i18n::text("ui.not_embedded_substituted")
                                });
                                row(ui, &f.name, detail);
                            }
                        }
                        PropsTab::Advanced => {
                            row(ui, crate::i18n::text("ui.pdf_version"), i.pdf_version.clone());
                            row(ui, crate::i18n::text("ui.location"), doc.path.clone().unwrap_or_default());
                            row(ui, crate::i18n::text("ui.file_size"), crate::msg!(value_value_bytes, human_size(i.file_size), i.file_size));
                            let p = &i.pages[0];
                            row(ui, crate::i18n::text("ui.page_size"), crate::msg!(value_value_in, p.width / 72.0, p.height / 72.0));
                            row(ui, crate::i18n::text("ui.number_of_pages"), i.pages.len().to_string());
                            row(ui, crate::i18n::text("ui.tagged_pdf"), yes(i.tagged));
                            row(ui, crate::i18n::text("ui.form_fields"), i.fields.len().to_string());
                            row(ui, crate::i18n::text("ui.comments"), i.annotations.len().to_string());
                            row(ui, crate::i18n::text("ui.layers"), i.layers.len().to_string());
                            row(ui, crate::i18n::text("ui.attachments"), i.attachments.len().to_string());
                            row(ui, "JavaScript", yes(i.has_javascript));
                            // Each incremental update is a revision; earlier ones open as their own document.
                            let ends = doc.revision_ends();
                            if ends.len() > 1 {
                                ui.label(egui::RichText::new(crate::i18n::text("ui.revisions")).color(t.text_muted));
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(ends.len().to_string());
                                    for n in (1..ends.len()).rev().take(12) {
                                        if ui
                                            .small_button(crate::msg!(view_revision_value, n = n))
                                            .on_hover_text(crate::i18n::text("ui.open_the_file_as_it_was_saved_then"))
                                            .clicked()
                                        {
                                            open_revision = Some(n);
                                        }
                                    }
                                });
                                ui.end_row();
                            }
                            // Reading Options: binding and language.
                            if let Some((_, v)) = app.view_draft.as_mut() {
                                let editable = doc.allows_modification();
                                ui.label(egui::RichText::new(crate::i18n::text("ui.binding")).color(t.text_muted));
                                ui.add_enabled_ui(editable, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.radio_value(&mut v.right_to_left, false, crate::i18n::text("ui.left_edge"));
                                        ui.radio_value(&mut v.right_to_left, true, crate::i18n::text("ui.right_edge"));
                                    });
                                });
                                ui.end_row();
                                let l = ui.label(egui::RichText::new(crate::i18n::text("ui.language")).color(t.text_muted));
                                let mut lang = v.language.clone().unwrap_or_default();
                                if ui
                                    .add_enabled(
                                        editable,
                                        egui::TextEdit::singleline(&mut lang).hint_text(crate::i18n::text("ui.e_g_en_us")).desired_width(160.0),
                                    )
                                    .labelled_by(l.id)
                                    .changed()
                                {
                                    v.language = (!lang.trim().is_empty()).then(|| lang.trim().to_string());
                                }
                                ui.end_row();
                            }
                            // What was repaired while reading a damaged file (fidelity: never silent).
                            let repairs = doc.repair_log();
                            row(
                                ui,
                                crate::i18n::text("ui.repairs"),
                                if repairs.is_empty() { crate::i18n::text("ui.none").to_string() } else { repairs.len().to_string() },
                            );
                            if !repairs.is_empty() {
                                ui.label("");
                                egui::CollapsingHeader::new(crate::i18n::text("ui.repair_log")).show(ui, |ui| {
                                    for r in &repairs {
                                        ui.add(egui::Label::new(egui::RichText::new(r).small()).wrap());
                                    }
                                });
                                ui.end_row();
                            }
                        }
                    })
                });
            }
            Dialog::Split => {
                ui.label(egui::RichText::new(crate::i18n::text("ui.split_document")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                let Some((vi, id)) = app.active_ids() else { return };
                let n = app.session.get(id).map(|d| d.info.pages.len()).unwrap_or(0);
                let selected: Vec<usize> = app.views[vi].selected.iter().copied().filter(|p| *p > 0).collect();
                let marks = app.session.bookmark_splits(id);
                let draft = &mut app.split_draft;
                use crate::SplitMode as M;
                if selected.is_empty() && draft.mode == M::Selection {
                    draft.mode = M::Pages;
                }
                ui.radio_value(&mut draft.mode, M::Pages, crate::i18n::text("ui.number_of_pages"));
                ui.add_enabled_ui(draft.mode == M::Pages, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.label(crate::i18n::text("ui.pages_per_file"));
                        ui.add(egui::DragValue::new(&mut draft.every).range(1..=n.max(1)));
                    });
                });
                ui.radio_value(&mut draft.mode, M::Size, crate::i18n::text("ui.file_size"));
                ui.add_enabled_ui(draft.mode == M::Size, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.label(crate::i18n::text("ui.at_most"));
                        ui.add(egui::DragValue::new(&mut draft.size_mb).range(0.05..=2000.0).speed(0.1).suffix(crate::i18n::text("ui.mb")));
                    });
                });
                ui.add_enabled_ui(!marks.is_empty(), |ui| {
                    ui.radio_value(&mut draft.mode, M::Bookmarks, crate::msg!(top_level_bookmarks_value, marks.len()))
                });
                ui.add_enabled_ui(!selected.is_empty(), |ui| {
                    ui.radio_value(&mut draft.mode, M::Selection, crate::i18n::text("ui.before_each_selected_page_select_pages_in_organize"))
                });
                let plan = match draft.mode {
                    M::Pages => crate::SplitPlan::By(printcraft_engine::SplitBy::PageCount(draft.every)),
                    M::Selection => crate::SplitPlan::By(printcraft_engine::SplitBy::Before(selected)),
                    M::Size => crate::SplitPlan::Size((draft.size_mb * 1_048_576.0) as usize),
                    M::Bookmarks => crate::SplitPlan::Bookmarks,
                };
                let files = match &plan {
                    crate::SplitPlan::By(by) => Some(printcraft_engine::split_ranges(n, by).len()),
                    crate::SplitPlan::Bookmarks => {
                        Some(printcraft_engine::split_ranges(n, &printcraft_engine::SplitBy::Before(marks.iter().map(|m| m.0).collect())).len())
                    }
                    crate::SplitPlan::Size(_) => None,
                };
                ui.add_space(8.0);
                let text = match files {
                    Some(f) => crate::msg!(creates_value_file_value_from_value_pages, crate::i18n::plural_suffix(f), n = n, f = f),
                    None => crate::msg!(each_file_holds_as_many_of_the_value_pages_as_fit, n = n),
                };
                ui.label(egui::RichText::new(text).color(t.text_muted));
                if files.is_none_or(|f| f > 1) {
                    split_ready = Some(plan);
                }
            }
            Dialog::ReplacePages => {
                let count = app.active_ids().and_then(|(_, id)| app.session.get(id)).map(|d| d.info.pages.len()).unwrap_or(1).max(1);
                let Some(d) = app.replace_draft.as_mut() else {
                    close = true;
                    return;
                };
                ui.label(egui::RichText::new(crate::i18n::text("ui.replace_pages")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                d.to = d.to.clamp(1, count);
                d.from = d.from.clamp(1, d.to);
                let n = d.to - d.from + 1;
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::text("ui.original_replace_pages"));
                    ui.add(egui::DragValue::new(&mut d.from).range(1..=count));
                    ui.label(crate::i18n::text("ui.to_663ea1"));
                    ui.add(egui::DragValue::new(&mut d.to).range(1..=count));
                    ui.label(egui::RichText::new(crate::msg!(of_value_3cb85d, count = count)).color(t.text_muted));
                });
                let max_start = d.src_pages.saturating_sub(n) + 1;
                d.src_from = d.src_from.clamp(1, max_start.max(1));
                ui.horizontal(|ui| {
                    ui.label(crate::msg!(replacement_pages_of_value, d.name));
                    ui.add(egui::DragValue::new(&mut d.src_from).range(1..=max_start.max(1)));
                    ui.label(crate::msg!(to_value, d.src_from + n - 1));
                    ui.label(egui::RichText::new(crate::msg!(of_value_e82ce1, d.src_pages)).color(t.text_muted));
                });
                let fits = n <= d.src_pages;
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(if fits {
                        crate::i18n::text("ui.only_the_page_content_changes_links_comments_form_fields_and_bookmarks_on_the_original_pages_stay")
                    } else {
                        crate::i18n::text("ui.the_replacement_file_doesn_t_have_that_many_pages")
                    })
                    .small()
                    .color(t.text_faint),
                );
                ui.add_space(10.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add_enabled_ui(fits, |ui| widgets::pill_button(ui, crate::i18n::text("ui.ok"), true)).inner.clicked() {
                        replace_now = true;
                        close = true;
                    }
                    if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                        close = true;
                    }
                });
                return;
            }
            Dialog::RedactPages => {
                let n = app.active_ids().and_then(|(_, id)| app.session.get(id)).map_or(1, |d| d.info.pages.len());
                let (ok, cancel) = crate::redact_ui::pages_body(ui, &mut app.redact_pages_draft, n, &t);
                if ok {
                    redact_now = Some(dialog);
                }
                close = ok || cancel;
                return;
            }
            Dialog::RedactSearch => {
                let (go, cancel) = crate::redact_ui::search_body(ui, &mut app.redact_search, &t);
                if go {
                    redact_now = Some(dialog);
                }
                close = cancel;
                return;
            }
            Dialog::RedactProps => {
                let mut prefs = app.redact_prefs.clone();
                let (ok, cancel) = crate::redact_ui::props_body(ui, &mut prefs, &t);
                app.redact_prefs = prefs;
                close = ok || cancel;
                return;
            }
            Dialog::RedactApply => {
                let marks = app.active_ids().and_then(|(_, id)| app.session.get(id)).map_or(0, |d| d.redaction_marks());
                let (ok, cancel) = crate::redact_ui::apply_body(ui, marks, &t);
                if ok {
                    redact_now = Some(dialog);
                }
                close = ok || cancel;
                return;
            }
            Dialog::LinkProps => {
                let pages = app.active_ids().and_then(|(_, id)| app.session.get(id)).map_or(1, |d| d.info.pages.len());
                let Some(d) = app.link_draft.as_mut() else {
                    close = true;
                    return;
                };
                let (ok, cancel) = crate::link_ui::body(ui, d, pages, &t);
                if ok {
                    link_now = Some(crate::link_ui::edit_for(d));
                }
                close = ok || cancel;
                return;
            }
            Dialog::Extract => {
                let count = app.active_ids().map_or(0, |(i, _)| app.views[i].target_pages().len());
                ui.label(egui::RichText::new(crate::i18n::text("ui.extract_pages")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                ui.label(crate::msg!(value_page_value_selected, crate::i18n::plural_suffix(count), count = count));
                ui.checkbox(&mut app.extract_draft.delete, crate::i18n::text("ui.delete_pages_after_extracting"));
                ui.checkbox(&mut app.extract_draft.separate, crate::i18n::text("ui.extract_pages_as_separate_files"));
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::pill_button(ui, crate::i18n::text("ui.extract"), true).clicked() {
                        extract_now = true;
                        close = true;
                    }
                    if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                        close = true;
                    }
                });
                return;
            }
            Dialog::RotatePages => {
                use printcraft_engine::{PageOrientation as O, PageParity as P};
                let n = app.active_ids().and_then(|(_, id)| app.session.get(id)).map_or(1, |d| d.info.pages.len());
                let d = &mut app.rotate_draft;
                ui.label(egui::RichText::new(crate::i18n::text("ui.rotate_pages")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                egui::Grid::new("rotate-pages").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label(crate::i18n::text("ui.direction"));
                    egui::ComboBox::from_id_salt("rotate-dir")
                        .selected_text(match d.degrees {
                            270 => crate::i18n::text("ui.counterclockwise_90_degrees"),
                            180 => crate::i18n::text("ui.180_degrees"),
                            _ => crate::i18n::text("ui.clockwise_90_degrees"),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut d.degrees, 90, crate::i18n::text("ui.clockwise_90_degrees"));
                            ui.selectable_value(&mut d.degrees, 270, crate::i18n::text("ui.counterclockwise_90_degrees"));
                            ui.selectable_value(&mut d.degrees, 180, crate::i18n::text("ui.180_degrees"));
                        });
                    ui.end_row();
                    ui.label(crate::i18n::text("ui.pages_0f36bb"));
                    ui.vertical(|ui| {
                        ui.radio_value(&mut d.which, 0, crate::i18n::text("ui.all"));
                        ui.radio_value(&mut d.which, 1, crate::i18n::text("ui.selection"));
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut d.which, 2, crate::i18n::text("ui.from"));
                            ui.add_enabled(d.which == 2, egui::DragValue::new(&mut d.from).range(1..=n));
                            ui.label(crate::i18n::text("ui.to_663ea1"));
                            ui.add_enabled(d.which == 2, egui::DragValue::new(&mut d.to).range(1..=n));
                            ui.label(crate::msg!(of_value_a2eabd, n = n));
                        });
                    });
                    ui.end_row();
                    ui.label(crate::i18n::text("ui.rotate"));
                    egui::ComboBox::from_id_salt("rotate-parity")
                        .selected_text(match d.parity {
                            P::Both => crate::i18n::text("ui.even_and_odd_pages"),
                            P::Even => crate::i18n::text("ui.even_pages_only"),
                            P::Odd => crate::i18n::text("ui.odd_pages_only"),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut d.parity, P::Both, crate::i18n::text("ui.even_and_odd_pages"));
                            ui.selectable_value(&mut d.parity, P::Even, crate::i18n::text("ui.even_pages_only"));
                            ui.selectable_value(&mut d.parity, P::Odd, crate::i18n::text("ui.odd_pages_only"));
                        });
                    ui.end_row();
                    ui.label("");
                    egui::ComboBox::from_id_salt("rotate-orient")
                        .selected_text(match d.orientation {
                            O::Both => crate::i18n::text("ui.landscape_and_portrait_pages"),
                            O::Landscape => crate::i18n::text("ui.landscape_pages"),
                            O::Portrait => crate::i18n::text("ui.portrait_pages"),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut d.orientation, O::Both, crate::i18n::text("ui.landscape_and_portrait_pages"));
                            ui.selectable_value(&mut d.orientation, O::Landscape, crate::i18n::text("ui.landscape_pages"));
                            ui.selectable_value(&mut d.orientation, O::Portrait, crate::i18n::text("ui.portrait_pages"));
                        });
                    ui.end_row();
                });
                d.to = d.to.max(d.from);
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::pill_button(ui, crate::i18n::text("ui.ok"), true).clicked() {
                        rotate_now = true;
                        close = true;
                    }
                    if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                        close = true;
                    }
                });
                return;
            }
            Dialog::DuplicateField => {
                let n = app.active_ids().and_then(|(_, id)| app.session.get(id)).map_or(1, |d| d.info.pages.len());
                let Some(d) = app.duplicate_draft.as_mut() else {
                    close = true;
                    return;
                };
                ui.label(egui::RichText::new(crate::i18n::text("ui.duplicate_field")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                ui.label(crate::msg!(duplicate_value_onto, d.name));
                ui.radio_value(&mut d.all, true, crate::i18n::text("ui.all_pages"));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.all, false, crate::i18n::text("ui.from"));
                    ui.add_enabled(!d.all, egui::DragValue::new(&mut d.from).range(1..=n));
                    ui.label(crate::i18n::text("ui.to_663ea1"));
                    ui.add_enabled(!d.all, egui::DragValue::new(&mut d.to).range(1..=n));
                    ui.label(crate::msg!(of_value_a2eabd, n = n));
                });
                d.to = d.to.max(d.from);
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::pill_button(ui, crate::i18n::text("ui.ok"), true).clicked() {
                        let pages: Vec<usize> = if d.all { (0..n).collect() } else { (d.from - 1..d.to.min(n)).collect() };
                        duplicate_now = Some(Edit::DuplicateField { name: d.name.clone(), pages });
                        close = true;
                    }
                    if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                        close = true;
                    }
                });
                return;
            }
            Dialog::Optimize => {
                let (ok, cancel) = crate::optimize_ui::body(ui, &mut app.optimize_draft, &t);
                optimize_now = ok;
                close = ok || cancel;
                if std::mem::take(&mut app.optimize_draft.audit) {
                    app.space_audit = app.active_ids().and_then(|(_, id)| app.session.get(id)).map(|d| d.audit_space()).unwrap_or_default();
                    next = Dialog::AuditSpace;
                }
                return;
            }
            Dialog::CertificateViewer => {
                ui.set_width(700.0);
                let trusted = app.session.trusted_certificates().to_vec();
                let Some(v) = app.cert_viewer.as_mut() else {
                    close = true;
                    return;
                };
                let (done, action) = crate::sign_ui::cert_viewer(ui, v, &trusted, &t);
                close = done;
                match action {
                    Some(crate::sign_ui::CertAction::Trust(c)) => app.trust_certificate(*c),
                    Some(crate::sign_ui::CertAction::Export(c)) => {
                        let pem = crate::sign_ui::certificate_pem(&c);
                        app.write_files(
                            &[(format!("{}.cer", c.display_name()), std::sync::Arc::new(pem.into_bytes()))],
                            crate::i18n::text("ui.export_certificate"),
                        );
                    }
                    None => {}
                }
                return;
            }
            Dialog::AuditSpace => {
                if crate::optimize_ui::audit_body(ui, &app.space_audit, &t) {
                    next = Dialog::Optimize;
                }
                return;
            }
            Dialog::Sign => {
                if crate::sign_ui::dialog(ui, app, &t) {
                    app.sign_draft = None;
                    close = true;
                }
                return;
            }
            Dialog::SummarizeComments => {
                ui.label(egui::RichText::new(crate::i18n::text("ui.summarize_options")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                ui.label(crate::i18n::text("ui.choose_a_layout"));
                let _ = ui.radio(true, crate::i18n::text("ui.comments_only"));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::text("ui.sort_comments_by"));
                    egui::ComboBox::from_id_salt("summary-sort").selected_text(app.language.tr(app.summary_sort.name())).show_ui(ui, |ui| {
                        for s in printcraft_engine::SummarySort::ALL {
                            ui.selectable_value(&mut app.summary_sort, s, app.language.tr(s.name()));
                        }
                    });
                });
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::pill_button(ui, crate::i18n::text("ui.create_pdf_comment_summary"), true).clicked() {
                        summarize_now = true;
                        close = true;
                    }
                    if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                        close = true;
                    }
                });
                return;
            }
            Dialog::Revert => {
                let name = app.active_ids().and_then(|(_, id)| app.session.get(id)).map(|d| d.name.clone()).unwrap_or_default();
                ui.label(egui::RichText::new(crate::i18n::text("ui.revert")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                ui.label(crate::msg!(revert_to_the_previously_saved_version_of_value_changes_since_then_can_t_be_undone_afterwards, name = name));
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::pill_button(ui, crate::i18n::text("ui.revert"), true).clicked() {
                        revert_now = true;
                        close = true;
                    }
                    if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                        close = true;
                    }
                });
                return;
            }
            Dialog::Print => {
                let Some((i, id)) = app.active_ids() else {
                    close = true;
                    return;
                };
                let Some(doc) = app.session.get(id) else { return };
                let sizes: Vec<(f64, f64)> = doc.info.pages.iter().map(|p| (p.width as f64, p.height as f64)).collect();
                let labels: Vec<String> = doc.info.pages.iter().map(|p| p.label.clone()).collect();
                let thumbs: std::collections::HashMap<usize, egui::TextureId> =
                    (0..sizes.len()).filter_map(|p| app.views[i].thumb_id(p).map(|t| (p, t))).collect();
                let (go, cancel) = crate::print_ui::body(ui, &mut app.print_draft, &t, &sizes, &labels, &|p| thumbs.get(&p).copied());
                print_go = go;
                close = go || cancel;
                return;
            }
            Dialog::RemoveHidden => {
                let (ok, cancel) = crate::redact_ui::hidden_body(ui, &mut app.hidden_draft, &t);
                if ok {
                    redact_now = Some(dialog);
                }
                close = ok || cancel;
                return;
            }
            Dialog::Sanitize => {
                let (ok, cancel) = crate::redact_ui::sanitize_body(ui, &t);
                if ok {
                    redact_now = Some(dialog);
                }
                close = ok || cancel;
                return;
            }
            Dialog::FieldProps => {
                let Some(d) = app.field_props.as_mut() else {
                    close = true;
                    return;
                };
                let (apply, cancel) = crate::prepare::body(ui, d, &t);
                field_props_now = apply;
                close = apply || cancel;
                return;
            }
            Dialog::CommentProps => {
                let (apply, cancel) = crate::comment_props::body(ui, app, &t);
                props_now = apply;
                close = apply || cancel;
                return;
            }
            Dialog::Signature => {
                let (apply, cancel) = crate::fill_sign::signature_pad(ui, &t, &mut app.signature_draft, &mut app.signature_preview);
                if apply {
                    let d = std::mem::take(&mut app.signature_draft);
                    let tool = if d.initials {
                        app.initials = Some(d.saved());
                        crate::fill_sign::FillTool::Initials
                    } else {
                        app.signature = Some(d.saved());
                        crate::fill_sign::FillTool::Signature
                    };
                    app.quick_tool = crate::QuickTool::Fill(tool);
                    app.toast = None;
                }
                close = apply || cancel;
                return;
            }
            Dialog::AltText => {
                let (save, cancel) = crate::a11y_ui::alt_body(ui, app, &t);
                alt_now = save;
                close = save || cancel;
                return;
            }
            Dialog::CreateStamp => {
                let (save, cancel) = crate::stamps_ui::create_body(ui, app, &t);
                stamp_now = save;
                close = save || cancel;
                return;
            }
            Dialog::Combine => {
                ui.set_width(620.0);
                let (go, cancel) = crate::combine_ui::body(ui, app, &t);
                combine_now = go;
                if cancel {
                    app.combine_draft.clear();
                }
                close = go || cancel;
                return;
            }
            Dialog::PdfA => {
                close = crate::standards_ui::body(ui, app, &t);
                return;
            }
            Dialog::ActionWizard => {
                ui.set_width(620.0);
                close = crate::actions_ui::body(ui, app, &t);
                return;
            }
            Dialog::CompareFiles => {
                let (go, cancel) = crate::compare_ui::body(ui, app, &t);
                compare_now = go;
                close = go || cancel;
                return;
            }
            Dialog::JsConsole => {
                ui.set_width(640.0);
                close = crate::js_ui::console_body(ui, app, &t);
                return;
            }
            Dialog::DocumentJs => {
                ui.set_width(640.0);
                close = crate::js_ui::document_js_body(ui, app, &t);
                return;
            }
            Dialog::Preferences => {
                close = crate::js_ui::preferences_body(ui, app, &t);
                return;
            }
            Dialog::RecognizeText => {
                let (go, cancel) = crate::ocr_ui::body(ui, app, &t);
                ocr_now = go;
                close = go || cancel;
                return;
            }
            Dialog::AccessibilityOptions => {
                let (start, cancel) = crate::a11y_ui::options_body(ui, app, &t);
                a11y_now = start;
                close = start || cancel;
                return;
            }
            Dialog::Export(kind) => {
                let (apply, cancel) = crate::export_ui::body(ui, app, &t, kind);
                export_now = apply;
                close = apply || cancel;
                return;
            }
            Dialog::Marks(kind) => {
                ui.set_width(720.0);
                let (apply, cancel) = crate::marks_ui::body(ui, app, &t, kind);
                marks_now = apply;
                close = apply || cancel;
                return;
            }
            Dialog::PageBoxes => {
                let (apply, cancel) = crate::pageboxes::body(ui, app, &t);
                boxes_now = apply;
                close = apply || cancel;
                return;
            }
            Dialog::Protect => {
                let (apply, cancel) = crate::protect::body(ui, app, &t);
                protect_now = apply;
                close = apply || cancel;
                return;
            }
            Dialog::NumberPages => {
                use printcraft_engine::LabelStyle as L;
                ui.label(egui::RichText::new(crate::i18n::text("ui.number_pages")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                let Some((_, id)) = app.active_ids() else { return };
                let n = app.session.get(id).map(|d| d.info.pages.len()).unwrap_or(1).max(1);
                let d = &mut app.number_draft;
                d.to = d.to.clamp(1, n);
                d.from = d.from.clamp(1, d.to);
                // Editable values get a visible border (the dialog and field fills are alike).
                let boxed = |ui: &mut egui::Ui, add: &mut dyn FnMut(&mut egui::Ui) -> egui::Response| {
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(1.0, t.border))
                        .corner_radius(egui::CornerRadius::same(5))
                        .inner_margin(egui::Margin::symmetric(4, 1))
                        .show(ui, |ui| add(ui))
                        .inner
                };
                egui::Grid::new("number_pages").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label(crate::i18n::text("ui.pages"));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut d.from).range(1..=n));
                        ui.label(crate::i18n::text("ui.to_663ea1"));
                        ui.add(egui::DragValue::new(&mut d.to).range(1..=n));
                        ui.label(egui::RichText::new(crate::msg!(of_value_a2eabd, n = n)).color(t.text_muted));
                    });
                    ui.end_row();
                    ui.label(crate::i18n::text("ui.style"));
                    let styles = [
                        (L::Decimal, "1, 2, 3"),
                        (L::LowerRoman, "i, ii, iii"),
                        (L::UpperRoman, "I, II, III"),
                        (L::LowerAlpha, "a, b, c"),
                        (L::UpperAlpha, "A, B, C"),
                        (L::None, crate::i18n::text("ui.none_prefix_only")),
                    ];
                    let current = styles.iter().find(|(s, _)| *s == d.style).map_or("1, 2, 3", |(_, l)| *l);
                    egui::ComboBox::from_id_salt("label_style").selected_text(current).show_ui(ui, |ui| {
                        for (s, l) in styles {
                            ui.selectable_value(&mut d.style, s, l);
                        }
                    });
                    ui.end_row();
                    let l = ui.label(crate::i18n::text("ui.prefix"));
                    boxed(ui, &mut |ui| ui.add(egui::TextEdit::singleline(&mut d.prefix).desired_width(160.0).frame(egui::Frame::NONE)))
                        .labelled_by(l.id);
                    ui.end_row();
                    ui.label(crate::i18n::text("ui.start"));
                    ui.add(egui::DragValue::new(&mut d.start).range(1..=99_999));
                    ui.end_row();
                });
                d.to = d.to.max(d.from);
                let label = |k: u32| format!("{}{}", d.prefix, d.style.format(k));
                ui.add_space(8.0);
                let preview = if d.from == d.to {
                    label(d.start)
                } else {
                    format!("{}, {} … {}", label(d.start), label(d.start + 1), label(d.start + (d.to - d.from) as u32))
                };
                ui.label(egui::RichText::new(crate::msg!(labels_value_later_pages_keep_their_labels, preview = preview)).color(t.text_muted));
                number_now = Some(Edit::NumberPages { from: d.from - 1, to: d.to - 1, style: d.style, prefix: d.prefix.clone(), first: d.start });
            }
            Dialog::Recovery => {
                ui.horizontal(|ui| {
                    ui.add(crate::icons::image("clock-3", 22.0, t.accent));
                    ui.label(egui::RichText::new(crate::i18n::text("ui.recover_unsaved_documents")).font(theme::semibold(18.0)));
                });
                ui.add_space(6.0);
                ui.label(crate::i18n::text("ui.printcraft_didn_t_shut_down_normally_these_documents_had_changes_that_were_autosaved"));
                ui.add_space(8.0);
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                egui::Grid::new("recoverable").num_columns(2).spacing([18.0, 6.0]).show(ui, |ui| {
                    for m in &app.recoverable {
                        ui.label(egui::RichText::new(&m.name).font(theme::medium(13.0)));
                        let mins = now.saturating_sub(m.saved_at) / 60;
                        let when = match mins {
                            0 => crate::i18n::text("ui.just_now").to_string(),
                            1..=59 => crate::msg!(value_min_ago, mins = mins),
                            _ => crate::msg!(value_h_ago, mins / 60),
                        };
                        let lock = if m.encrypted { crate::i18n::text("ui.password_protected") } else { "" };
                        ui.label(egui::RichText::new(format!("{when}{lock}")).color(t.text_muted));
                        ui.end_row();
                    }
                });
            }
            Dialog::Shortcuts => {
                ui.label(egui::RichText::new(crate::i18n::text("ui.keyboard_shortcuts")).font(theme::semibold(18.0)));
                ui.add_space(8.0);
                let mac = cfg!(target_os = "macos") || cfg!(target_arch = "wasm32");
                // Registered commands first (always in sync with the real bindings), then the
                // keys the document view handles itself.
                let mut rows: Vec<(String, String)> = printcraft_engine::commands::COMMANDS
                    .iter()
                    .filter_map(|c| c.shortcut.map(|k| (k.label(mac), app.language.tr(c.label).trim_end_matches('…').to_string())))
                    .collect();
                for (k, v) in [
                    ("⌘G / ⇧⌘G", crate::i18n::text("ui.next_previous_match")),
                    ("⌘C", crate::i18n::text("ui.copy_selected_text")),
                    (crate::i18n::text("ui.double_click"), crate::i18n::text("ui.select_a_word")),
                    ("Esc", crate::i18n::text("ui.clear_selection_close_find")),
                    ("⌘1", crate::i18n::text("ui.actual_size")),
                    ("⌘0", crate::i18n::text("ui.zoom_to_page_level")),
                    ("⌘2", crate::i18n::text("ui.fit_to_width")),
                    ("⌘3", crate::i18n::text("ui.fit_visible")),
                    ("⌘+ / ⌘−", crate::i18n::text("ui.zoom_in_out_also_pinch_or_scroll")),
                    ("⇧⌘+ / ⇧⌘−", crate::i18n::text("ui.rotate_view")),
                    ("Home / End", crate::i18n::text("ui.first_last_page")),
                    ("⌘← / ⌘→", crate::i18n::text("ui.previous_next_page")),
                    (crate::i18n::text("ui.delete"), crate::i18n::text("ui.delete_selected_pages_organize")),
                    ("⌘A", crate::i18n::text("ui.select_all_pages_organize")),
                ] {
                    rows.push((k.to_string(), v.to_string()));
                }
                egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                    egui::Grid::new("keys").num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
                        for (k, v) in &rows {
                            ui.label(egui::RichText::new(k).font(egui::FontId::monospace(12.5)));
                            ui.label(crate::i18n::current().tr(v));
                            ui.end_row();
                        }
                    });
                });
            }
            Dialog::About => {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::PRODUCT_NAME).font(theme::semibold(20.0)));
                        ui.label(crate::msg!(version_value, crate::i18n::PRODUCT_VERSION));
                    });
                });
                ui.add_space(6.0);
                ui.label(crate::i18n::text("ui.a_clean_room_open_source_pdf_application_written_in_rust_mit_or_apache_2_0"));
                ui.label(
                    egui::RichText::new(crate::i18n::text(
                        "ui.rendering_hayro_bootstrap_ui_egui_icons_lucide_isc_fonts_inter_jetbrains_mono_dancing_script_ofl",
                    ))
                    .color(t.text_muted)
                    .small(),
                );
                ui.label(crate::i18n::text(
                    "ui.based_on_printcraft_copyright_the_upstream_contributors_see_license_notice_and_attribution_for_third_party_acknowledgements",
                ));
            }
        }
        ui.add_space(12.0);
        let changed = draft_changes(app).is_some_and(|c| !c.is_empty());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if dialog == Dialog::Recovery {
                if widgets::pill_button(ui, crate::i18n::text("ui.recover"), true).clicked() {
                    recover = Some(true);
                    close = true;
                }
                if widgets::pill_button(ui, crate::i18n::text("ui.discard"), false).clicked() {
                    recover = Some(false);
                    close = true;
                }
            } else if dialog == Dialog::NumberPages {
                if widgets::pill_button(ui, crate::i18n::text("ui.ok"), true).clicked() {
                    apply_number = true;
                    close = true;
                }
                if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                    close = true;
                }
            } else if dialog == Dialog::Split {
                if ui.add_enabled_ui(split_ready.is_some(), |ui| widgets::pill_button(ui, crate::i18n::text("ui.split"), true)).inner.clicked() {
                    split_now = split_ready.clone();
                    close = true;
                }
                if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                    close = true;
                }
            } else if changed {
                if widgets::pill_button(ui, crate::i18n::text("ui.ok"), true).clicked() {
                    apply = true;
                    close = true;
                }
                if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                    close = true;
                }
            } else if widgets::pill_button(ui, crate::i18n::text("ui.close"), true).clicked() {
                close = true;
            }
        });
    });
    if let Some(yes) = recover {
        let keys: Vec<String> = app.recoverable.iter().map(|m| m.key.clone()).collect();
        if yes {
            app.recover(&keys);
        } else {
            app.discard_recovered(&keys);
        }
    }
    if replace_now && let Some(d) = app.replace_draft.take() {
        let n = d.to - d.from + 1;
        app.apply_edit(Edit::ReplacePages {
            pages: (d.from - 1..d.to).collect(),
            name: d.name.clone(),
            bytes: d.bytes.clone(),
            src_pages: (d.src_from - 1..d.src_from - 1 + n).collect(),
        });
    }
    if print_go {
        app.print_now();
    }
    if revert_now {
        app.revert_active();
    }
    if summarize_now {
        app.summarize_comments();
    }
    if optimize_now {
        app.optimize_with_draft();
    }
    if let Some(e) = duplicate_now {
        app.duplicate_draft = None;
        app.apply_edit(e);
    }
    if extract_now {
        app.extract_selection();
    }
    if let Some(e) = link_now {
        app.apply_edit(e);
        app.link_draft = None;
    }
    if rotate_now {
        app.rotate_with_draft();
    }
    match redact_now {
        Some(Dialog::RedactPages) => app.redact_pages(),
        Some(Dialog::RedactSearch) => {
            let n = app.redact_search();
            app.redact_search.found = Some(n);
        }
        Some(Dialog::RemoveHidden) => {
            let which: Vec<printcraft_engine::Hidden> = app.hidden_draft.found.iter().filter(|f| f.2 && f.1 > 0).map(|f| f.0).collect();
            let n: usize = app.hidden_draft.found.iter().filter(|f| f.2).map(|f| f.1).sum();
            if app.apply_edit(Edit::RemoveHidden { which }) {
                app.notify(crate::msg!(removed_value_hidden_item_value_save_to_remove_them_from_the_file, crate::i18n::plural_suffix(n), n = n));
            }
        }
        Some(Dialog::Sanitize) if app.apply_edit(Edit::Sanitize) => {
            app.notify(crate::i18n::text("ui.document_sanitized_save_to_finish_saving_rewrites_the_whole_file"));
        }
        Some(Dialog::RedactApply) => {
            let marks = app.active_ids().and_then(|(_, id)| app.session.get(id)).map_or(0, |d| d.redaction_marks());
            if app.apply_edit(Edit::ApplyRedactions { pages: None }) {
                app.notify(crate::msg!(
                    applied_value_redaction_mark_value_save_to_remove_the_content_from_the_file,
                    crate::i18n::plural_suffix(marks),
                    marks = marks
                ));
            }
        }
        _ => {}
    }
    if field_props_now
        && let Some(d) = app.field_props.take()
        && let Some(props) = d.props()
        && app.apply_edit(Edit::SetFieldProps { name: d.field.clone(), props: Box::new(props) })
        && let Some(i) = app.active
    {
        // Keep the (possibly renamed) field selected.
        let prefix = d.field.rsplit_once('.').map(|(p, _)| format!("{p}.")).unwrap_or_default();
        app.views[i].prepare.selected = Some((format!("{prefix}{}", d.name.trim()), d.widget));
    }
    if props_now && let Some(d) = app.comment_props.take() {
        let mut edits = crate::comment_props::edits(&d);
        match edits.len() {
            0 => {}
            1 => {
                app.apply_edit(edits.remove(0));
            }
            _ => {
                app.apply_edit(Edit::Batch { label: "Change comment properties".into(), edits });
            }
        }
    }
    if export_now && let Dialog::Export(kind) = dialog {
        app.start_export(kind);
    }
    if marks_now && let (Dialog::Marks(kind), Some((_, id))) = (dialog, app.active_ids()) {
        let count = app.session.get(id).map(|d| d.info.pages.len()).unwrap_or(0);
        let edit = crate::marks_ui::edit(&app.marks_draft, kind, count);
        app.apply_edit(edit);
    }
    if boxes_now && let Some((i, id)) = app.active_ids() {
        let count = app.session.get(id).map(|d| d.info.pages.len()).unwrap_or(0);
        let edit = app.boxes_draft.edit(app.views[i].current, count);
        app.apply_edit(edit);
        app.boxes_draft.seeded = None;
    }
    if protect_now && app.apply_edit(app.protect_draft.edit()) {
        app.protect_draft = Default::default();
        app.notify(crate::i18n::text("ui.password_protection_will_be_applied_when_you_save"));
    }
    if apply_number && let Some(edit) = number_now {
        app.apply_edit(edit);
    }
    if let Some(by) = split_now {
        app.split_active(&by);
    }
    if apply && let Some(edits) = draft_changes(app) {
        app.apply_edit(Edit::Batch { label: "Change document properties".into(), edits });
    }
    // Protect / Remove security replace the Properties dialog.
    let replaces = link_command.is_some_and(|c| c.starts_with("protect.")) || open_revision.is_some();
    if modal.should_close() || close || replaces {
        app.dialog = None;
        app.props_draft = None;
        app.view_draft = None;
    } else {
        app.dialog = Some(next);
    }
    if let Some(cmd) = link_command {
        app.execute(cmd);
    }
    if let Some(n) = open_revision {
        app.open_revision(n);
    }
    if alt_now {
        app.save_alt_text();
    }
    if stamp_now {
        app.save_custom_stamp();
    }
    if combine_now {
        app.combine_staged();
    }
    if ocr_now {
        app.start_ocr();
    }
    if compare_now {
        app.run_compare();
    }
    if a11y_now {
        app.a11y_skipped.clear();
        app.run_accessibility_check();
    }
}

/// Info edits needed to make the document match the Description draft.
fn draft_changes(app: &PrintCraftApp) -> Option<Vec<Edit>> {
    let (id, draft) = app.props_draft.as_ref()?;
    let doc = app.session.get(*id)?;
    let mut edits: Vec<Edit> = INFO_KEYS
        .iter()
        .zip(draft.iter())
        .filter(|(k, v)| doc.info_value(k).unwrap_or_default().trim() != v.trim())
        .map(|(k, v)| Edit::SetInfo { key: (*k).to_string(), value: v.clone() })
        .collect();
    if let Some((vid, v)) = &app.view_draft
        && vid == id
        && *v != doc.initial_view()
    {
        edits.push(Edit::SetInitialView(Box::new(v.clone())));
    }
    Some(edits)
}

/// The "Save changes?" prompt's keys (issue #8), read before anything else can take them: Enter
/// saves (the default button) and Escape cancels; Don't save takes ⌘D / Ctrl+D, the macOS
/// convention, or Alt+D / Alt+N, the mnemonics Windows and Linux desktops use for it.
pub(crate) fn save_prompt_key(ctx: &egui::Context) -> Option<Option<bool>> {
    use egui::{Key, Modifiers};
    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::Enter) {
            Some(Some(true))
        } else if i.consume_key(Modifiers::NONE, Key::Escape) {
            Some(None)
        } else if [(Modifiers::COMMAND, Key::D), (Modifiers::ALT, Key::D), (Modifiers::ALT, Key::N)].into_iter().any(|(m, k)| i.consume_key(m, k)) {
            Some(Some(false))
        } else {
            None
        }
    })
}

/// "Save changes?" when closing a tab or quitting with unsaved edits.
fn save_prompt(app: &mut PrintCraftApp, ctx: &egui::Context) {
    let Some(req) = app.close_request else { return };
    let index = match req {
        CloseRequest::Tab(i) => Some(i),
        CloseRequest::Quit | CloseRequest::All => app.first_dirty(),
    };
    let Some(name) = index.and_then(|i| app.views.get(i)).and_then(|v| app.session.get(v.id)).map(|d| d.name.clone()) else {
        // Nothing left to ask about (tab already gone or no dirty documents).
        app.resolve_close(ctx, Some(false));
        return;
    };
    let t = Tokens::get(ctx);
    let mut choice: Option<Option<bool>> = None;
    let modal = egui::Modal::new(egui::Id::new("save_prompt")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.horizontal(|ui| {
            ui.add(crate::icons::image("save", 22.0, t.accent));
            ui.label(egui::RichText::new(crate::msg!(save_changes_to_value_before_closing, name = name)).font(theme::semibold(16.0)));
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new(crate::i18n::text("ui.your_changes_will_be_lost_if_you_don_t_save_them")).color(t.text_muted));
        ui.add_space(14.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, crate::i18n::text("ui.save"), true).clicked() {
                choice = Some(Some(true));
            }
            if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                choice = Some(None);
            }
            ui.add_space(24.0);
            if widgets::pill_button(ui, crate::i18n::text("ui.don_t_save"), false).clicked() {
                choice = Some(Some(false));
            }
        });
    });
    if choice.is_none() && modal.should_close() {
        choice = Some(None);
    }
    if let Some(c) = choice {
        app.resolve_close(ctx, c);
    }
}

fn yes(b: bool) -> String {
    if b { crate::i18n::text("ui.yes").into() } else { crate::i18n::text("ui.no").into() }
}

/// Password prompt for encrypted documents (Acrobat: "Password" dialog on open).
fn password(app: &mut PrintCraftApp, ctx: &egui::Context) {
    let Some(prompt) = app.password_prompt.as_mut() else { return };
    let t = Tokens::get(ctx);
    let mut submit = false;
    let mut cancel = false;
    let modal = egui::Modal::new(egui::Id::new("password")).show(ctx, |ui| {
        ui.set_width(400.0);
        ui.horizontal(|ui| {
            ui.add(crate::icons::image("lock", 22.0, t.accent));
            ui.label(egui::RichText::new(crate::i18n::text("ui.password_required")).font(theme::semibold(17.0)));
        });
        ui.add_space(6.0);
        ui.label(crate::msg!(value_is_protected_enter_a_password_to_open_it, prompt.name));
        ui.add_space(8.0);
        let r = ui.add(
            egui::TextEdit::singleline(&mut prompt.input).password(true).hint_text(crate::i18n::text("ui.password")).desired_width(f32::INFINITY),
        );
        // Enter submits. The field keeps focus (we request it every frame), so check the key
        // while it is focused as well as on the frame focus is lost.
        if (r.has_focus() || r.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit = true;
        }
        r.request_focus();
        if let Some(e) = &prompt.error {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(e).color(egui::Color32::from_rgb(0xD1, 0x3B, 0x3B)));
        }
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, crate::i18n::text("ui.open"), true).clicked() {
                submit = true;
            }
            if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
                cancel = true;
            }
        });
    });
    if submit {
        let pw = prompt.input.clone();
        app.submit_password(Some(pw));
    } else if cancel || modal.should_close() {
        app.submit_password(None);
    }
}
