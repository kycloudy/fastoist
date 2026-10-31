//! Property pickers shared by the task dialog and quick add: project, date, deadline, priority, labels.

use crate::model::{Item, Store};
use crate::theme::{self, Icon, *};
use crate::widgets::{self, fmt_time};
use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Weekday};
use eframe::egui::{self, Align2, Color32, Key, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use serde_json::{json, Value};

/// A due date being edited: a date (with optional time) or text Todoist parses ("every mon").
#[derive(Clone, Debug, PartialEq)]
pub enum DueEdit {
    At { date: NaiveDate, time: Option<NaiveTime>, recur: Option<String> },
    Text(String),
}

impl DueEdit {
    pub fn from_item(it: &Item) -> Option<Self> {
        let at = it.due_at()?;
        let recur = it.due.as_ref().filter(|d| d.is_recurring).map(|d| d.string.clone());
        Some(DueEdit::At { date: at.date, time: at.time, recur })
    }

    pub fn from_text(s: &str) -> Option<Self> {
        let s = s.trim();
        (!s.is_empty()).then(|| DueEdit::Text(s.to_string()))
    }

    pub fn to_json(&self) -> Value {
        match self {
            DueEdit::Text(s) => json!({ "string": s }),
            DueEdit::At { date, time, recur } => {
                let d = match time {
                    Some(t) => format!("{}T{}", date.format("%Y-%m-%d"), t.format("%H:%M:%S")),
                    None => date.format("%Y-%m-%d").to_string(),
                };
                match recur {
                    Some(r) => json!({ "date": d, "string": r, "is_recurring": true }),
                    None => json!({ "date": d }),
                }
            }
        }
    }

    pub fn label(&self, today: NaiveDate) -> (String, Color32) {
        match self {
            DueEdit::Text(s) => (s.clone(), PURPLE),
            DueEdit::At { date, time, .. } => widgets::due_label(crate::model::DueAt { date: *date, time: *time }, today),
        }
    }

    fn date(&self) -> Option<NaiveDate> {
        match self {
            DueEdit::At { date, .. } => Some(*date),
            DueEdit::Text(_) => None,
        }
    }

    fn with_date(&self, d: NaiveDate) -> Self {
        match self {
            DueEdit::At { time, recur, .. } => DueEdit::At { date: d, time: *time, recur: recur.clone() },
            DueEdit::Text(_) => DueEdit::At { date: d, time: None, recur: None },
        }
    }
}

/// The editable properties of a task.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Draft {
    pub project_id: String,
    pub section_id: Option<String>,
    pub due: Option<DueEdit>,
    pub deadline: Option<NaiveDate>,
    pub priority: u8,
    pub labels: Vec<String>,
}

impl Draft {
    pub fn from_item(it: &Item) -> Self {
        Self {
            project_id: it.project_id.clone(),
            section_id: it.section_id.clone(),
            due: DueEdit::from_item(it),
            deadline: it.deadline.as_ref().and_then(|d| d.day()),
            priority: it.priority.max(1),
            labels: it.labels.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    Project,
    Date,
    Deadline,
    Priority,
    Labels,
}

/// An open picker, anchored under the control that opened it.
pub struct Pop {
    pub field: Field,
    anchor: Rect,
    query: String,
    time: String,
    month: NaiveDate,
    focus: bool,
    repeat: bool,
}

impl Pop {
    pub fn new(field: Field, anchor: Rect, draft: &Draft, today: NaiveDate) -> Self {
        let base = match field {
            Field::Deadline => draft.deadline,
            _ => draft.due.as_ref().and_then(DueEdit::date),
        }
        .unwrap_or(today);
        let query = match (&draft.due, field) {
            (Some(d), Field::Date) => d.label(today).0,
            _ => String::new(),
        };
        Self { field, anchor, query, time: String::new(), month: base.with_day(1).unwrap_or(base), focus: true, repeat: false }
    }
}

pub enum PopResult {
    Open,
    Changed,
    Close,
}

pub fn priority_name(p: u8) -> String {
    format!("Priority {}", 5 - p.clamp(1, 4))
}

/// Parse "3pm", "3:30 pm", "15:00", "9".
pub fn parse_time(s: &str) -> Option<NaiveTime> {
    let s = s.trim().to_lowercase().replace(' ', "");
    let (body, pm, am) = if let Some(b) = s.strip_suffix("pm") {
        (b.to_string(), true, false)
    } else if let Some(b) = s.strip_suffix("am") {
        (b.to_string(), false, true)
    } else {
        (s.clone(), false, false)
    };
    let (h, m) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (body.parse::<u32>().ok()?, 0),
    };
    let h = match (pm, am, h) {
        (true, _, 12) => 12,
        (true, _, h) => h + 12,
        (_, true, 12) => 0,
        (_, _, h) => h,
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

fn row(ui: &mut Ui, ic: Option<(Icon, Color32)>, text: &str, right: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() || selected {
        p.rect_filled(rect, 4.0, HOVER);
    }
    let mut x = rect.left() + 10.0;
    if let Some((ic, col)) = ic {
        theme::icon(p, Rect::from_center_size(Pos2::new(x + 8.0, rect.center().y), Vec2::splat(16.0)), ic, col);
        x += 26.0;
    }
    p.text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, text, bold_font(13.5), TEXT);
    if !right.is_empty() {
        p.text(Pos2::new(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, right, font(12.5), MUTED);
    }
    if selected {
        theme::icon(p, Rect::from_center_size(Pos2::new(rect.right() - 14.0, rect.center().y), Vec2::splat(14.0)), Icon::Check, TEXT);
    }
    resp
}

fn search(ui: &mut Ui, text: &mut String, hint: &str, focus: &mut bool) -> egui::Response {
    let r = ui.add(egui::TextEdit::singleline(text).hint_text(hint).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(8, 6)));
    if std::mem::take(focus) {
        r.request_focus();
    }
    r
}

/// A month grid. Returns the clicked day.
fn calendar(ui: &mut Ui, store: &Store, month: &mut NaiveDate, selected: Option<NaiveDate>, today: NaiveDate) -> Option<NaiveDate> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(month.format("%b %Y").to_string()).font(bold_font(13.5)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let step = |m: NaiveDate, k: i32| {
                let t = m.year() * 12 + m.month0() as i32 + k;
                NaiveDate::from_ymd_opt(t.div_euclid(12), t.rem_euclid(12) as u32 + 1, 1).unwrap_or(m)
            };
            if widgets::icon_button(ui, Icon::Right, "Next month").clicked() {
                *month = step(*month, 1);
            }
            if widgets::icon_button(ui, Icon::Dot, "This month").clicked() {
                *month = today.with_day(1).unwrap_or(today);
            }
            if widgets::icon_button(ui, Icon::Left, "Previous month").clicked() {
                *month = step(*month, -1);
            }
        });
    });
    let cell = Vec2::new((ui.available_width() / 7.0).floor(), 32.0);
    let (head, _) = ui.allocate_exact_size(Vec2::new(cell.x * 7.0, 20.0), Sense::hover());
    for (i, d) in ["M", "T", "W", "T", "F", "S", "S"].iter().enumerate() {
        ui.painter().text(Pos2::new(head.left() + cell.x * (i as f32 + 0.5), head.center().y), Align2::CENTER_CENTER, *d, font(11.0), MUTED);
    }
    let lead = month.weekday().num_days_from_monday() as i64;
    let first = *month - Duration::days(lead);
    let rows = ((lead + days_in_month(*month) as i64) as f32 / 7.0).ceil() as i64;
    let (grid, _) = ui.allocate_exact_size(Vec2::new(cell.x * 7.0, cell.y * rows as f32), Sense::hover());
    for k in 0..rows * 7 {
        let d = first + Duration::days(k);
        if d.month() != month.month() {
            continue;
        }
        let r = Rect::from_min_size(grid.min + Vec2::new(cell.x * (k % 7) as f32, cell.y * (k / 7) as f32), cell);
        let resp = ui.interact(r, ui.scope_id().with(("cal", d)), Sense::click());
        let c = r.center() - Vec2::new(0.0, 2.0);
        let p = ui.painter();
        let sel = selected == Some(d);
        if sel {
            p.circle_filled(c, 12.0, ACCENT);
        } else if resp.hovered() {
            p.circle_filled(c, 12.0, HOVER);
        }
        let col = if sel { Color32::WHITE } else if d == today { ACCENT_TEXT } else if d < today { FAINT } else { TEXT };
        p.text(c, Align2::CENTER_CENTER, d.day().to_string(), if d == today { bold_font(13.0) } else { font(13.0) }, col);
        if !sel && store.items.values().any(|i| i.due_date() == Some(d)) {
            p.circle_filled(Pos2::new(c.x, r.bottom() - 3.0), 1.5, MUTED);
        }
        if resp.clicked() {
            picked = Some(d);
        }
    }
    picked
}

fn days_in_month(m: NaiveDate) -> u32 {
    let next = if m.month() == 12 { NaiveDate::from_ymd_opt(m.year() + 1, 1, 1) } else { NaiveDate::from_ymd_opt(m.year(), m.month() + 1, 1) };
    next.map(|n| (n - Duration::days(1)).day()).unwrap_or(30)
}

fn next_weekend(today: NaiveDate) -> NaiveDate {
    let mut d = today + Duration::days(1);
    while d.weekday() != Weekday::Sat {
        d += Duration::days(1);
    }
    d
}

fn next_week(today: NaiveDate) -> NaiveDate {
    today + Duration::days(7 - today.weekday().num_days_from_monday() as i64)
}

fn date_body(ui: &mut Ui, store: &Store, pop: &mut Pop, draft: &mut Draft, today: NaiveDate) -> PopResult {
    let mut res = PopResult::Open;
    let r = search(ui, &mut pop.query, "Type a date", &mut pop.focus);
    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        draft.due = DueEdit::from_text(&pop.query);
        return PopResult::Close;
    }
    ui.separator();
    let base = draft.due.clone().unwrap_or(DueEdit::At { date: today, time: None, recur: None });
    let quick = [
        (Icon::Today(today.day()), GREEN, "Today", today.format("%a").to_string(), Some(today)),
        (Icon::Sun, ORANGE, "Tomorrow", (today + Duration::days(1)).format("%a").to_string(), Some(today + Duration::days(1))),
        (Icon::Upcoming, PURPLE, "Next week", next_week(today).format("%a %b %-d").to_string(), Some(next_week(today))),
        (Icon::Sofa, BLUE_SOFT, "Next weekend", next_weekend(today).format("%a %b %-d").to_string(), Some(next_weekend(today))),
        (Icon::Slash, MUTED, "No Date", String::new(), None),
    ];
    for (ic, col, name, right, d) in quick {
        if row(ui, Some((ic, col)), name, &right, false).clicked() {
            draft.due = d.map(|d| base.with_date(d));
            res = PopResult::Close;
        }
    }
    ui.separator();
    if let Some(d) = calendar(ui, store, &mut pop.month, draft.due.as_ref().and_then(DueEdit::date), today) {
        draft.due = Some(base.with_date(d));
        res = PopResult::Changed;
    }
    ui.separator();
    // Time
    ui.horizontal(|ui| {
        theme::icon(ui.painter(), Rect::from_center_size(ui.cursor().min + Vec2::new(8.0, 14.0), Vec2::splat(14.0)), Icon::Clock, MUTED);
        ui.add_space(20.0);
        let current = match &draft.due {
            Some(DueEdit::At { time: Some(t), .. }) => fmt_time(*t),
            _ => String::new(),
        };
        let hint = if current.is_empty() { "Time, e.g. 3pm".to_string() } else { current };
        let r = ui.add(egui::TextEdit::singleline(&mut pop.time).hint_text(hint).desired_width(ui.available_width() - 60.0));
        let set = ui.small_button("Set").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
        if set {
            let t = parse_time(&pop.time);
            if t.is_some() || pop.time.trim().is_empty() {
                if let DueEdit::At { date, recur, .. } = base.clone() {
                    draft.due = Some(DueEdit::At { date, time: t, recur });
                    res = PopResult::Changed;
                }
                pop.time.clear();
            }
        }
    });
    // Repeat
    let d = base.date().unwrap_or(today);
    let time = match &base {
        DueEdit::At { time: Some(t), .. } => format!(" at {}", t.format("%-I:%M%P")),
        _ => String::new(),
    };
    if row(ui, Some((Icon::Repeat, MUTED)), "Repeat", "", pop.repeat).clicked() {
        pop.repeat = !pop.repeat;
    }
    if pop.repeat {
        let opts = [
            ("Every day".to_string(), format!("every day{time}")),
            (format!("Every week on {}", d.format("%A")), format!("every {}{time}", d.format("%A").to_string().to_lowercase())),
            ("Every weekday (Mon - Fri)".to_string(), format!("every workday{time}")),
            (format!("Every month on the {}", ordinal(d.day())), format!("every {}{time}", ordinal(d.day()))),
            (format!("Every year on {}", d.format("%B %-d")), format!("every {}{time}", d.format("%B %-d").to_string().to_lowercase())),
        ];
        for (label, s) in opts {
            ui.horizontal(|ui| {
                ui.add_space(26.0);
                if row(ui, None, &label, "", false).clicked() {
                    draft.due = Some(DueEdit::Text(s));
                    res = PopResult::Close;
                }
            });
        }
    }
    res
}

fn ordinal(n: u32) -> String {
    let suf = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suf}")
}

fn project_body(ui: &mut Ui, store: &Store, pop: &mut Pop, draft: &mut Draft) -> PopResult {
    let mut res = PopResult::Open;
    let r = search(ui, &mut pop.query, "Type a project name", &mut pop.focus);
    let q = pop.query.trim().to_lowercase();
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    ui.separator();
    let inbox = store.inbox_id().unwrap_or("").to_string();
    let mut first: Option<(String, Option<String>)> = None;
    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
        let mut pick = |ui: &mut Ui, ic: (Icon, Color32), name: &str, depth: usize, pid: &str, sid: Option<&str>, first: &mut Option<(String, Option<String>)>| {
            if !q.is_empty() && !name.to_lowercase().contains(&q) {
                return;
            }
            if first.is_none() {
                *first = Some((pid.to_string(), sid.map(String::from)));
            }
            let sel = draft.project_id == pid && draft.section_id.as_deref() == sid;
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 16.0);
                if row(ui, Some(ic), name, "", sel).clicked() {
                    draft.project_id = pid.to_string();
                    draft.section_id = sid.map(String::from);
                    res = PopResult::Close;
                }
            });
        };
        pick(ui, (Icon::Inbox, MUTED), "Inbox", 0, &inbox, None, &mut first);
        if q.is_empty() {
            ui.label(RichText::new("My Projects").font(bold_font(12.5)).color(MUTED));
        }
        for (p, d) in store.project_tree() {
            pick(ui, (Icon::Hash, named_color(&p.color)), &p.name, d, &p.id, None, &mut first);
            for s in store.sections_of(&p.id) {
                pick(ui, (Icon::Section, MUTED), &s.name, d + 1, &p.id, Some(&s.id), &mut first);
            }
        }
    });
    if enter {
        if let Some((p, s)) = first {
            draft.project_id = p;
            draft.section_id = s;
            res = PopResult::Close;
        }
    }
    res
}

fn priority_body(ui: &mut Ui, draft: &mut Draft) -> PopResult {
    let mut res = PopResult::Open;
    for p in [4u8, 3, 2, 1] {
        if row(ui, Some((Icon::Flag, priority_color(p))), &priority_name(p), "", draft.priority == p).clicked() {
            draft.priority = p;
            res = PopResult::Close;
        }
    }
    res
}

fn labels_body(ui: &mut Ui, store: &Store, pop: &mut Pop, draft: &mut Draft) -> PopResult {
    let mut res = PopResult::Open;
    let r = search(ui, &mut pop.query, "Type a label", &mut pop.focus);
    let q = pop.query.trim().to_string();
    ui.separator();
    let mut names: Vec<String> = store.sorted_labels().iter().map(|l| l.name.clone()).collect();
    for l in &draft.labels {
        if !names.contains(l) {
            names.push(l.clone());
        }
    }
    let ql = q.to_lowercase();
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for n in names.iter().filter(|n| ql.is_empty() || n.to_lowercase().contains(&ql)) {
            let on = draft.labels.contains(n);
            let col = store.labels.values().find(|l| &l.name == n).map(|l| named_color(&l.color)).unwrap_or(MUTED);
            if row(ui, Some((Icon::Tag, col)), n, "", on).clicked() {
                if on {
                    draft.labels.retain(|x| x != n);
                } else {
                    draft.labels.push(n.clone());
                }
                res = PopResult::Changed;
            }
        }
        let exists = names.iter().any(|n| n.eq_ignore_ascii_case(&q));
        if !q.is_empty() && !exists && !q.contains(' ') {
            if row(ui, Some((Icon::Plus, MUTED)), &format!("Create \"{q}\""), "", false).clicked() {
                draft.labels.push(q.clone());
                pop.query.clear();
                res = PopResult::Changed;
            }
        }
    });
    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !q.is_empty() {
        if let Some(n) = names.iter().find(|n| n.to_lowercase().contains(&ql)) {
            if !draft.labels.contains(n) {
                draft.labels.push(n.clone());
            }
        } else if !q.contains(' ') {
            draft.labels.push(q.clone());
        }
        pop.query.clear();
        pop.focus = true;
        res = PopResult::Changed;
    }
    res
}

fn deadline_body(ui: &mut Ui, store: &Store, pop: &mut Pop, draft: &mut Draft, today: NaiveDate) -> PopResult {
    let mut res = PopResult::Open;
    if let Some(d) = calendar(ui, store, &mut pop.month, draft.deadline, today) {
        draft.deadline = Some(d);
        res = PopResult::Close;
    }
    if draft.deadline.is_some() {
        ui.separator();
        if row(ui, Some((Icon::Slash, MUTED)), "No deadline", "", false).clicked() {
            draft.deadline = None;
            res = PopResult::Close;
        }
    }
    res
}

/// Show the open picker. A click outside it closes it.
pub fn show(ctx: &egui::Context, store: &Store, pop: &mut Pop, draft: &mut Draft, today: NaiveDate) -> PopResult {
    let width = match pop.field {
        Field::Priority => 200.0,
        Field::Date | Field::Deadline => 260.0,
        _ => 300.0,
    };
    let screen = ctx.content_rect();
    let mut pos = pop.anchor.left_bottom() + Vec2::new(0.0, 4.0);
    pos.x = pos.x.min(screen.right() - width - 24.0);
    let area = egui::Area::new(egui::Id::unique("picker")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::new()
            .fill(POPUP)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(8.0)
            .inner_margin(8.0)
            .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(120) })
            .show(ui, |ui| {
                ui.set_width(width);
                match pop.field {
                    Field::Date => date_body(ui, store, pop, draft, today),
                    Field::Project => project_body(ui, store, pop, draft),
                    Field::Priority => priority_body(ui, draft),
                    Field::Labels => labels_body(ui, store, pop, draft),
                    Field::Deadline => deadline_body(ui, store, pop, draft, today),
                }
            })
            .inner
    });
    let mut res = area.inner;
    // Clicks outside the picker (and outside its own sub-menus) close it.
    let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
        && ctx.pointer_interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !pop.anchor.contains(p))
        ;
    if clicked_outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
        res = PopResult::Close;
    }
    res
}

/// One property row in the task dialog's side panel: label, then value or a "+" to add one.
pub fn side_row(ui: &mut Ui, label: &str, value: Option<(Icon, Color32, String)>, active: bool) -> egui::Response {
    let has = value.is_some();
    let h = if has { 58.0 } else { 44.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    let p = ui.painter();
    if active {
        p.rect_filled(rect.shrink2(Vec2::new(0.0, 2.0)), 6.0, SELECTED);
    } else if resp.hovered() {
        p.rect_filled(rect.shrink2(Vec2::new(0.0, 2.0)), 6.0, HOVER);
    }
    p.text(Pos2::new(rect.left() + 8.0, rect.top() + 14.0), Align2::LEFT_CENTER, label, bold_font(12.5), MUTED);
    match value {
        Some((ic, col, text)) => {
            theme::icon(p, Rect::from_center_size(Pos2::new(rect.left() + 16.0, rect.top() + 40.0), Vec2::splat(14.0)), ic, col);
            p.text(Pos2::new(rect.left() + 32.0, rect.top() + 40.0), Align2::LEFT_CENTER, text, font(13.0), if ic == Icon::Hash || ic == Icon::Inbox || ic == Icon::Flag { TEXT } else { col });
        }
        None => theme::icon(p, Rect::from_center_size(Pos2::new(rect.right() - 14.0, rect.top() + 14.0), Vec2::splat(14.0)), Icon::Plus, MUTED),
    }
    p.line_segment([rect.left_bottom() + Vec2::new(8.0, 0.0), rect.right_bottom() - Vec2::new(8.0, 0.0)], Stroke::new(1.0, LINE));
    resp
}

/// Text and color for showing a project/section choice.
pub fn project_value(store: &Store, d: &Draft) -> (Icon, Color32, String) {
    if store.inbox_id() == Some(d.project_id.as_str()) || d.project_id.is_empty() {
        return (Icon::Inbox, MUTED, "Inbox".into());
    }
    let p = store.projects.get(&d.project_id);
    let mut name = p.map(|p| p.name.clone()).unwrap_or_default();
    if let Some(s) = d.section_id.as_ref().and_then(|s| store.sections.get(s)) {
        name = format!("{name} / {}", s.name);
    }
    (Icon::Hash, p.map(|p| named_color(&p.color)).unwrap_or(MUTED), name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(parse_time("3pm"), NaiveTime::from_hms_opt(15, 0, 0));
        assert_eq!(parse_time("3:30 PM"), NaiveTime::from_hms_opt(15, 30, 0));
        assert_eq!(parse_time("12am"), NaiveTime::from_hms_opt(0, 0, 0));
        assert_eq!(parse_time("18:05"), NaiveTime::from_hms_opt(18, 5, 0));
        assert_eq!(parse_time("x"), None);
    }

    #[test]
    fn due_json_keeps_recurrence() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let v = DueEdit::At { date: d, time: None, recur: Some("every mon".into()) }.to_json();
        assert_eq!(v["string"], "every mon");
        assert_eq!(v["date"], "2026-11-02");
    }
}
