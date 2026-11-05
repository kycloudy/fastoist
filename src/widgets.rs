//! Task rows, calendar chips, sidebar rows, and date labels.

use crate::model::{DueAt, Event, Item, Store};
use crate::theme::{self, Icon, *};
use chrono::{Datelike, NaiveDate, NaiveTime};
use eframe::egui::{
    self, text::LayoutJob, Align2, Color32, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Stroke, TextFormat,
    Ui, Vec2,
};
use std::sync::Arc;

/// Something the user did that changes data or navigation. Views collect these; the app applies them.
#[derive(Clone, Debug)]
pub enum Action {
    Open(String),
    Complete(String),
    Uncomplete(String),
    Delete(String),
    Schedule { id: String, date: Option<NaiveDate>, time: Option<Option<NaiveTime>> },
    Priority(String, u8),
    AddTask(AddDefaults),
    AddSubtask { parent: String, content: String },
    /// Show the week that holds this day in Upcoming.
    ShowWeek(NaiveDate),
    /// Open a Google event to change it.
    OpenEvent(String),
    MoveEvent { key: String, date: NaiveDate, time: Option<NaiveTime> },
    Go(crate::app::View),
}

#[derive(Clone, Debug, Default)]
pub struct AddDefaults {
    pub project_id: Option<String>,
    pub section_id: Option<String>,
    pub parent_id: Option<String>,
    pub due: Option<NaiveDate>,
    pub labels: Vec<String>,
}

/// Drag payload for moving a Google event. Holds the event key.
#[derive(Clone, Debug)]
pub struct EventDrag(pub String);

/// Drag payload for moving a task onto a calendar day or time.
#[derive(Clone, Debug)]
pub struct TaskDrag(pub String);

pub fn fmt_time(t: NaiveTime) -> String {
    if t.format("%M").to_string() == "00" {
        t.format("%-I %p").to_string()
    } else {
        t.format("%-I:%M %p").to_string()
    }
}

pub fn day_name(d: NaiveDate, today: NaiveDate) -> String {
    let diff = (d - today).num_days();
    match diff {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        2..=6 => d.format("%A").to_string(),
        _ if d.year() == today.year() => d.format("%b %-d").to_string(),
        _ => d.format("%b %-d %Y").to_string(),
    }
}

pub fn due_label(at: DueAt, today: NaiveDate) -> (String, Color32) {
    let diff = (at.date - today).num_days();
    let mut text = day_name(at.date, today);
    if let Some(t) = at.time {
        text.push(' ');
        text.push_str(&fmt_time(t));
    }
    let overdue = diff < 0 || (diff == 0 && at.time.is_some_and(|t| t < chrono::Local::now().time()));
    let color = match diff {
        _ if overdue => RED,
        0 => GREEN,
        1 => ORANGE,
        2..=6 => PURPLE,
        _ => MUTED,
    };
    (text, color)
}

pub fn deadline_label(d: NaiveDate, today: NaiveDate) -> (String, Color32) {
    let diff = (d - today).num_days();
    let text = match diff {
        0 => "Today".to_string(),
        1 => "In 1 day".to_string(),
        2..=7 => format!("In {diff} days"),
        _ if d.year() == today.year() => d.format("%b %-d").to_string(),
        _ => d.format("%b %-d %Y").to_string(),
    };
    let color = match diff {
        _ if diff < 0 => RED,
        0..=1 => ORANGE,
        2..=7 => PURPLE,
        _ => MUTED,
    };
    (text, color)
}

fn galley(ui: &Ui, text: &str, font: FontId, color: Color32, wrap: f32) -> Arc<Galley> {
    ui.painter().layout(text.to_string(), font, color, wrap)
}

fn galley_one(ui: &Ui, text: &str, font: FontId, color: Color32) -> Arc<Galley> {
    ui.painter().layout_no_wrap(text.to_string(), font, color)
}

/// Paint an icon followed by text; returns the x after it.
fn meta(p: &Painter, ui: &Ui, x: f32, y: f32, ic: Option<Icon>, text: &str, color: Color32) -> f32 {
    let mut x = x;
    if let Some(ic) = ic {
        theme::icon(p, Rect::from_center_size(Pos2::new(x + 6.0, y), Vec2::splat(12.0)), ic, color);
        x += 15.0;
    }
    let g = galley_one(ui, text, font(12.0), color);
    let w = g.size().x;
    p.galley(Pos2::new(x, y - g.size().y / 2.0), g, color);
    x + w + 10.0
}

pub struct RowOpts {
    pub show_project: bool,
    pub show_due: bool,
    pub depth: usize,
    pub compact: bool,
}

impl Default for RowOpts {
    fn default() -> Self {
        Self { show_project: true, show_due: true, depth: 0, compact: false }
    }
}

/// A task row as in Todoist's list views. Click opens, circle completes, drag reschedules.
pub fn task_row(ui: &mut Ui, store: &Store, item: &Item, today: NaiveDate, o: &RowOpts, out: &mut Vec<Action>) -> Response {
    let full_w = ui.available_width();
    let indent = o.depth as f32 * 28.0;
    let x_text = indent + 28.0;
    let wrap = (full_w - x_text - 8.0).max(60.0);
    let title_font = font(if o.compact { 13.5 } else { 14.0 });
    let title = galley(ui, &item.content, title_font, TEXT, wrap);
    let desc_line = item.description.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    let desc = (!desc_line.is_empty()).then(|| {
        let mut job = LayoutJob::single_section(desc_line.clone(), TextFormat::simple(font(12.0), MUTED));
        job.wrap.max_width = wrap;
        job.wrap.max_rows = 1;
        ui.painter().layout_job(job)
    });
    let sub = store.subtasks(&item.id);
    let has_meta = (o.show_due && item.due.is_some()) || item.deadline.is_some() || !item.labels.is_empty() || o.show_project || !sub.is_empty();
    let pad = if o.compact { 7.0 } else { 9.0 };
    let mut h = pad + title.size().y;
    if let Some(d) = &desc {
        h += d.size().y + 1.0;
    }
    if has_meta {
        h += 19.0;
    }
    h += pad;

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(full_w, h), Sense::click_and_drag());
    let p = ui.painter_at(rect.expand(2.0));
    let hovered = resp.hovered();
    let circle_c = Pos2::new(rect.left() + indent + 10.0, rect.top() + pad + title.size().y / 2.0);
    let circle_hit = Rect::from_center_size(circle_c, Vec2::splat(24.0));
    let pointer = ui.ctx().pointer_hover_pos();
    let over_circle = pointer.is_some_and(|pt| circle_hit.contains(pt));

    theme::check_circle(&p, circle_c, 9.0, item.priority, over_circle, false);
    let mut y = rect.top() + pad;
    p.galley(Pos2::new(rect.left() + x_text, y), title.clone(), TEXT);
    y += title.size().y + 1.0;
    if let Some(d) = desc {
        let dh = d.size().y;
        p.galley(Pos2::new(rect.left() + x_text, y), d, MUTED);
        y += dh;
    }
    if has_meta {
        let my = y + 9.5;
        let mut x = rect.left() + x_text;
        if !sub.is_empty() {
            x = meta(&p, ui, x, my, None, &format!("⑂ 0/{}", sub.len()), MUTED);
        }
        if o.show_due {
            if let Some(at) = item.due_at() {
                let (t, c) = due_label(at, today);
                x = meta(&p, ui, x, my, Some(Icon::Calendar), &t, c);
                if item.is_recurring() {
                    theme::icon(&p, Rect::from_center_size(Pos2::new(x - 2.0, my), Vec2::splat(11.0)), Icon::Repeat, c);
                    x += 10.0;
                }
            }
        }
        if store.reminders.values().any(|r| r.item_id == item.id) {
            theme::icon(&p, Rect::from_center_size(Pos2::new(x + 4.0, my), Vec2::splat(12.0)), Icon::Alarm, MUTED);
            x += 20.0;
        }
        if let Some(d) = item.deadline.as_ref().and_then(|d| d.day()) {
            let (t, c) = deadline_label(d, today);
            x = meta(&p, ui, x, my, Some(Icon::Deadline), &t, c);
        }
        for l in &item.labels {
            x = meta(&p, ui, x, my, Some(Icon::Tag), l, MUTED);
        }
        if o.show_project {
            let proj = store.projects.get(&item.project_id);
            let name = proj.map(|p| p.name.as_str()).unwrap_or("");
            let col = proj.map(|p| named_color(&p.color)).unwrap_or(MUTED);
            let section = item.section_id.as_ref().and_then(|s| store.sections.get(s)).map(|s| format!(" / {}", s.name)).unwrap_or_default();
            let g = galley_one(ui, &format!("{name}{section}"), font(12.0), MUTED);
            let gx = rect.right() - 16.0 - g.size().x;
            // Skip the project when the row has no room left for it.
            if gx >= x {
                p.galley(Pos2::new(gx, my - g.size().y / 2.0), g, MUTED);
                p.text(Pos2::new(rect.right() - 6.0, my), Align2::CENTER_CENTER, "#", font(13.0), col);
            }
        }
    }
    p.line_segment([Pos2::new(rect.left() + indent, rect.bottom()), Pos2::new(rect.right(), rect.bottom())], Stroke::new(1.0, LINE));
    if hovered && !over_circle {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    resp.dnd_set_drag_payload(TaskDrag(item.id.clone()));
    if resp.clicked() {
        if over_circle {
            out.push(Action::Complete(item.id.clone()));
        } else {
            out.push(Action::Open(item.id.clone()));
        }
    }
    task_menu(&resp, item, today, out);
    resp
}

pub fn task_menu(resp: &Response, item: &Item, today: NaiveDate, out: &mut Vec<Action>) {
    resp.context_menu(|ui| {
        ui.set_min_width(200.0);
        if ui.button("Open").clicked() {
            out.push(Action::Open(item.id.clone()));
        }
        if ui.button("Complete").clicked() {
            out.push(Action::Complete(item.id.clone()));
        }
        ui.separator();
        ui.label(egui::RichText::new("Date").small().color(MUTED));
        ui.horizontal(|ui| {
            let next_week = today + chrono::Duration::days(7 - today.weekday().num_days_from_monday() as i64);
            for (label, d) in [("Today", Some(today)), ("Tomorrow", Some(today.succ_opt().unwrap())), ("Next week", Some(next_week)), ("No date", None)] {
                if ui.small_button(label).clicked() {
                    out.push(Action::Schedule { id: item.id.clone(), date: d, time: None });
                    ui.close();
                }
            }
        });
        ui.label(egui::RichText::new("Priority").small().color(MUTED));
        ui.horizontal(|ui| {
            for (n, p) in [(1, 4u8), (2, 3), (3, 2), (4, 1)] {
                let txt = egui::RichText::new(format!("P{n}")).color(priority_color(p));
                if ui.selectable_label(item.priority == p, txt).clicked() {
                    out.push(Action::Priority(item.id.clone(), p));
                    ui.close();
                }
            }
        });
        ui.separator();
        if ui.button(egui::RichText::new("Delete").color(RED)).clicked() {
            out.push(Action::Delete(item.id.clone()));
        }
    });
}

/// "+ Add task" row.
pub fn add_row(ui: &mut Ui, defaults: AddDefaults, out: &mut Vec<Action>) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    let hovered = resp.hovered();
    let p = ui.painter();
    let c = Pos2::new(rect.left() + 10.0, rect.center().y);
    if hovered {
        p.circle_filled(c, 9.0, ACCENT);
        theme::icon(p, Rect::from_center_size(c, Vec2::splat(12.0)), Icon::Plus, Color32::WHITE);
    } else {
        theme::icon(p, Rect::from_center_size(c, Vec2::splat(14.0)), Icon::Plus, ACCENT);
    }
    p.text(Pos2::new(rect.left() + 28.0, rect.center().y), Align2::LEFT_CENTER, "Add task", font(14.0), if hovered { ACCENT_TEXT } else { MUTED });
    if resp.clicked() {
        out.push(Action::AddTask(defaults));
    }
}

/// A section or group header with a rule under it.
pub fn group_header(ui: &mut Ui, title: &str, count: Option<usize>) {
    ui.add_space(14.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
    let p = ui.painter();
    let g = galley_one(ui, title, bold_font(14.0), TEXT);
    let w = g.size().x;
    p.galley(Pos2::new(rect.left(), rect.center().y - g.size().y / 2.0), g, TEXT);
    if let Some(n) = count {
        p.text(Pos2::new(rect.left() + w + 8.0, rect.center().y), Align2::LEFT_CENTER, n.to_string(), font(12.0), MUTED);
    }
    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, LINE));
}

/// A sidebar navigation row.
pub fn nav_row(ui: &mut Ui, ic: Icon, icon_color: Color32, label: &str, count: usize, selected: bool, indent: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 6.0, SELECTED);
    } else if resp.hovered() {
        p.rect_filled(rect, 6.0, HOVER);
    }
    let fg = if selected { ACCENT_TEXT } else { TEXT };
    let ic_col = if selected { ACCENT_TEXT } else { icon_color };
    theme::icon(p, Rect::from_center_size(Pos2::new(rect.left() + 16.0 + indent, rect.center().y), Vec2::splat(16.0)), ic, ic_col);
    let max_w = rect.width() - 64.0 - indent;
    let mut job = LayoutJob::single_section(label.to_string(), TextFormat::simple(font(14.0), fg));
    job.wrap.max_width = max_w;
    job.wrap.max_rows = 1;
    let g = ui.painter().layout_job(job);
    p.galley(Pos2::new(rect.left() + 34.0 + indent, rect.center().y - g.size().y / 2.0), g, fg);
    if count > 0 {
        p.text(Pos2::new(rect.right() - 12.0, rect.center().y), Align2::RIGHT_CENTER, count.to_string(), font(12.0), if selected { ACCENT_TEXT } else { FAINT });
    }
    resp
}

/// A small icon button with a hover background.
pub fn icon_button(ui: &mut Ui, ic: Icon, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, HOVER);
    }
    theme::icon(ui.painter(), rect, ic, if resp.hovered() { TEXT } else { MUTED });
    resp.on_hover_text(tip)
}

/// A calendar chip for a task. Draggable; click opens; circle completes.
pub fn task_chip(ui: &mut Ui, rect: Rect, item: &Item, done: bool, show_time: bool, today: NaiveDate, out: &mut Vec<Action>) {
    task_card(ui, rect, rect.height(), item, done, show_time, today, out);
}

/// A timed task in the week grid: one rounded block that spans its whole duration.
pub fn task_block(ui: &mut Ui, rect: Rect, item: &Item, done: bool, today: NaiveDate, out: &mut Vec<Action>) {
    let head = rect.height().min(22.0);
    task_card(ui, rect, head, item, done, true, today, out);
    // Tall blocks also show the time range under the title.
    if let (Some(t), Some(d)) = (item.due_at().and_then(|d| d.time), item.duration.as_ref()) {
        if rect.height() >= head + 16.0 {
            let end = t + chrono::Duration::minutes(d.minutes());
            let p = ui.painter_at(rect);
            p.text(Pos2::new(rect.left() + 19.0, rect.top() + head + 1.0), Align2::LEFT_TOP, format!("{} – {}", fmt_time(t), fmt_time(end)), font(11.5), MUTED);
        }
    }
}

/// Paint a task as a filled rounded block. The title row sits in the top `head` pixels.
fn task_card(ui: &mut Ui, rect: Rect, head: f32, item: &Item, done: bool, show_time: bool, today: NaiveDate, out: &mut Vec<Action>) {
    let id = ui.scope_id().with(("chip", &item.id, rect.min.x as i32, rect.min.y as i32));
    let resp = ui.interact(rect, id, if done { Sense::click() } else { Sense::click_and_drag() });
    let p = ui.painter_at(rect);
    let alpha = if done { 0.45 } else { 1.0 };
    let mut fill = chip_fill(item.priority).gamma_multiply(alpha);
    if resp.hovered() {
        fill = fill.gamma_multiply(1.25);
    }
    p.rect_filled(rect, if head < rect.height() { 6.0 } else { 4.0 }, fill);
    let row = Rect::from_min_size(rect.min, Vec2::new(rect.width(), head));
    let c = Pos2::new(rect.left() + 10.0, row.center().y);
    let over_circle = ui.ctx().pointer_hover_pos().is_some_and(|pt| (pt - c).length() < 9.0);
    if item.is_recurring() && !done {
        theme::icon(&p, Rect::from_center_size(c, Vec2::splat(12.0)), Icon::Repeat, TEXT);
    } else {
        theme::check_circle(&p, c, 5.5, item.priority, over_circle, done);
    }
    let text_col = if done { MUTED } else { TEXT };
    let mut right = rect.right() - 6.0;
    // Narrow chips keep the title and drop the time.
    if show_time && !done && rect.width() > 150.0 {
        if let Some(t) = item.due_at().and_then(|d| d.time) {
            let g = galley_one(ui, &fmt_time(t), font(11.5), MUTED);
            right -= g.size().x;
            p.galley(Pos2::new(right, row.center().y - g.size().y / 2.0), g, MUTED);
            right -= 6.0;
        }
    }
    let mut fmt = TextFormat::simple(font(12.5), text_col);
    if done {
        fmt.strikethrough = Stroke::new(1.0, MUTED);
    }
    let mut job = LayoutJob::single_section(item.content.clone(), fmt);
    job.wrap.max_width = (right - rect.left() - 20.0).max(10.0);
    job.wrap.max_rows = 1;
    let g = ui.painter().layout_job(job);
    p.galley(Pos2::new(rect.left() + 19.0, row.center().y - g.size().y / 2.0), g, text_col);

    if done {
        if resp.clicked() && over_circle {
            out.push(Action::Uncomplete(item.id.clone()));
        }
        return;
    }
    resp.dnd_set_drag_payload(TaskDrag(item.id.clone()));
    if resp.clicked() {
        out.push(if over_circle { Action::Complete(item.id.clone()) } else { Action::Open(item.id.clone()) });
    }
    task_menu(&resp, item, today, out);
}

pub fn event_color(ev: &Event) -> Color32 {
    ev.color.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(EVENT)
}

/// Make an event draggable and clickable when Fastoist can change it.
pub fn event_interact(ui: &mut Ui, rect: Rect, ev: &Event, out: &mut Vec<Action>) {
    let when = if ev.all_day { "All day".to_string() } else { format!("{} – {}", fmt_time(ev.start.time()), fmt_time(ev.end.time())) };
    if ev.editable {
        let id = ui.scope_id().with(("ev", ev.key(), rect.min.x as i32, rect.min.y as i32));
        let resp = ui.interact(rect, id, Sense::click_and_drag());
        resp.dnd_set_drag_payload(EventDrag(ev.key()));
        if resp.clicked() {
            out.push(Action::OpenEvent(ev.key()));
        }
        resp.on_hover_text(format!("{}\n{when}", ev.title));
    } else {
        let id = ui.scope_id().with(("ev", &ev.title, rect.min.x as i32, rect.min.y as i32));
        ui.interact(rect, id, Sense::hover()).on_hover_text(format!("{}\n{when}", ev.title));
    }
}

pub fn event_chip(ui: &mut Ui, rect: Rect, ev: &Event, out: &mut Vec<Action>) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(0x2b, 0x2b, 0x2b));
    p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(3.0, 3.0), Vec2::new(2.5, rect.height() - 6.0)), 1.0, event_color(ev));
    let mut right = rect.right() - 6.0;
    if !ev.all_day && rect.width() > 150.0 {
        let g = galley_one(ui, &fmt_time(ev.start.time()), font(11.5), MUTED);
        right -= g.size().x;
        p.galley(Pos2::new(right, rect.center().y - g.size().y / 2.0), g, MUTED);
        right -= 6.0;
    }
    let mut job = LayoutJob::single_section(ev.title.clone(), TextFormat::simple(font(12.5), TEXT));
    job.wrap.max_width = (right - rect.left() - 12.0).max(10.0);
    job.wrap.max_rows = 1;
    let g = ui.painter().layout_job(job);
    p.galley(Pos2::new(rect.left() + 10.0, rect.center().y - g.size().y / 2.0), g, TEXT);
    event_interact(ui, rect, ev, out);
}

/// Order tasks the way Todoist's date views do: timed first by time, then by priority.
pub fn date_sort(a: &Item, b: &Item) -> std::cmp::Ordering {
    let ka = (a.due_date(), a.due_at().and_then(|d| d.time).is_none(), a.due_at().and_then(|d| d.time), std::cmp::Reverse(a.priority), a.day_order, a.child_order);
    let kb = (b.due_date(), b.due_at().and_then(|d| d.time).is_none(), b.due_at().and_then(|d| d.time), std::cmp::Reverse(b.priority), b.day_order, b.child_order);
    ka.cmp(&kb)
}
