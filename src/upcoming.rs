//! Upcoming: month and week calendars, the day list, and the Plan panel.

use crate::app::{App, Layout};
use crate::model::{Event, Item, Store};
use crate::theme::{Icon, *};
use crate::widgets::{self, task_chip, Action, AddDefaults, EventDrag, RowOpts, TaskDrag};
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, Timelike, Weekday};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

const CHIP_H: f32 = 22.0;
const HOUR_H: f32 = 56.0;

fn week_start(d: NaiveDate, start_day: u8) -> NaiveDate {
    let start = match start_day {
        7 => Weekday::Sun,
        6 => Weekday::Sat,
        _ => Weekday::Mon,
    };
    let back = (7 + d.weekday().num_days_from_monday() as i64 - start.num_days_from_monday() as i64) % 7;
    d - Duration::days(back)
}

fn tasks_on<'a>(store: &'a Store, d: NaiveDate) -> Vec<&'a Item> {
    let mut v: Vec<&Item> = store.items.values().filter(|i| i.due_date() == Some(d)).collect();
    v.sort_by(|a, b| widgets::date_sort(a, b));
    v
}

fn done_on<'a>(store: &'a Store, d: NaiveDate) -> Vec<&'a Item> {
    let mut v: Vec<&Item> = store.completed.values().filter(|i| i.due_date() == Some(d)).collect();
    v.sort_by(|a, b| a.child_order.cmp(&b.child_order));
    v
}

fn events_on(store: &Store, d: NaiveDate) -> Vec<&Event> {
    store.events.iter().filter(|e| if e.all_day { e.start.date() <= d && d < e.end.date().max(e.start.date() + Duration::days(1)) } else { e.start.date() == d }).collect()
}

/// Accept a dragged task on `rect`, scheduling it on `date` (and `time` if given).
fn drop_zone(ui: &Ui, rect: Rect, id: egui::Id, date: NaiveDate, time: Option<NaiveTime>, out: &mut Vec<Action>) {
    let resp = ui.interact(rect, id, Sense::hover());
    if resp.dnd_hover_payload::<TaskDrag>().is_some() || resp.dnd_hover_payload::<EventDrag>().is_some() {
        ui.painter().rect_stroke(rect.shrink(1.0), 4.0, Stroke::new(1.5, ACCENT), egui::StrokeKind::Inside);
    }
    if let Some(p) = resp.dnd_release_payload::<TaskDrag>() {
        out.push(Action::Schedule { id: p.0.clone(), date: Some(date), time: time.map(Some) });
    }
    if let Some(p) = resp.dnd_release_payload::<EventDrag>() {
        out.push(Action::MoveEvent { key: p.0.clone(), date, time });
    }
}

fn drag_preview(ui: &Ui, store: &Store) {
    let ctx = ui.ctx();
    if let (Some(p), Some(pos)) = (egui::DragAndDrop::payload::<TaskDrag>(ctx), ctx.pointer_interact_pos()) {
        if let Some(it) = store.items.get(&p.0) {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("drag")));
            let g = painter.layout_no_wrap(it.content.clone(), font(12.5), TEXT);
            let r = Rect::from_min_size(pos + Vec2::new(12.0, 8.0), g.size() + Vec2::new(16.0, 8.0));
            painter.rect_filled(r, 4.0, chip_fill(it.priority));
            painter.galley(r.min + Vec2::new(8.0, 4.0), g, TEXT);
        }
    }
    if let (Some(p), Some(pos)) = (egui::DragAndDrop::payload::<EventDrag>(ctx), ctx.pointer_interact_pos()) {
        if let Some(e) = store.gevents.iter().find(|e| e.key() == p.0) {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("drag")));
            let g = painter.layout_no_wrap(e.title.clone(), font(12.5), TEXT);
            let r = Rect::from_min_size(pos + Vec2::new(12.0, 8.0), g.size() + Vec2::new(16.0, 8.0));
            painter.rect_filled(r, 4.0, widgets::event_color(e).gamma_multiply(0.7));
            painter.galley(r.min + Vec2::new(8.0, 4.0), g, TEXT);
        }
    }
}

pub fn show(app: &mut App, ui: &mut Ui, out: &mut Vec<Action>) {
    let today = App::today();
    let start_day = app.store.user.start_day;

    // Header: title, month, navigation, Plan toggle.
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Upcoming").font(bold_font(26.0)));
            ui.label(egui::RichText::new(app.anchor.format("%B %Y").to_string()).font(bold_font(14.0)));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
            ui.add_space(14.0);
            if app.google_signed_in() && app.store.gcals.iter().any(|c| c.writable) {
                let d = if app.anchor < today { today } else { app.anchor };
                let d = if app.cfg.layout == Layout::Month && d.month() != today.month() { d.with_day(1).unwrap_or(d) } else { d };
                if ui.button("New event").clicked() {
                    app.event_editor = Some(crate::dialogs::EventEditor::new_on(d, &app.store));
                }
                ui.add_space(6.0);
            }
            let plan_n = plan_items(app).0.len() + plan_items(app).1.len();
            let plan = ui.add(egui::Button::new(format!("Plan: {plan_n}")).selected(app.cfg.plan_open));
            if plan.clicked() {
                app.cfg.plan_open = !app.cfg.plan_open;
                app.save_config();
            }
            ui.add_space(6.0);
            let step = match app.cfg.layout {
                Layout::Month => 0,
                Layout::Week => 7,
                Layout::List => 7,
            };
            let shift = |d: NaiveDate, dir: i32| -> NaiveDate {
                if step == 0 {
                    let m = d.year() * 12 + d.month0() as i32 + dir;
                    NaiveDate::from_ymd_opt(m.div_euclid(12), m.rem_euclid(12) as u32 + 1, 1).unwrap_or(d)
                } else {
                    d + Duration::days(step as i64 * dir as i64)
                }
            };
            if widgets::icon_button(ui, Icon::Right, "Next").clicked() {
                app.anchor = shift(app.anchor, 1);
                app.scroll_week = true;
            }
            if ui.button("Today").clicked() {
                app.anchor = today;
                app.scroll_week = true;
            }
            if widgets::icon_button(ui, Icon::Left, "Previous").clicked() {
                app.anchor = shift(app.anchor, -1);
                app.scroll_week = true;
            }
        });
    });
    ui.add_space(6.0);

    match app.cfg.layout {
        Layout::Month => month(app, ui, today, start_day, out),
        Layout::Week => week(app, ui, today, start_day, out),
        Layout::List => list(app, ui, today, start_day, out),
    }
    drag_preview(ui, &app.store);
}

fn month(app: &mut App, ui: &mut Ui, today: NaiveDate, start_day: u8, out: &mut Vec<Action>) {
    let first = app.anchor.with_day(1).unwrap_or(app.anchor);
    let start = week_start(first, start_day);
    let weeks = 6;
    app.want_completed(start, start + Duration::days(weeks * 7 - 1));

    let full = ui.available_rect_before_wrap();
    let head_h = 24.0;
    let col_w = full.width() / 7.0;
    for i in 0..7 {
        let d = start + Duration::days(i);
        let r = Rect::from_min_size(Pos2::new(full.left() + i as f32 * col_w, full.top()), Vec2::new(col_w, head_h));
        ui.painter().text(r.center(), Align2::CENTER_CENTER, d.format("%a").to_string(), font(13.0), MUTED);
    }
    ui.allocate_rect(Rect::from_min_size(full.min, Vec2::new(full.width(), head_h)), Sense::hover());

    let mut sa = egui::ScrollArea::vertical().id_salt("month").auto_shrink([false, false]);
    if std::mem::take(&mut app.scroll_week) {
        sa = sa.vertical_scroll_offset(0.0);
    }
    let store = &app.store;
    sa.show(ui, |ui| {
        let w = ui.available_width();
        let col_w = w / 7.0;
        let cell_h = ((full.height() - head_h) / weeks as f32).max(128.0);
        let (grid, _) = ui.allocate_exact_size(Vec2::new(w, cell_h * weeks as f32), Sense::hover());
        for wk in 0..weeks {
            for dx in 0..7 {
                let d = start + Duration::days(wk * 7 + dx);
                let cell = Rect::from_min_size(Pos2::new(grid.left() + dx as f32 * col_w, grid.top() + wk as f32 * cell_h), Vec2::new(col_w, cell_h));
                let in_month = d.month() == first.month();
                let p = ui.painter();
                if !in_month {
                    p.rect_filled(cell, 0.0, Color32::from_rgb(0x1b, 0x1b, 0x1b));
                }
                p.line_segment([cell.left_top(), cell.right_top()], Stroke::new(1.0, LINE));
                p.line_segment([cell.right_top(), cell.right_bottom()], Stroke::new(1.0, LINE));
                drop_zone(ui, cell, egui::Id::unique(("mcell", d)), d, None, out);

                // Day number, top right.
                let label = if d.day() == 1 { d.format("%b %-d").to_string() } else { d.day().to_string() };
                let num_pos = Pos2::new(cell.right() - 8.0, cell.top() + 13.0);
                if d == today {
                    let g = p.layout_no_wrap(label, bold_font(13.0), Color32::WHITE);
                    let r = Rect::from_min_size(Pos2::new(num_pos.x - g.size().x - 5.0, num_pos.y - 10.0), g.size() + Vec2::new(10.0, 2.0));
                    p.rect_filled(r, 4.0, ACCENT);
                    p.galley(r.min + Vec2::new(5.0, 1.0), g, Color32::WHITE);
                } else {
                    p.text(num_pos, Align2::RIGHT_CENTER, label, font(13.0), if in_month { TEXT } else { FAINT });
                }

                // Chips: completed, open tasks, then events.
                let done = done_on(store, d);
                let open = tasks_on(store, d);
                let evs = events_on(store, d);
                let total = done.len() + open.len() + evs.len();
                let room = ((cell_h - 30.0) / (CHIP_H + 2.0)).floor().max(1.0) as usize;
                let shown = if total > room { room - 1 } else { total };
                let mut y = cell.top() + 26.0;
                let chip = |y: f32| Rect::from_min_size(Pos2::new(cell.left() + 4.0, y), Vec2::new(col_w - 8.0, CHIP_H));
                let mut n = 0;
                for it in done.iter().chain(open.iter()) {
                    if n >= shown {
                        break;
                    }
                    let is_done = it.checked || store.completed.contains_key(&it.id) && !store.items.contains_key(&it.id);
                    task_chip(ui, chip(y), it, is_done, true, today, out);
                    y += CHIP_H + 2.0;
                    n += 1;
                }
                for e in evs {
                    if n >= shown {
                        break;
                    }
                    widgets::event_chip(ui, chip(y), e, out);
                    y += CHIP_H + 2.0;
                    n += 1;
                }
                if total > shown {
                    let r = chip(y);
                    let resp = ui.interact(r, egui::Id::unique(("more", d)), Sense::click());
                    ui.painter().text(Pos2::new(r.left() + 4.0, r.center().y), Align2::LEFT_CENTER, format!("{} more", total - shown), bold_font(12.5), if resp.hovered() { ACCENT_TEXT } else { TEXT });
                    if resp.clicked() {
                        out.push(Action::ShowWeek(d));
                    }
                }
                // Double-click an empty spot to add a task on that day.
                let bg = ui.interact(cell, egui::Id::unique(("mbg", d)), Sense::click());
                if bg.double_clicked() {
                    out.push(Action::AddTask(AddDefaults { due: Some(d), ..Default::default() }));
                }
            }
        }
    });
}

fn lanes(spans: &[(f32, f32)]) -> (Vec<usize>, usize) {
    let mut ends: Vec<f32> = Vec::new();
    let mut lane = Vec::with_capacity(spans.len());
    for &(s, e) in spans {
        match ends.iter().position(|&end| end <= s + 0.5) {
            Some(i) => {
                ends[i] = e;
                lane.push(i);
            }
            None => {
                ends.push(e);
                lane.push(ends.len() - 1);
            }
        }
    }
    (lane, ends.len().max(1))
}

fn week(app: &mut App, ui: &mut Ui, today: NaiveDate, start_day: u8, out: &mut Vec<Action>) {
    let start = week_start(app.anchor, start_day);
    app.want_completed(start, start + Duration::days(6));
    let scroll_now = std::mem::take(&mut app.scroll_week);
    let store = &app.store;
    let gutter = 56.0;
    let full = ui.available_rect_before_wrap();
    let col_w = (full.width() - gutter - 12.0) / 7.0;
    let col_x = |i: i64| full.left() + gutter + i as f32 * col_w;

    // Day headers.
    let head_h = 26.0;
    for i in 0..7 {
        let d = start + Duration::days(i);
        let r = Rect::from_min_size(Pos2::new(col_x(i), full.top()), Vec2::new(col_w, head_h));
        let txt = format!("{} {}", d.format("%a"), d.day());
        if d == today {
            let g = ui.painter().layout_no_wrap(txt, bold_font(13.0), Color32::WHITE);
            let br = Rect::from_center_size(r.center(), g.size() + Vec2::new(12.0, 4.0));
            ui.painter().rect_filled(br, 4.0, ACCENT);
            ui.painter().galley(br.min + Vec2::new(6.0, 2.0), g, Color32::WHITE);
        } else {
            ui.painter().text(r.center(), Align2::CENTER_CENTER, txt, font(13.0), MUTED);
        }
    }

    // All-day row.
    let all_day: Vec<(Vec<&Item>, Vec<&Item>, Vec<&Event>)> = (0..7)
        .map(|i| {
            let d = start + Duration::days(i);
            let open = tasks_on(store, d).into_iter().filter(|t| t.due_at().and_then(|a| a.time).is_none()).collect();
            let done = done_on(store, d).into_iter().filter(|t| t.due_at().and_then(|a| a.time).is_none()).collect();
            let ev = events_on(store, d).into_iter().filter(|e| e.all_day).collect();
            (open, done, ev)
        })
        .collect();
    let rows = all_day.iter().map(|(a, b, c)| a.len() + b.len() + c.len()).max().unwrap_or(0).clamp(1, 6);
    let ad_h = rows as f32 * (CHIP_H + 2.0) + 6.0;
    let ad_top = full.top() + head_h;
    ui.painter().text(Pos2::new(full.left() + gutter - 8.0, ad_top + 12.0), Align2::RIGHT_CENTER, "All day", font(11.5), MUTED);
    for i in 0..7 {
        let d = start + Duration::days(i);
        let cell = Rect::from_min_size(Pos2::new(col_x(i), ad_top), Vec2::new(col_w, ad_h));
        drop_zone(ui, cell, egui::Id::unique(("adcell", d)), d, None, out);
        let mut y = ad_top + 2.0;
        let (open, done, ev) = &all_day[i as usize];
        let chip = |y: f32| Rect::from_min_size(Pos2::new(cell.left() + 2.0, y), Vec2::new(col_w - 4.0, CHIP_H));
        for it in done.iter().take(rows) {
            task_chip(ui, chip(y), it, true, false, today, out);
            y += CHIP_H + 2.0;
        }
        for it in open {
            if y + CHIP_H > ad_top + ad_h {
                break;
            }
            task_chip(ui, chip(y), it, false, false, today, out);
            y += CHIP_H + 2.0;
        }
        for e in ev {
            if y + CHIP_H > ad_top + ad_h {
                break;
            }
            widgets::event_chip(ui, chip(y), e, out);
            y += CHIP_H + 2.0;
        }
    }
    let used = head_h + ad_h;
    ui.painter().line_segment([Pos2::new(full.left(), full.top() + used), Pos2::new(full.right(), full.top() + used)], Stroke::new(1.0, LINE));
    ui.allocate_rect(Rect::from_min_size(full.min, Vec2::new(full.width(), used + 1.0)), Sense::hover());

    let mut sa = egui::ScrollArea::vertical().id_salt("week").auto_shrink([false, false]);
    if scroll_now {
        let now = Local::now().time();
        sa = sa.vertical_scroll_offset(((now.hour() as f32 - 2.0).max(0.0) * HOUR_H).min(16.0 * HOUR_H));
    }
    sa.show(ui, |ui| {
        let (grid, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0 * HOUR_H), Sense::hover());
        let p = ui.painter().clone();
        for h in 0..24 {
            let y = grid.top() + h as f32 * HOUR_H;
            p.line_segment([Pos2::new(grid.left() + gutter, y), Pos2::new(grid.right(), y)], Stroke::new(1.0, LINE));
            if h > 0 {
                let t = NaiveTime::from_hms_opt(h, 0, 0).unwrap();
                p.text(Pos2::new(grid.left() + gutter - 8.0, y), Align2::RIGHT_CENTER, widgets::fmt_time(t), font(11.5), MUTED);
            }
        }
        for i in 0..7 {
            let x = grid.left() + gutter + i as f32 * col_w;
            p.line_segment([Pos2::new(x, grid.top()), Pos2::new(x, grid.bottom())], Stroke::new(1.0, LINE));
        }
        for i in 0..7 {
            let d = start + Duration::days(i);
            let x = grid.left() + gutter + i as f32 * col_w;
            // Drop zones per 30 minutes.
            for slot in 0..48 {
                let r = Rect::from_min_size(Pos2::new(x, grid.top() + slot as f32 * HOUR_H / 2.0), Vec2::new(col_w, HOUR_H / 2.0));
                let t = NaiveTime::from_hms_opt(slot / 2, (slot % 2) * 30, 0).unwrap();
                drop_zone(ui, r, egui::Id::unique(("slot", d, slot)), d, Some(t), out);
            }
            let y_of = |t: NaiveTime| grid.top() + (t.num_seconds_from_midnight() as f32 / 3600.0) * HOUR_H;

            enum Block<'a> {
                Task(&'a Item, bool),
                Ev(&'a Event),
            }
            let mut blocks: Vec<(f32, f32, Block)> = Vec::new();
            for (it, done) in tasks_on(store, d).into_iter().map(|t| (t, false)).chain(done_on(store, d).into_iter().map(|t| (t, true))) {
                if let Some(t) = it.due_at().and_then(|a| a.time) {
                    let mins = it.duration.as_ref().map(|d| d.minutes()).unwrap_or(30).max(15);
                    let y0 = y_of(t);
                    blocks.push((y0, (y0 + mins as f32 / 60.0 * HOUR_H).min(grid.bottom()), Block::Task(it, done)));
                }
            }
            for e in events_on(store, d).into_iter().filter(|e| !e.all_day) {
                let y0 = y_of(e.start.time());
                let y1 = if e.end.date() > d { grid.bottom() } else { y_of(e.end.time()) };
                blocks.push((y0, y1.max(y0 + HOUR_H / 4.0), Block::Ev(e)));
            }
            blocks.sort_by(|a, b| a.0.total_cmp(&b.0));
            let spans: Vec<(f32, f32)> = blocks.iter().map(|b| (b.0, b.1)).collect();
            let (lane, n) = lanes(&spans);
            let lw = (col_w - 6.0) / n as f32;
            for (k, (y0, y1, b)) in blocks.iter().enumerate() {
                let r = Rect::from_min_max(Pos2::new(x + 3.0 + lane[k] as f32 * lw, *y0 + 1.0), Pos2::new(x + 3.0 + (lane[k] + 1) as f32 * lw - 2.0, *y1 - 1.0));
                match b {
                    Block::Task(it, done) => {
                        if r.height() < CHIP_H + 8.0 {
                            task_chip(ui, Rect::from_min_size(r.min, Vec2::new(r.width(), r.height().max(CHIP_H))), it, *done, true, today, out);
                        } else {
                            task_chip(ui, Rect::from_min_size(r.min, Vec2::new(r.width(), CHIP_H)), it, *done, true, today, out);
                            ui.painter().rect_filled(Rect::from_min_max(r.min + Vec2::new(0.0, CHIP_H), r.max), egui::CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 }, chip_fill(it.priority).gamma_multiply(if *done { 0.35 } else { 0.7 }));
                        }
                    }
                    Block::Ev(e) => {
                        let painter = ui.painter_at(r);
                        painter.rect_filled(r, 4.0, Color32::from_rgb(0x2b, 0x2b, 0x2b));
                        painter.rect_filled(Rect::from_min_size(r.min + Vec2::new(3.0, 3.0), Vec2::new(2.5, r.height() - 6.0)), 1.0, widgets::event_color(e));
                        let g = painter.layout(e.title.clone(), bold_font(12.0), TEXT, r.width() - 14.0);
                        let th = g.size().y;
                        painter.galley(r.min + Vec2::new(10.0, 4.0), g, TEXT);
                        if r.height() > th + 22.0 {
                            painter.text(r.min + Vec2::new(10.0, 6.0 + th), Align2::LEFT_TOP, format!("{}–{}", widgets::fmt_time(e.start.time()), widgets::fmt_time(e.end.time())), font(11.5), MUTED);
                        }
                        widgets::event_interact(ui, r, e, out);
                    }
                }
            }
            if d == today {
                let y = y_of(Local::now().time());
                p.line_segment([Pos2::new(x, y), Pos2::new(x + col_w, y)], Stroke::new(2.0, ACCENT));
                p.circle_filled(Pos2::new(x, y), 4.0, ACCENT);
            }
        }
    });
    ui.ctx().request_repaint_after(std::time::Duration::from_secs(60));
}

fn list(app: &mut App, ui: &mut Ui, today: NaiveDate, start_day: u8, out: &mut Vec<Action>) {
    let start = week_start(app.anchor, start_day).max(today);
    let days = 21;
    // Today, Previous, and Next jump back to the top of the list.
    let mut sa = egui::ScrollArea::vertical().id_salt("ulist").auto_shrink([false, false]);
    if std::mem::take(&mut app.scroll_week) {
        sa = sa.vertical_scroll_offset(0.0);
    }
    let store = &app.store;
    sa.show(ui, |ui| {
        let w = ui.available_width();
        let cw = (w - 64.0).min(820.0);
        let pad = ((w - cw) / 2.0).max(16.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(cw);
                if start == today {
                    let mut overdue: Vec<&Item> = store.items.values().filter(|i| i.due_date().is_some_and(|d| d < today)).collect();
                    overdue.sort_by(|a, b| widgets::date_sort(a, b));
                    if !overdue.is_empty() {
                        widgets::group_header(ui, "Overdue", Some(overdue.len()));
                        for it in overdue {
                            widgets::task_row(ui, store, it, today, &RowOpts::default(), out);
                        }
                    }
                }
                for i in 0..days {
                    let d = start + Duration::days(i);
                    let rel = match (d - today).num_days() {
                        0 => " · Today",
                        1 => " · Tomorrow",
                        _ => "",
                    };
                    let title = format!("{}{} · {}", d.format("%b %-d"), rel, d.format("%A"));
                    let items = tasks_on(store, d);
                    widgets::group_header(ui, &title, None);
                    let zone_top = ui.cursor().top();
                    for e in events_on(store, d) {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
                        widgets::event_chip(ui, r, e, out);
                    }
                    for it in &items {
                        widgets::task_row(ui, store, it, today, &RowOpts { show_due: it.due_at().and_then(|a| a.time).is_some(), ..Default::default() }, out);
                    }
                    widgets::add_row(ui, AddDefaults { due: Some(d), ..Default::default() }, out);
                    let zone = Rect::from_min_max(Pos2::new(ui.min_rect().left(), zone_top - 28.0), Pos2::new(ui.min_rect().right(), ui.cursor().top()));
                    drop_zone(ui, zone, egui::Id::unique(("lday", d)), d, None, out);
                }
                ui.add_space(80.0);
            });
        });
    });
}

/// Tasks for the Plan panel: (dated without a time in the next 7 days, deadline but no date).
pub fn plan_items(app: &App) -> (Vec<&Item>, Vec<&Item>) {
    let today = App::today();
    let keep = |i: &&Item| {
        app.plan_project.as_ref().is_none_or(|p| &i.project_id == p || app.store.projects.get(&i.project_id).and_then(|x| x.parent_id.as_ref()) == Some(p))
            && app.plan_label.as_ref().is_none_or(|l| i.labels.contains(l))
    };
    let mut all_day: Vec<&Item> = app
        .store
        .items
        .values()
        .filter(|i| i.due_at().is_some_and(|a| a.time.is_none() && a.date >= today && a.date < today + Duration::days(7)))
        .filter(keep)
        .collect();
    all_day.sort_by(|a, b| widgets::date_sort(a, b));
    let mut unscheduled: Vec<&Item> = app.store.items.values().filter(|i| i.due.is_none() && i.deadline.is_some()).filter(keep).collect();
    unscheduled.sort_by_key(|i| i.deadline.as_ref().map(|d| d.date.clone()));
    (all_day, unscheduled)
}

pub fn plan_panel(app: &mut App, ui: &mut Ui, out: &mut Vec<Action>) {
    let today = App::today();
    let (a, b) = plan_items(app);
    let (a, b): (Vec<Item>, Vec<Item>) = (a.into_iter().cloned().collect(), b.into_iter().cloned().collect());
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Plan").font(bold_font(17.0)));
        ui.label(egui::RichText::new((a.len() + b.len()).to_string()).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, Icon::Panel, "Close Plan").clicked() {
                app.cfg.plan_open = false;
                app.save_config();
            }
        });
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let pname = app.plan_project.as_ref().map(|p| app.store.project_name(p).to_string()).unwrap_or_else(|| "All projects".into());
        egui::ComboBox::from_id_salt("plan_p").selected_text(format!("# {pname}")).show_ui(ui, |ui| {
            ui.selectable_value(&mut app.plan_project, None, "All projects");
            for (p, d) in app.store.project_tree() {
                ui.selectable_value(&mut app.plan_project, Some(p.id.clone()), format!("{}{}", "   ".repeat(d), p.name));
            }
        });
        let lname = app.plan_label.clone().unwrap_or_else(|| "All labels".into());
        egui::ComboBox::from_id_salt("plan_l").selected_text(lname).show_ui(ui, |ui| {
            ui.selectable_value(&mut app.plan_label, None, "All labels");
            for l in app.store.sorted_labels() {
                ui.selectable_value(&mut app.plan_label, Some(l.name.clone()), &l.name);
            }
        });
    });
    ui.add_space(6.0);
    let store = &app.store;
    egui::ScrollArea::vertical().id_salt("plan").auto_shrink([false, false]).show(ui, |ui| {
        egui::CollapsingHeader::new(egui::RichText::new(format!("All-day (next 7 days)  {}", a.len())).font(bold_font(14.0))).default_open(true).show(ui, |ui| {
            for it in &a {
                widgets::task_row(ui, store, it, today, &RowOpts { compact: true, ..Default::default() }, out);
            }
            widgets::add_row(ui, AddDefaults { due: Some(today), ..Default::default() }, out);
        });
        ui.add_space(8.0);
        egui::CollapsingHeader::new(egui::RichText::new(format!("Unscheduled with deadline  {}", b.len())).font(bold_font(14.0))).default_open(true).show(ui, |ui| {
            for it in &b {
                widgets::task_row(ui, store, it, today, &RowOpts { compact: true, ..Default::default() }, out);
            }
        });
    });
}
