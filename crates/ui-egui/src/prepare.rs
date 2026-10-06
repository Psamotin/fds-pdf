//! Prepare a form (Acrobat's Prepare Form, execution plan M6.6): add fields with the field
//! tools (click for the default size, or drag a rectangle), select, move and resize them, see
//! their name tags, delete them, and edit them in Field Properties.
//!
//! While the Prepare a form panel is open the page shows every field with a light-blue fill and
//! its name; clicks select fields instead of filling them in, as in Acrobat.

use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, vec2};
use printcraft_engine::form_scripts::{CalcOp, Calculate, DATE_PRESETS, Format, TIME_PRESETS, Validate, format_value};
use printcraft_engine::{BorderStyle, Edit, FieldFont, FieldLook, FieldProps, FormField, FormFieldKind, NewField};
use printcraft_render::DocInfo;

use crate::canvas::{DocView, PageXform};
use crate::theme;

const SELECT_BLUE: Color32 = Color32::from_rgb(0x14, 0x73, 0xE6);
/// Acrobat's field fill in Prepare Form (light blue).
const FIELD_FILL: Color32 = Color32::from_rgba_premultiplied(0x1B, 0x34, 0x54, 0x59);

/// The Add form components tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldTool {
    Text,
    CheckBox,
    Radio,
    Combo,
    List,
    Button,
    Image,
    Date,
    Signature,
}

pub const FIELD_TOOLS: [FieldTool; 9] = [
    FieldTool::Text,
    FieldTool::CheckBox,
    FieldTool::Radio,
    FieldTool::Combo,
    FieldTool::List,
    FieldTool::Button,
    FieldTool::Image,
    FieldTool::Date,
    FieldTool::Signature,
];

impl FieldTool {
    pub fn command(self) -> &'static str {
        match self {
            FieldTool::Text => "form.add.text",
            FieldTool::CheckBox => "form.add.checkbox",
            FieldTool::Radio => "form.add.radio",
            FieldTool::Combo => "form.add.combo",
            FieldTool::List => "form.add.list",
            FieldTool::Button => "form.add.button",
            FieldTool::Image => "form.add.image",
            FieldTool::Date => "form.add.date",
            FieldTool::Signature => "form.add.signature",
        }
    }

    pub fn from_command(id: &str) -> Option<Self> {
        FIELD_TOOLS.into_iter().find(|t| t.command() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            FieldTool::Text => crate::i18n::text("ui.text_field"),
            FieldTool::CheckBox => crate::i18n::text("ui.checkbox"),
            FieldTool::Radio => crate::i18n::text("ui.radio_button"),
            FieldTool::Combo => crate::i18n::text("ui.drop_down_list"),
            FieldTool::List => crate::i18n::text("ui.list_box"),
            FieldTool::Button => crate::i18n::text("ui.button"),
            FieldTool::Image => crate::i18n::text("ui.image_field"),
            FieldTool::Date => crate::i18n::text("ui.date_field"),
            FieldTool::Signature => crate::i18n::text("ui.digital_signature_b1e335"),
        }
    }

    /// The field a click or drag creates. Radio buttons dropped next to a selected radio group
    /// join it with the next free choice.
    pub fn new_field(self, join: Option<&FormField>) -> NewField {
        match self {
            FieldTool::Text => NewField::Text { multiline: false },
            FieldTool::CheckBox => NewField::CheckBox,
            FieldTool::Radio => match join.filter(|f| f.kind == FormFieldKind::Radio) {
                Some(g) => {
                    let taken: Vec<&str> = g.widgets.iter().filter_map(|w| w.on_state.as_deref()).collect();
                    let export =
                        (1..=u64::MAX).map(|i| format!("Choice{i}")).find(|c| !taken.contains(&c.as_str())).unwrap_or_else(|| "Choice".into());
                    NewField::Radio { group: Some(g.name.clone()), export }
                }
                None => NewField::Radio { group: None, export: "Choice1".into() },
            },
            FieldTool::Combo => NewField::Combo { options: Vec::new(), editable: false },
            FieldTool::List => NewField::List { options: Vec::new(), multi: false },
            FieldTool::Button => NewField::Button { caption: String::new() },
            FieldTool::Image => NewField::Image,
            FieldTool::Date => NewField::Date,
            FieldTool::Signature => NewField::Signature,
        }
    }

    /// Width and height in points of a field placed with a single click.
    pub fn default_size(self) -> (f64, f64) {
        match self {
            FieldTool::CheckBox | FieldTool::Radio => (14.0, 14.0),
            FieldTool::List => (144.0, 54.0),
            FieldTool::Button => (72.0, 22.0),
            FieldTool::Image => (144.0, 108.0),
            FieldTool::Signature => (180.0, 36.0),
            FieldTool::Date => (108.0, 22.0),
            FieldTool::Text | FieldTool::Combo => (144.0, 22.0),
        }
    }
}

/// What the pointer is doing to a field.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Grab {
    /// Drawing a new field from this screen point.
    Draw(Pos2),
    Move,
    /// Resizing by a corner: (moves left edge, moves top edge) in screen terms.
    Corner(bool, bool),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrepareView {
    /// The selected field and which of its widgets (the anchor of Align and Match Size).
    pub selected: Option<(String, usize)>,
    /// More fields selected with Shift- or ⌘-click.
    pub also: Vec<(String, usize)>,
    /// Select the field an Add field edit creates (the form's length before it).
    pub select_added: Option<usize>,
    grab: Option<(usize, Grab, Pos2)>,
}

fn screen_rect(xf: &PageXform, info: &DocInfo, page: usize, r: [f64; 4]) -> Rect {
    xf.user_rect(info, page, [r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32])
}

fn to_user(xf: &PageXform, info: &DocInfo, page: usize, p: Pos2) -> [f64; 2] {
    let (vx, vy) = xf.screen_to_view(p);
    let u = info.pages[page].view_to_user(vx, vy);
    [u[0] as f64, u[1] as f64]
}

/// A screen rect → a normalised user-space rect.
fn user_rect(xf: &PageXform, info: &DocInfo, page: usize, r: Rect) -> [f64; 4] {
    let (a, b) = (to_user(xf, info, page, r.min), to_user(xf, info, page, r.max));
    [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
}

fn handles(r: Rect) -> [(Pos2, bool, bool); 4] {
    [(r.left_top(), true, true), (r.right_top(), false, true), (r.left_bottom(), true, false), (r.right_bottom(), false, false)]
}

/// The rect while a move or resize is in progress.
fn dragged(r: Rect, grab: Grab, delta: egui::Vec2) -> Rect {
    match grab {
        Grab::Move => r.translate(delta),
        Grab::Corner(left, top) => {
            let mut r = r;
            if left {
                r.min.x += delta.x;
            } else {
                r.max.x += delta.x;
            }
            if top {
                r.min.y += delta.y;
            } else {
                r.max.y += delta.y;
            }
            Rect::from_two_pos(r.min, r.max)
        }
        Grab::Draw(_) => r,
    }
}

pub(crate) struct Outcome {
    /// The click or drag belonged to Prepare a form.
    pub consumed: bool,
    /// Open Field Properties for the selected field.
    pub properties: bool,
    /// A field was placed: go back to the Select tool.
    pub placed: bool,
}

/// Pointer input on one page while preparing a form.
#[allow(clippy::too_many_arguments)]
pub(crate) fn page_input(
    ui: &egui::Ui,
    resp: &egui::Response,
    xf: &PageXform,
    page: usize,
    info: &DocInfo,
    form: &[FormField],
    tool: Option<FieldTool>,
    allowed: bool,
    view: &mut DocView,
) -> Outcome {
    let mut out = Outcome { consumed: false, properties: false, placed: false };
    let pointer = ui.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));
    let prep = &mut view.prepare;

    // A grab in progress on this page.
    if let Some((gp, grab, start)) = prep.grab
        && gp == page
    {
        out.consumed = true;
        if resp.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
            prep.grab = None;
            let end = pointer.unwrap_or(start);
            match grab {
                Grab::Draw(from) => {
                    let Some(tool) = tool else { return out };
                    let r = Rect::from_two_pos(from, end).intersect(xf.rect);
                    let rect = user_rect(xf, info, page, r);
                    let rect = if r.width() < 4.0 || r.height() < 4.0 {
                        // A click: the default size, top-left at the pointer.
                        let at = to_user(xf, info, page, from);
                        let (w, h) = tool.default_size();
                        [at[0], at[1] - h, at[0] + w, at[1]]
                    } else {
                        rect
                    };
                    let join = prep.selected.as_ref().and_then(|(n, _)| form.iter().find(|f| &f.name == n));
                    prep.select_added = Some(form.len());
                    view.pending_edit = Some(Edit::AddField { page, rect, kind: tool.new_field(join), name: None });
                    out.placed = true;
                }
                Grab::Move | Grab::Corner(..) => {
                    let Some((name, wi)) = prep.selected.clone() else { return out };
                    let Some(w) = form.iter().find(|f| f.name == name).and_then(|f| f.widgets.get(wi)) else { return out };
                    let delta = end - start;
                    if delta.length() >= 1.0 {
                        let r = dragged(screen_rect(xf, info, page, w.rect), grab, delta);
                        let rect = user_rect(xf, info, page, r);
                        view.pending_edit =
                            Some(Edit::SetFieldProps { name, props: Box::new(FieldProps { rect: Some((wi, rect)), ..Default::default() }) });
                    }
                }
            }
        }
        return out;
    }

    let Some(p) = pointer.filter(|p| xf.rect.contains(*p)) else { return out };
    if !allowed {
        return out;
    }
    if let Some(tool) = tool {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        if resp.drag_started() || resp.clicked() {
            let origin = ui.input(|i| i.pointer.press_origin()).filter(|o| xf.rect.contains(*o)).unwrap_or(p);
            prep.grab = Some((page, Grab::Draw(origin), origin));
            out.consumed = true;
            if resp.clicked() {
                // A click without a drag: finish at once.
                prep.grab = None;
                let at = to_user(xf, info, page, p);
                let (w, h) = tool.default_size();
                let join = prep.selected.as_ref().and_then(|(n, _)| form.iter().find(|f| &f.name == n));
                prep.select_added = Some(form.len());
                view.pending_edit = Some(Edit::AddField { page, rect: [at[0], at[1] - h, at[0] + w, at[1]], kind: tool.new_field(join), name: None });
                out.placed = true;
            }
        }
        return out;
    }

    // Right-click a field: select it (the context menu acts on it).
    if resp.secondary_clicked()
        && let Some((f, wi)) = form.iter().find_map(|f| {
            f.widgets.iter().enumerate().find(|(_, w)| w.page == Some(page) && screen_rect(xf, info, page, w.rect).contains(p)).map(|(i, _)| (f, i))
        })
    {
        let key = (f.name.clone(), wi);
        if let Some(i) = prep.also.iter().position(|k| *k == key) {
            // Part of the selection: it becomes the anchor the others line up with.
            prep.also.remove(i);
            if let Some(old) = prep.selected.replace(key) {
                prep.also.push(old);
            }
        } else if prep.selected.as_ref() != Some(&key) {
            prep.selected = Some(key);
            prep.also.clear();
        }
    }
    // The Select tool: corner handles of the selection, then fields.
    let selected_rect = prep
        .selected
        .as_ref()
        .and_then(|(n, wi)| form.iter().find(|f| &f.name == n)?.widgets.get(*wi).filter(|w| w.page == Some(page)))
        .map(|w| screen_rect(xf, info, page, w.rect));
    let corner = selected_rect.and_then(|r| handles(r).into_iter().find(|(c, ..)| c.distance(p) <= 6.0));
    let hit = form.iter().find_map(|f| {
        f.widgets.iter().enumerate().find(|(_, w)| w.page == Some(page) && screen_rect(xf, info, page, w.rect).contains(p)).map(|(i, _)| (f, i))
    });
    if let Some((_, left, top)) = corner {
        ui.ctx().set_cursor_icon(if left == top { egui::CursorIcon::ResizeNwSe } else { egui::CursorIcon::ResizeNeSw });
    } else if hit.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
    }
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
        if let Some((_, left, top)) = selected_rect.and_then(|r| handles(r).into_iter().find(|(c, ..)| c.distance(origin) <= 6.0)) {
            prep.grab = Some((page, Grab::Corner(left, top), origin));
            out.consumed = true;
        } else if let Some((f, wi)) = form.iter().find_map(|f| {
            f.widgets
                .iter()
                .enumerate()
                .find(|(_, w)| w.page == Some(page) && screen_rect(xf, info, page, w.rect).contains(origin))
                .map(|(i, _)| (f, i))
        }) {
            prep.selected = Some((f.name.clone(), wi));
            prep.grab = Some((page, Grab::Move, origin));
            out.consumed = true;
        }
        return out;
    }
    if resp.double_clicked() && hit.is_some() {
        out.consumed = true;
        out.properties = true;
        return out;
    }
    if resp.clicked() {
        let extend = ui.input(|i| i.modifiers.shift || i.modifiers.command);
        match hit {
            // Shift/⌘-click adds or removes a field from the selection.
            Some((f, wi)) if extend && prep.selected.is_some() => {
                let key = (f.name.clone(), wi);
                if prep.selected.as_ref() != Some(&key) {
                    if let Some(i) = prep.also.iter().position(|k| *k == key) {
                        prep.also.remove(i);
                    } else {
                        prep.also.push(key);
                    }
                }
                out.consumed = true;
            }
            Some((f, wi)) => {
                prep.selected = Some((f.name.clone(), wi));
                prep.also.clear();
                out.consumed = true;
            }
            None if corner.is_none() => {
                prep.selected = None;
                prep.also.clear();
            }
            None => out.consumed = true,
        }
    }
    out.consumed |= hit.is_some() && resp.is_pointer_button_down_on();
    out
}

/// Align, Center, Distribute and Set Fields to Same Size (Prepare a form, several fields
/// selected).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrange {
    AlignLeft,
    AlignRight,
    AlignTop,
    AlignBottom,
    /// Centres on one vertical line (the anchor's).
    AlignCenterH,
    /// Centres on one horizontal line.
    AlignCenterV,
    DistributeH,
    DistributeV,
    SameWidth,
    SameHeight,
    SameSize,
}

impl Arrange {
    pub fn label(self) -> &'static str {
        match self {
            Arrange::AlignLeft | Arrange::AlignRight | Arrange::AlignTop | Arrange::AlignBottom | Arrange::AlignCenterH | Arrange::AlignCenterV => {
                "Align fields"
            }
            Arrange::DistributeH | Arrange::DistributeV => "Distribute fields",
            Arrange::SameWidth | Arrange::SameHeight | Arrange::SameSize => "Match field sizes",
        }
    }
}

/// The edits that arrange the selected widgets (user space; the anchor stays put).
pub fn arrange(form: &[FormField], anchor: &(String, usize), others: &[(String, usize)], op: Arrange) -> Option<Edit> {
    let rect_of = |k: &(String, usize)| form.iter().find(|f| f.name == k.0).and_then(|f| f.widgets.get(k.1)).map(|w| w.rect);
    let a = rect_of(anchor)?;
    let mut items: Vec<((String, usize), [f64; 4])> = others.iter().filter_map(|k| rect_of(k).map(|r| (k.clone(), r))).collect();
    if items.is_empty() {
        return None;
    }
    let (aw, ah) = (a[2] - a[0], a[3] - a[1]);
    let mut moved: Vec<((String, usize), [f64; 4])> = Vec::new();
    match op {
        Arrange::DistributeH | Arrange::DistributeV => {
            // Even gaps between all selected fields (the outermost stay).
            items.push((anchor.clone(), a));
            if items.len() < 3 {
                return None;
            }
            let horiz = op == Arrange::DistributeH;
            items.sort_by(|x, y| if horiz { x.1[0].total_cmp(&y.1[0]) } else { y.1[3].total_cmp(&x.1[3]) });
            let size = |r: &[f64; 4]| if horiz { r[2] - r[0] } else { r[3] - r[1] };
            let (first, last) = (items[0].1, items[items.len() - 1].1);
            let span = if horiz { last[2] - first[0] } else { first[3] - last[1] };
            let total: f64 = items.iter().map(|(_, r)| size(r)).sum();
            let gap = (span - total) / (items.len() - 1) as f64;
            let mut at = if horiz { first[0] } else { first[3] };
            for (k, r) in &items {
                let s = size(r);
                let nr = if horiz { [at, r[1], at + s, r[3]] } else { [r[0], at - s, r[2], at] };
                at = if horiz { at + s + gap } else { at - s - gap };
                moved.push((k.clone(), nr));
            }
        }
        _ => {
            for (k, r) in items {
                let (w, h) = (r[2] - r[0], r[3] - r[1]);
                let nr = match op {
                    Arrange::AlignLeft => [a[0], r[1], a[0] + w, r[3]],
                    Arrange::AlignRight => [a[2] - w, r[1], a[2], r[3]],
                    Arrange::AlignTop => [r[0], a[3] - h, r[2], a[3]],
                    Arrange::AlignBottom => [r[0], a[1], r[2], a[1] + h],
                    Arrange::AlignCenterH => {
                        let cx = (a[0] + a[2]) / 2.0;
                        [cx - w / 2.0, r[1], cx + w / 2.0, r[3]]
                    }
                    Arrange::AlignCenterV => {
                        let cy = (a[1] + a[3]) / 2.0;
                        [r[0], cy - h / 2.0, r[2], cy + h / 2.0]
                    }
                    Arrange::SameWidth => [r[0], r[1], r[0] + aw, r[3]],
                    Arrange::SameHeight => [r[0], r[3] - ah, r[2], r[3]],
                    _ => [r[0], r[3] - ah, r[0] + aw, r[3]],
                };
                moved.push((k, nr));
            }
        }
    }
    let edits: Vec<Edit> = moved
        .into_iter()
        .filter(|(k, nr)| rect_of(k).is_some_and(|r| r.iter().zip(nr).any(|(x, y)| (x - y).abs() > 1e-6)))
        .map(|((name, wi), rect)| Edit::SetFieldProps { name, props: Box::new(FieldProps { rect: Some((wi, rect)), ..Default::default() }) })
        .collect();
    (!edits.is_empty()).then(|| Edit::Batch { label: op.label().into(), edits })
}

/// Fields with their light-blue fill and name tags, the selection with its handles, and the
/// rectangle being drawn or dragged.
pub(crate) fn paint_page(ui: &egui::Ui, painter: &egui::Painter, xf: &PageXform, page: usize, info: &DocInfo, form: &[FormField], view: &DocView) {
    let prep = &view.prepare;
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let grab = prep.grab.filter(|(gp, ..)| *gp == page);
    let zoom = (xf.rect.width() / xf.pw.max(1.0)).clamp(0.5, 3.0);
    for f in form {
        for (wi, w) in f.widgets.iter().enumerate().filter(|(_, w)| w.page == Some(page)) {
            let mut r = screen_rect(xf, info, page, w.rect);
            let selected = prep.selected.as_ref().is_some_and(|(n, i)| n == &f.name && *i == wi);
            let also = prep.also.iter().any(|(n, i)| n == &f.name && *i == wi);
            if selected && let (Some((_, g, start)), Some(p)) = (grab, pointer) {
                r = dragged(r, g, p - start);
            }
            painter.rect_filled(r, CornerRadius::ZERO, FIELD_FILL);
            painter.rect_stroke(
                r,
                CornerRadius::ZERO,
                Stroke::new(if also { 2.0 } else { 1.0 }, if selected || also { SELECT_BLUE } else { Color32::from_gray(90) }),
                egui::StrokeKind::Inside,
            );
            // The name tag: white text on black, clipped to the field.
            let label = f.name.rsplit('.').next().unwrap_or(&f.name);
            let font = theme::regular((9.0 * zoom).clamp(7.0, 13.0));
            let galley = painter.layout_no_wrap(label.to_string(), font, Color32::WHITE);
            let size = galley.size() + vec2(6.0, 2.0);
            let tag = Rect::from_center_size(r.center(), size);
            let clip = painter.clip_rect().intersect(r);
            let p2 = painter.with_clip_rect(clip);
            p2.rect_filled(tag, CornerRadius::same(1), if selected { SELECT_BLUE } else { Color32::BLACK });
            p2.galley(tag.min + vec2(3.0, 1.0), galley, Color32::WHITE);
            if selected {
                painter.rect_stroke(r.expand(1.0), CornerRadius::ZERO, Stroke::new(2.0, SELECT_BLUE), egui::StrokeKind::Outside);
                for (c, ..) in handles(r) {
                    painter.circle(c, 3.5, Color32::WHITE, Stroke::new(1.5, SELECT_BLUE));
                }
            }
        }
    }
    if let (Some((_, Grab::Draw(from), _)), Some(p)) = (grab, pointer) {
        let r = Rect::from_two_pos(from, p).intersect(xf.rect);
        painter.rect_filled(r, CornerRadius::ZERO, FIELD_FILL);
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, SELECT_BLUE), egui::StrokeKind::Inside);
    }
}

/// Delete removes the selected field; Escape clears the selection.
pub(crate) fn keys(ctx: &egui::Context, view: &mut DocView) {
    if view.prepare.selected.is_none() || ctx.egui_wants_keyboard_input() {
        return;
    }
    let (del, esc) = ctx.input(|i| (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace), i.key_pressed(egui::Key::Escape)));
    if del && let Some((name, _)) = view.prepare.selected.take() {
        view.pending_edit = Some(Edit::DeleteField { name });
    } else if esc {
        view.prepare.selected = None;
    }
}

/// After an Add field edit: select the new field (the form's last).
pub(crate) fn after_refresh(view: &mut DocView, form: &[FormField]) {
    if let Some(n) = view.prepare.select_added.take() {
        if form.len() > n
            && let Some(f) = form.last()
        {
            view.prepare.selected = Some((f.name.clone(), 0));
        } else if let Some((name, _)) = &view.prepare.selected
            && let Some(f) = form.iter().find(|f| &f.name == name)
        {
            // A radio button joined the selected group: select the new button.
            view.prepare.selected = Some((f.name.clone(), f.widgets.len().saturating_sub(1)));
        }
    }
    if let Some((name, wi)) = &view.prepare.selected
        && !form.iter().any(|f| &f.name == name && *wi < f.widgets.len())
    {
        view.prepare.selected = None;
    }
}

// ───────────────────────────────────────────────────────────────────────── Field Properties

impl crate::PrintCraftApp {
    /// Open Field Properties for a field of the active document.
    pub fn open_field_props(&mut self, name: &str, widget: usize) {
        let _locale = crate::i18n::scope(self.language);
        let Some((_, id)) = self.active_ids() else { return };
        let Some(f) = self.session.get(id).and_then(|d| d.form.iter().find(|f| f.name == name).cloned()) else { return };
        let mut d = FieldDraft::new(&f, widget);
        d.look = self.session.get(id).and_then(|doc| doc.field_look(name));
        if let Some(o) = d.original.as_mut() {
            o.look = d.look;
        }
        let others: Vec<String> =
            self.session.get(id).map(|doc| doc.form.iter().filter(|x| x.name != name).map(|x| x.name.clone()).collect()).unwrap_or_default();
        d.others = others.clone();
        d.actions = self.session.get(id).map(|doc| doc.field_actions(name)).unwrap_or_default();
        if let Some(o) = d.original.as_mut() {
            o.others = others;
            o.actions = d.actions.clone();
        }
        self.field_props = Some(d);
        self.dialog = Some(crate::Dialog::FieldProps);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldTab {
    General,
    Appearance,
    Position,
    Options,
    Format,
    Validate,
    Calculate,
    Actions,
}

/// Actions tab: the "Select Action" choices.
pub const ACTION_KINDS: [&str; 8] = [
    "Run a JavaScript",
    "Open a web link",
    "Reset a form",
    "Execute a menu item",
    "Go to a page view",
    "Show a field",
    "Hide a field",
    "Submit a form",
];

/// Actions tab: the action being added.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionDraft {
    pub trigger: printcraft_engine::FieldTrigger,
    pub kind: usize,
    /// The script, URL, field names (comma-separated), menu item or page number.
    pub text: String,
}

impl Default for ActionDraft {
    fn default() -> Self {
        ActionDraft { trigger: printcraft_engine::FieldTrigger::MouseUp, kind: 0, text: String::new() }
    }
}

impl ActionDraft {
    pub fn action(&self) -> Result<printcraft_engine::FieldAction, String> {
        use printcraft_engine::FieldAction as A;
        let t = self.text.trim();
        let names = || t.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect::<Vec<_>>();
        Ok(match self.kind {
            0 => A::JavaScript(self.text.clone()),
            1 if !t.is_empty() => A::Uri(t.to_string()),
            2 => A::Reset(names()),
            3 => A::Named(if t.is_empty() { "Print".into() } else { t.to_string() }),
            4 => A::GoTo(t.parse::<usize>().ok().filter(|p| *p >= 1).ok_or(crate::i18n::text("ui.enter_a_page_number"))? - 1),
            5 if !t.is_empty() => A::ShowHide { fields: names(), hide: false },
            6 if !t.is_empty() => A::ShowHide { fields: names(), hide: true },
            7 if !t.is_empty() => A::Submit(t.to_string()),
            1 | 7 => return Err(crate::i18n::text("ui.enter_a_url").into()),
            _ => return Err(crate::i18n::text("ui.enter_the_field_names").into()),
        })
    }
}

/// The Field Properties dialog's working copy.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDraft {
    pub field: String,
    pub kind: FormFieldKind,
    pub widget: usize,
    pub tab: FieldTab,
    pub name: String,
    pub tooltip: String,
    pub read_only: bool,
    pub required: bool,
    pub multiline: bool,
    pub limit: bool,
    pub max_len: usize,
    pub options: Vec<String>,
    pub new_option: String,
    /// `/Ff` as edited on the Options tab.
    pub flags: u32,
    /// Field Properties ▸ General ▸ Locked.
    pub locked: bool,
    /// `/Q`: 0 left, 1 centre, 2 right.
    pub quadding: i64,
    /// `/DV` (text and choices), or the default state (buttons: empty = off).
    pub default: String,
    /// Check boxes and radio buttons: this widget's "on" state (export value).
    pub on_state: String,
    /// 0 = auto.
    pub font_size: f64,
    /// Left, bottom, width, height in points.
    pub position: [f64; 4],
    pub look: Option<FieldLook>,
    pub format: Format,
    pub validate: Validate,
    pub calculate: Calculate,
    /// Every other field's name (the Calculate tab picks from them).
    pub others: Vec<String>,
    /// Actions tab: each trigger's action, and the one being added.
    pub actions: Vec<(printcraft_engine::FieldTrigger, printcraft_engine::FieldAction)>,
    pub new_action: ActionDraft,
    original: Box<Option<FieldDraft>>,
}

fn da_size(da: &str) -> f64 {
    let toks: Vec<&str> = da.split_whitespace().collect();
    toks.windows(2).rev().find(|w| w[1] == "Tf").and_then(|w| w[0].parse().ok()).unwrap_or(0.0)
}

impl FieldDraft {
    pub fn new(f: &FormField, widget: usize) -> Self {
        let r = f.widgets.get(widget).map(|w| w.rect).unwrap_or_default();
        let mut d = FieldDraft {
            field: f.name.clone(),
            kind: f.kind,
            widget,
            tab: FieldTab::General,
            name: f.name.rsplit('.').next().unwrap_or(&f.name).to_string(),
            tooltip: f.tooltip.clone().unwrap_or_default(),
            read_only: f.read_only(),
            required: f.has(printcraft_engine::field_flags::REQUIRED),
            multiline: f.has(printcraft_engine::field_flags::MULTILINE),
            limit: f.max_len.is_some(),
            max_len: f.max_len.unwrap_or(0),
            options: f.options.iter().map(|(_, d)| d.clone()).collect(),
            new_option: String::new(),
            flags: f.flags,
            locked: f.locked(),
            quadding: f.quadding,
            default: f.default.first().cloned().unwrap_or_default(),
            on_state: f.widgets.get(widget).and_then(|w| w.on_state.clone()).unwrap_or_default(),
            font_size: da_size(&f.da),
            position: [r[0], r[1], r[2] - r[0], r[3] - r[1]],
            look: None,
            format: f.actions.format.clone(),
            validate: f.actions.validate.clone(),
            calculate: f.actions.calculate.clone(),
            others: Vec::new(),
            actions: Vec::new(),
            new_action: ActionDraft::default(),
            original: Box::new(None),
        };
        d.original = Box::new(Some(d.clone()));
        d
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            FormFieldKind::Text => crate::i18n::text("ui.text_field_properties"),
            FormFieldKind::CheckBox => crate::i18n::text("ui.check_box_properties"),
            FormFieldKind::Radio => crate::i18n::text("ui.radio_button_properties"),
            FormFieldKind::Combo => crate::i18n::text("ui.dropdown_properties"),
            FormFieldKind::List => crate::i18n::text("ui.list_box_properties"),
            FormFieldKind::PushButton => crate::i18n::text("ui.button_properties"),
            FormFieldKind::Signature => crate::i18n::text("ui.digital_signature_properties"),
        }
    }

    fn tabs(&self) -> Vec<(FieldTab, &'static str)> {
        let mut t = vec![
            (FieldTab::General, crate::i18n::text("ui.general")),
            (FieldTab::Appearance, crate::i18n::text("ui.appearance")),
            (FieldTab::Position, crate::i18n::text("ui.position")),
        ];
        if !matches!(self.kind, FormFieldKind::PushButton | FormFieldKind::Signature) {
            t.push((FieldTab::Options, crate::i18n::text("ui.options_d0db8b")));
        }
        if matches!(self.kind, FormFieldKind::Text | FormFieldKind::Combo) {
            t.extend([
                (FieldTab::Format, crate::i18n::text("ui.format")),
                (FieldTab::Validate, crate::i18n::text("ui.validate")),
                (FieldTab::Calculate, crate::i18n::text("ui.calculate")),
            ]);
        }
        t.push((FieldTab::Actions, crate::i18n::text("ui.actions")));
        t
    }

    /// The properties that changed (`None` when nothing did).
    pub fn props(&self) -> Option<FieldProps> {
        let o = self.original.as_ref().as_ref()?;
        let ch = |a: bool, b: bool| (a != b).then_some(a);
        let p = FieldProps {
            name: (self.name.trim() != o.name).then(|| self.name.trim().to_string()),
            tooltip: (self.tooltip != o.tooltip).then(|| self.tooltip.clone()),
            read_only: ch(self.read_only, o.read_only),
            required: ch(self.required, o.required),
            multiline: ch(self.multiline, o.multiline),
            max_len: ((self.limit, self.max_len) != (o.limit, o.max_len)).then_some((self.limit && self.max_len > 0).then_some(self.max_len)),
            options: (self.options != o.options).then(|| self.options.clone()),
            font_size: ((self.font_size - o.font_size).abs() > 1e-6).then_some(self.font_size),
            rect: (self.position != o.position).then(|| {
                let [x, y, w, h] = self.position;
                (self.widget, [x, y, x + w.max(4.0), y + h.max(4.0)])
            }),
            look: (self.look != o.look).then_some(self.look).flatten(),
            format: (self.format != o.format).then(|| self.format.clone()),
            validate: (self.validate != o.validate).then(|| self.validate.clone()),
            calculate: (self.calculate != o.calculate).then(|| self.calculate.clone()),
            flags: OPTION_FLAGS.iter().filter(|b| (self.flags ^ o.flags) & **b != 0).map(|b| (*b, self.flags & b != 0)).collect(),
            quadding: (self.quadding != o.quadding).then_some(self.quadding),
            locked: ch(self.locked, o.locked),
            actions: (self.actions != o.actions).then(|| self.actions.clone()),
            default_value: (self.default != o.default).then(|| (!self.default.is_empty()).then(|| self.default.clone())),
        };
        (p != FieldProps::default()).then_some(p)
    }
}

/// The `/Ff` bits the Options tab edits.
const OPTION_FLAGS: [u32; 10] = {
    use printcraft_engine::field_flags as ff;
    [
        ff::DO_NOT_SCROLL,
        ff::RICH_TEXT,
        ff::PASSWORD,
        ff::FILE_SELECT,
        ff::DO_NOT_SPELL_CHECK,
        ff::COMB,
        ff::SORT,
        ff::EDIT,
        ff::MULTI_SELECT,
        ff::COMMIT_ON_SEL_CHANGE,
    ]
};

/// A check box for a `/Ff` bit (`inverted`: checked when the bit is clear).
fn flag_box(ui: &mut egui::Ui, flags: &mut u32, bit: u32, inverted: bool, label: &str) {
    let mut on = (*flags & bit != 0) != inverted;
    if ui.checkbox(&mut on, label).changed() {
        if on != inverted {
            *flags |= bit;
        } else {
            *flags &= !bit;
        }
    }
}

/// Draw Field Properties; returns (apply, cancel).
pub(crate) fn body(ui: &mut egui::Ui, d: &mut FieldDraft, t: &crate::theme::Tokens) -> (bool, bool) {
    use crate::widgets;
    ui.set_width(600.0);
    ui.label(egui::RichText::new(d.title()).font(theme::semibold(18.0)));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for (tab, label) in d.tabs() {
            if widgets::mode_tab(ui, label, d.tab == tab).clicked() {
                d.tab = tab;
            }
        }
    });
    ui.separator();
    ui.add_space(6.0);
    let mut enter = false;
    // A locked field keeps its properties until it is unlocked (Acrobat greys them out).
    let was_locked = d.original.as_ref().as_ref().is_some_and(|o| o.locked) && d.locked;
    if was_locked {
        ui.label(egui::RichText::new(crate::i18n::text("ui.this_field_is_locked_unlock_it_to_change_its_properties")).small().color(t.text_muted));
    }
    ui.scope(|ui| {
        if was_locked {
            ui.disable();
        }
        match d.tab {
            FieldTab::General => {
                egui::Grid::new("field-general").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                    let l = ui.label(crate::i18n::text("ui.name_2683ca"));
                    let r = ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(340.0)).labelled_by(l.id);
                    enter |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.end_row();
                    let l = ui.label(crate::i18n::text("ui.tooltip"));
                    ui.add(egui::TextEdit::singleline(&mut d.tooltip).desired_width(340.0)).labelled_by(l.id);
                    ui.end_row();
                });
                ui.add_space(12.0);
                widgets::section_title(ui, crate::i18n::text("ui.common_properties"));
                ui.checkbox(&mut d.read_only, crate::i18n::text("ui.read_only"));
                ui.checkbox(&mut d.required, crate::i18n::text("ui.required"));
            }
            FieldTab::Appearance => {
                egui::Grid::new("field-appearance").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                    ui.label(crate::i18n::text("ui.font_size"));
                    let mut auto = d.font_size == 0.0;
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut auto, crate::i18n::text("ui.auto")).changed() {
                            d.font_size = if auto { 0.0 } else { 12.0 };
                        }
                        ui.add_enabled(
                            !auto,
                            egui::DragValue::new(&mut d.font_size).range(2.0..=100.0).speed(0.25).suffix(crate::i18n::text("ui.pt")),
                        );
                    });
                    ui.end_row();
                    if let Some(l) = d.look.as_mut() {
                        ui.label(crate::i18n::text("ui.font"));
                        egui::ComboBox::from_id_salt("field-font").selected_text(crate::i18n::current().tr(l.font.label())).show_ui(ui, |ui| {
                            for f in FieldFont::ALL {
                                ui.selectable_value(&mut l.font, f, crate::i18n::current().tr(f.label()));
                            }
                        });
                        ui.end_row();
                        ui.label(crate::i18n::text("ui.text_color"));
                        if let Some(c) = crate::comments::swatch_grid(ui, Some(l.text)) {
                            l.text = c;
                        }
                        ui.end_row();
                    }
                });
                if let Some(l) = d.look.as_mut() {
                    widgets::section_title(ui, crate::i18n::text("ui.borders_and_colors"));
                    egui::Grid::new("field-borders").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        for (label, slot) in
                            [(crate::i18n::text("ui.border_color"), &mut l.border), (crate::i18n::text("ui.fill_color"), &mut l.fill)]
                        {
                            ui.label(label);
                            ui.horizontal(|ui| {
                                let mut none = slot.is_none();
                                if ui.checkbox(&mut none, crate::i18n::text("ui.no_color")).changed() {
                                    *slot = if none { None } else { Some([0.0; 3]) };
                                }
                                if let Some(c) = crate::comments::swatch_grid(ui, *slot) {
                                    *slot = Some(c);
                                }
                            });
                            ui.end_row();
                        }
                        ui.label(crate::i18n::text("ui.line_thickness_fa3775"));
                        let thick = |w: f64| {
                            if w <= 1.0 {
                                crate::i18n::text("ui.thin")
                            } else if w <= 2.0 {
                                crate::i18n::text("ui.medium")
                            } else {
                                crate::i18n::text("ui.thick")
                            }
                        };
                        egui::ComboBox::from_id_salt("field-width").selected_text(thick(l.width)).show_ui(ui, |ui| {
                            for (w, label) in
                                [(1.0, crate::i18n::text("ui.thin")), (2.0, crate::i18n::text("ui.medium")), (3.0, crate::i18n::text("ui.thick"))]
                            {
                                ui.selectable_value(&mut l.width, w, label);
                            }
                        });
                        ui.end_row();
                        ui.label(crate::i18n::text("ui.line_style"));
                        egui::ComboBox::from_id_salt("field-style").selected_text(crate::i18n::current().tr(l.style.label())).show_ui(ui, |ui| {
                            for s in BorderStyle::ALL {
                                ui.selectable_value(&mut l.style, s, crate::i18n::current().tr(s.label()));
                            }
                        });
                        ui.end_row();
                    });
                }
            }
            FieldTab::Position => {
                egui::Grid::new("field-position").num_columns(4).spacing([12.0, 10.0]).show(ui, |ui| {
                    for (i, label) in [
                        crate::i18n::text("ui.left_73f993"),
                        crate::i18n::text("ui.bottom_bbd0a4"),
                        crate::i18n::text("ui.width"),
                        crate::i18n::text("ui.height"),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        ui.label(label);
                        let range = if i < 2 { -14_400.0..=14_400.0 } else { 4.0..=14_400.0 };
                        ui.add(egui::DragValue::new(&mut d.position[i]).range(range).speed(0.5).suffix(crate::i18n::text("ui.pt")));
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
                ui.label(egui::RichText::new(crate::i18n::text("ui.points_from_the_page_s_bottom_left_corner")).small().color(t.text_faint));
            }
            FieldTab::Format => format_tab(ui, d, t),
            FieldTab::Validate => validate_tab(ui, d),
            FieldTab::Calculate => calculate_tab(ui, d, t),
            FieldTab::Actions => actions_tab(ui, d, t),
            FieldTab::Options => match d.kind {
                FormFieldKind::Text => {
                    use printcraft_engine::field_flags as ff;
                    egui::Grid::new("text-options").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label(crate::i18n::text("ui.alignment"));
                        egui::ComboBox::from_id_salt("quadding")
                            .selected_text(
                                [crate::i18n::text("ui.left"), crate::i18n::text("ui.center"), crate::i18n::text("ui.right")]
                                    [d.quadding.clamp(0, 2) as usize],
                            )
                            .show_ui(ui, |ui| {
                                for (q, l) in
                                    [(0, crate::i18n::text("ui.left")), (1, crate::i18n::text("ui.center")), (2, crate::i18n::text("ui.right"))]
                                {
                                    ui.selectable_value(&mut d.quadding, q, l);
                                }
                            });
                        ui.end_row();
                        let l = ui.label(crate::i18n::text("ui.default_value"));
                        ui.add(egui::TextEdit::singleline(&mut d.default).desired_width(260.0)).labelled_by(l.id);
                        ui.end_row();
                    });
                    ui.add_space(6.0);
                    ui.checkbox(&mut d.multiline, crate::i18n::text("ui.multi_line"));
                    flag_box(ui, &mut d.flags, ff::DO_NOT_SCROLL, true, crate::i18n::text("ui.scroll_long_text"));
                    flag_box(ui, &mut d.flags, ff::RICH_TEXT, false, crate::i18n::text("ui.allow_rich_text_formatting"));
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut d.limit, crate::i18n::text("ui.limit_of"));
                        ui.add_enabled(d.limit, egui::DragValue::new(&mut d.max_len).range(0..=10_000));
                        ui.label(crate::i18n::text("ui.characters"));
                    });
                    flag_box(ui, &mut d.flags, ff::PASSWORD, false, crate::i18n::text("ui.password"));
                    flag_box(ui, &mut d.flags, ff::FILE_SELECT, false, crate::i18n::text("ui.field_is_used_for_file_selection"));
                    flag_box(ui, &mut d.flags, ff::DO_NOT_SPELL_CHECK, true, crate::i18n::text("ui.check_spelling"));
                    ui.horizontal(|ui| {
                        flag_box(ui, &mut d.flags, ff::COMB, false, crate::i18n::text("ui.comb_of"));
                        if d.flags & ff::COMB != 0 {
                            d.limit = true;
                            d.multiline = false;
                            d.flags &= !(ff::PASSWORD | ff::FILE_SELECT);
                            if d.max_len == 0 {
                                d.max_len = 10;
                            }
                        }
                        ui.add_enabled(d.flags & ff::COMB != 0, egui::DragValue::new(&mut d.max_len).range(1..=500));
                        ui.label(crate::i18n::text("ui.characters"));
                    });
                }
                FormFieldKind::CheckBox | FormFieldKind::Radio => {
                    use printcraft_engine::field_flags as ff;
                    let on = d.on_state.clone();
                    ui.label(crate::msg!(export_value_value, on = on));
                    let mut checked = !d.default.is_empty() && d.default == on;
                    let label = if d.kind == FormFieldKind::CheckBox {
                        crate::i18n::text("ui.check_box_is_checked_by_default")
                    } else {
                        crate::i18n::text("ui.button_is_checked_by_default")
                    };
                    if ui.checkbox(&mut checked, label).changed() {
                        d.default = if checked { on } else { String::new() };
                    }
                    if d.kind == FormFieldKind::Radio {
                        flag_box(
                            ui,
                            &mut d.flags,
                            ff::RADIOS_IN_UNISON,
                            false,
                            crate::i18n::text("ui.buttons_with_the_same_name_and_value_are_selected_in_unison"),
                        );
                    }
                }
                _ => {
                    ui.horizontal(|ui| {
                        let l = ui.label(crate::i18n::text("ui.item"));
                        let r = ui.add(egui::TextEdit::singleline(&mut d.new_option).desired_width(240.0)).labelled_by(l.id);
                        let typed = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (widgets::pill_button(ui, crate::i18n::text("ui.add"), false).clicked() || typed) && !d.new_option.trim().is_empty() {
                            d.options.push(d.new_option.trim().to_string());
                            d.new_option.clear();
                        }
                    });
                    ui.add_space(6.0);
                    widgets::section_title(ui, crate::i18n::text("ui.item_list"));
                    let mut remove = None;
                    let mut up = None;
                    egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                        for (i, o) in d.options.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(o);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button(crate::i18n::text("ui.delete")).on_hover_text(crate::msg!(delete_value, o = o)).clicked() {
                                        remove = Some(i);
                                    }
                                    if i > 0 && ui.small_button(crate::i18n::text("ui.up")).clicked() {
                                        up = Some(i);
                                    }
                                });
                            });
                        }
                    });
                    if let Some(i) = remove {
                        d.options.remove(i);
                    }
                    if let Some(i) = up {
                        d.options.swap(i - 1, i);
                    }
                    if d.options.is_empty() {
                        ui.label(egui::RichText::new(crate::i18n::text("ui.add_the_choices_people_pick_from")).color(t.text_muted));
                    }
                    use printcraft_engine::field_flags as ff;
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::text("ui.default_d1f6d9"));
                        let shown = if d.default.is_empty() { "(none)".to_string() } else { d.default.clone() };
                        egui::ComboBox::from_id_salt("choice-default").selected_text(shown).show_ui(ui, |ui| {
                            ui.selectable_value(&mut d.default, String::new(), "(none)");
                            for o in d.options.clone() {
                                ui.selectable_value(&mut d.default, o.clone(), o);
                            }
                        });
                    });
                    flag_box(ui, &mut d.flags, ff::SORT, false, crate::i18n::text("ui.sort_items"));
                    if d.kind == FormFieldKind::Combo {
                        flag_box(ui, &mut d.flags, ff::EDIT, false, crate::i18n::text("ui.allow_user_to_enter_custom_text"));
                        flag_box(ui, &mut d.flags, ff::DO_NOT_SPELL_CHECK, true, crate::i18n::text("ui.check_spelling"));
                    } else {
                        flag_box(ui, &mut d.flags, ff::MULTI_SELECT, false, crate::i18n::text("ui.multiple_selection"));
                    }
                    flag_box(ui, &mut d.flags, ff::COMMIT_ON_SEL_CHANGE, false, crate::i18n::text("ui.commit_selected_value_immediately"));
                }
            },
        }
    });
    ui.add_space(8.0);
    ui.checkbox(&mut d.locked, crate::i18n::text("ui.locked"));
    ui.add_space(6.0);
    let (mut apply, mut cancel) = (enter, false);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if widgets::pill_button(ui, crate::i18n::text("ui.ok"), true).clicked() {
            apply = true;
        }
        if widgets::pill_button(ui, crate::i18n::text("ui.cancel"), false).clicked() {
            cancel = true;
        }
    });
    (apply, cancel)
}

/// The categories of the Format tab.
#[derive(Clone, Copy, PartialEq)]
enum Category {
    None,
    Number,
    Percent,
    Date,
    Time,
    Special,
}

fn category(f: &Format) -> Category {
    match f {
        Format::None => Category::None,
        Format::Number { .. } => Category::Number,
        Format::Percent { .. } => Category::Percent,
        Format::Date(_) => Category::Date,
        Format::Time(_) => Category::Time,
        Format::Special(_) | Format::Mask(_) => Category::Special,
    }
}

const SEPARATORS: [&str; 5] = ["1,234.56", "1234.56", "1.234,56", "1234,56", "1'234.56"];
const NEGATIVES: [&str; 4] = ["-1,234.01", "1,234.01 (red)", "(1,234.01)", "(1,234.01) (red)"];
const SPECIALS: [&str; 5] = ["Zip Code", "Zip Code + 4", "Phone Number", "Social Security Number", "Arbitrary Mask"];

fn format_tab(ui: &mut egui::Ui, d: &mut FieldDraft, t: &crate::theme::Tokens) {
    let cat = category(&d.format);
    let mut picked = cat;
    ui.horizontal(|ui| {
        ui.label(crate::i18n::text("ui.select_format_category"));
        egui::ComboBox::from_id_salt("format-cat")
            .selected_text(match cat {
                Category::None => crate::i18n::text("ui.none"),
                Category::Number => crate::i18n::text("ui.number"),
                Category::Percent => crate::i18n::text("ui.percentage"),
                Category::Date => crate::i18n::text("ui.date_99c40a"),
                Category::Time => crate::i18n::text("ui.time"),
                Category::Special => crate::i18n::text("ui.special"),
            })
            .show_ui(ui, |ui| {
                for (c, l) in [
                    (Category::None, crate::i18n::text("ui.none")),
                    (Category::Number, crate::i18n::text("ui.number")),
                    (Category::Percent, crate::i18n::text("ui.percentage")),
                    (Category::Date, crate::i18n::text("ui.date_99c40a")),
                    (Category::Time, crate::i18n::text("ui.time")),
                    (Category::Special, crate::i18n::text("ui.special")),
                ] {
                    ui.selectable_value(&mut picked, c, l);
                }
            });
    });
    if picked != cat {
        d.format = match picked {
            Category::None => Format::None,
            Category::Number => Format::Number { decimals: 2, sep: 0, neg: 0, currency: String::new(), prepend: true },
            Category::Percent => Format::Percent { decimals: 2, sep: 0 },
            Category::Date => Format::Date("mm/dd/yyyy".into()),
            Category::Time => Format::Time("HH:MM".into()),
            Category::Special => Format::Special(0),
        };
    }
    ui.add_space(8.0);
    let sample = match &d.format {
        Format::Number { .. } | Format::Percent { .. } => "-1234.5",
        Format::Date(_) => "10/1/2026",
        Format::Time(_) => "14:05",
        Format::Special(2) => "5551234567",
        Format::Special(3) => "123456789",
        Format::Special(_) => "123456789",
        _ => "",
    };
    match &mut d.format {
        Format::None => {
            ui.label(egui::RichText::new(crate::i18n::text("ui.the_value_is_shown_as_typed")).color(t.text_muted));
        }
        Format::Number { decimals, sep, neg, currency, prepend } => {
            egui::Grid::new("fmt-number").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(crate::i18n::text("ui.decimal_places"));
                ui.add(egui::DragValue::new(decimals).range(0..=10));
                ui.end_row();
                ui.label(crate::i18n::text("ui.separator_style"));
                egui::ComboBox::from_id_salt("fmt-sep").selected_text(SEPARATORS[(*sep).min(4) as usize]).show_ui(ui, |ui| {
                    for (i, s) in SEPARATORS.iter().enumerate() {
                        ui.selectable_value(sep, i as u8, *s);
                    }
                });
                ui.end_row();
                ui.label(crate::i18n::text("ui.currency_symbol"));
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("fmt-cur")
                        .selected_text(if currency.is_empty() { crate::i18n::text("ui.none") } else { currency.as_str() })
                        .width(70.0)
                        .show_ui(ui, |ui| {
                            for c in ["", "$", "€", "£", "¥", "CHF "] {
                                ui.selectable_value(currency, c.to_string(), if c.is_empty() { crate::i18n::text("ui.none") } else { c });
                            }
                        });
                    ui.checkbox(prepend, crate::i18n::text("ui.before_the_number"));
                });
                ui.end_row();
                ui.label(crate::i18n::text("ui.negative_number_style"));
                egui::ComboBox::from_id_salt("fmt-neg").selected_text(crate::i18n::current().tr(NEGATIVES[(*neg).min(3) as usize])).show_ui(
                    ui,
                    |ui| {
                        for (i, s) in NEGATIVES.iter().enumerate() {
                            ui.selectable_value(neg, i as u8, crate::i18n::current().tr(s));
                        }
                    },
                );
                ui.end_row();
            });
        }
        Format::Percent { decimals, sep } => {
            egui::Grid::new("fmt-pct").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(crate::i18n::text("ui.decimal_places"));
                ui.add(egui::DragValue::new(decimals).range(0..=10));
                ui.end_row();
                ui.label(crate::i18n::text("ui.separator_style"));
                egui::ComboBox::from_id_salt("fmt-psep").selected_text(SEPARATORS[(*sep).min(4) as usize]).show_ui(ui, |ui| {
                    for (i, s) in SEPARATORS.iter().enumerate() {
                        ui.selectable_value(sep, i as u8, *s);
                    }
                });
                ui.end_row();
            });
        }
        Format::Date(p) | Format::Time(p) => {
            let presets: Vec<&str> = if matches!(cat, Category::Time) || picked == Category::Time {
                TIME_PRESETS.to_vec()
            } else {
                DATE_PRESETS.iter().copied().chain(["m/d/yyyy", "mm/dd/yyyy", "yyyy-mm-dd", "dd/mm/yyyy"]).collect()
            };
            ui.horizontal(|ui| {
                ui.label(crate::i18n::text("ui.format_9a5649"));
                egui::ComboBox::from_id_salt("fmt-date").selected_text(p.as_str()).width(170.0).show_ui(ui, |ui| {
                    for pr in presets {
                        ui.selectable_value(p, pr.to_string(), pr);
                    }
                });
                ui.label(crate::i18n::text("ui.custom"));
                ui.add(egui::TextEdit::singleline(p).desired_width(120.0));
            });
        }
        Format::Special(n) => {
            let mut idx = *n as usize;
            egui::ComboBox::from_id_salt("fmt-special").selected_text(crate::i18n::current().tr(SPECIALS[idx.min(4)])).show_ui(ui, |ui| {
                for (i, s) in SPECIALS.iter().enumerate() {
                    ui.selectable_value(&mut idx, i, *s);
                }
            });
            if idx == 4 {
                d.format = Format::Mask("999-999".into());
            } else {
                *n = idx as u8;
            }
        }
        Format::Mask(m) => {
            let mut idx = 4usize;
            egui::ComboBox::from_id_salt("fmt-special").selected_text(crate::i18n::current().tr(SPECIALS[4])).show_ui(ui, |ui| {
                for (i, s) in SPECIALS.iter().enumerate() {
                    ui.selectable_value(&mut idx, i, *s);
                }
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::text("ui.mask"));
                ui.add(egui::TextEdit::singleline(m).desired_width(160.0));
            });
            ui.label(egui::RichText::new(crate::i18n::text("ui.9_digit_a_letter_o_letter_or_digit_x_any_character")).small().color(t.text_faint));
            if idx < 4 {
                d.format = Format::Special(idx as u8);
            }
        }
    }
    if !sample.is_empty() && d.format != Format::None {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(crate::msg!(example_value, format_value(&d.format, sample))).color(t.text_muted));
    }
}

fn validate_tab(ui: &mut egui::Ui, d: &mut FieldDraft) {
    let mut ranged = matches!(d.validate, Validate::Range { .. });
    ui.radio_value(&mut ranged, false, crate::i18n::text("ui.field_value_is_not_validated"));
    ui.radio_value(&mut ranged, true, crate::i18n::text("ui.field_value_is_in_range"));
    if !ranged {
        d.validate = Validate::None;
        return;
    }
    if d.validate == Validate::None {
        d.validate = Validate::Range { min: Some(0.0), max: Some(100.0) };
    }
    if let Validate::Range { min, max } = &mut d.validate {
        ui.horizontal(|ui| {
            let mut has = min.is_some();
            ui.checkbox(&mut has, crate::i18n::text("ui.from_dc45d3"));
            let mut v = min.unwrap_or(0.0);
            ui.add_enabled(has, egui::DragValue::new(&mut v).speed(1.0));
            *min = has.then_some(v);
            let mut has = max.is_some();
            ui.checkbox(&mut has, crate::i18n::text("ui.to"));
            let mut v = max.unwrap_or(100.0);
            ui.add_enabled(has, egui::DragValue::new(&mut v).speed(1.0));
            *max = has.then_some(v);
        });
    }
}

fn actions_tab(ui: &mut egui::Ui, d: &mut FieldDraft, t: &crate::theme::Tokens) {
    use printcraft_engine::FieldTrigger as T;
    ui.label(egui::RichText::new(crate::i18n::text("ui.add_an_action")).font(theme::semibold(13.0)));
    let a = &mut d.new_action;
    egui::Grid::new("field-actions-add").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label(crate::i18n::text("ui.select_trigger"));
        egui::ComboBox::from_id_salt("action-trigger").selected_text(crate::i18n::current().tr(a.trigger.label())).width(220.0).show_ui(ui, |ui| {
            for tr in T::ALL {
                ui.selectable_value(&mut a.trigger, tr, crate::i18n::current().tr(tr.label()));
            }
        });
        ui.end_row();
        ui.label(crate::i18n::text("ui.select_action"));
        egui::ComboBox::from_id_salt("action-kind").selected_text(crate::i18n::current().tr(ACTION_KINDS[a.kind])).width(220.0).show_ui(ui, |ui| {
            for (i, k) in ACTION_KINDS.iter().enumerate() {
                ui.selectable_value(&mut a.kind, i, crate::i18n::current().tr(k));
            }
        });
        ui.end_row();
        let hint = match a.kind {
            0 => "JavaScript",
            1 => "https://…",
            2 => crate::i18n::text("ui.fields_to_reset_comma_separated_empty_all"),
            3 => crate::i18n::text("ui.print_nextpage_prevpage_firstpage_or_lastpage"),
            4 => crate::i18n::text("ui.page_number"),
            5 | 6 => crate::i18n::text("ui.field_names_comma_separated"),
            _ => crate::i18n::text("ui.url_to_submit_to"),
        };
        ui.label(if a.kind == 0 { crate::i18n::text("ui.script") } else { crate::i18n::text("ui.value") });
        if a.kind == 0 {
            ui.add(egui::TextEdit::multiline(&mut a.text).code_editor().desired_rows(3).desired_width(340.0).hint_text(hint).id_salt("action-text"));
        } else {
            ui.add(egui::TextEdit::singleline(&mut a.text).desired_width(340.0).hint_text(hint).id_salt("action-text"));
        }
        ui.end_row();
    });
    let mut error = None;
    if ui.button(crate::i18n::text("ui.add")).clicked() {
        match d.new_action.action() {
            Ok(act) => {
                let tr = d.new_action.trigger;
                d.actions.retain(|(x, _)| *x != tr);
                d.actions.push((tr, act));
                d.actions.sort_by_key(|(x, _)| T::ALL.iter().position(|y| y == x));
                d.new_action.text.clear();
            }
            Err(e) => error = Some(e),
        }
    }
    if let Some(e) = error {
        ui.label(egui::RichText::new(e).small().color(t.text_muted));
    }
    ui.add_space(8.0);
    ui.label(egui::RichText::new(crate::i18n::text("ui.actions")).font(theme::semibold(13.0)));
    egui::Frame::new().fill(t.hover).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if d.actions.is_empty() {
            ui.label(egui::RichText::new(crate::i18n::text("ui.no_actions")).color(t.text_muted));
        }
        let mut remove = None;
        for (i, (tr, act)) in d.actions.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::current().tr(tr.label())).strong());
                ui.label(act.describe());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.push_id(i, |ui| ui.button(crate::i18n::text("ui.delete"))).inner.clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            d.actions.remove(i);
        }
    });
}

fn calculate_tab(ui: &mut egui::Ui, d: &mut FieldDraft, t: &crate::theme::Tokens) {
    #[derive(PartialEq, Clone, Copy)]
    enum Mode {
        None,
        Simple,
        Notation,
    }
    let mode = match d.calculate {
        Calculate::None => Mode::None,
        Calculate::Simple { .. } => Mode::Simple,
        Calculate::Notation(_) => Mode::Notation,
    };
    let mut m = mode;
    ui.radio_value(&mut m, Mode::None, crate::i18n::text("ui.value_is_not_calculated"));
    ui.radio_value(&mut m, Mode::Simple, crate::i18n::text("ui.value_is_the"));
    if m != mode {
        d.calculate = match m {
            Mode::None => Calculate::None,
            Mode::Simple => Calculate::Simple { op: CalcOp::Sum, fields: Vec::new() },
            Mode::Notation => Calculate::Notation(String::new()),
        };
    }
    if let Calculate::Simple { op, fields } = &mut d.calculate {
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            egui::ComboBox::from_id_salt("calc-op").selected_text(crate::i18n::current().tr(op.label())).show_ui(ui, |ui| {
                for o in CalcOp::ALL {
                    ui.selectable_value(op, o, crate::i18n::current().tr(o.label()));
                }
            });
            ui.label(crate::i18n::text("ui.of_the_following_fields"));
        });
        egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
            for name in &d.others {
                let mut on = fields.contains(name);
                if ui.checkbox(&mut on, name).changed() {
                    if on {
                        fields.push(name.clone());
                    } else {
                        fields.retain(|f| f != name);
                    }
                }
            }
        });
    }
    let mut m2 = match d.calculate {
        Calculate::Notation(_) => Mode::Notation,
        _ => m,
    };
    if ui.radio_value(&mut m2, Mode::Notation, crate::i18n::text("ui.simplified_field_notation")).clicked()
        && !matches!(d.calculate, Calculate::Notation(_))
    {
        d.calculate = Calculate::Notation(String::new());
    }
    if let Calculate::Notation(expr) = &mut d.calculate {
        ui.add(egui::TextEdit::multiline(expr).hint_text("Price * Quantity").desired_rows(2).desired_width(420.0));
        ui.label(
            egui::RichText::new(crate::i18n::text("ui.field_names_with_and_parentheses_put_before_spaces_in_names")).small().color(t.text_faint),
        );
    }
}
