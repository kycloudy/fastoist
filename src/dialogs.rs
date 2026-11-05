//! Task dialog and quick add.

use crate::api::AddExtras;
use crate::app::App;
use crate::model::{Deadline, Due, Event, Item, Store};
use crate::pickers::{self, DueEdit, Draft, Field, Pop, PopResult};
use crate::theme::{self, Icon, *};
use crate::widgets::{self, Action, AddDefaults, RowOpts};
use eframe::egui::{self, Align2, Color32, Key, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde_json::{json, Map, Value};

/// Commands that turn `old` into `new` for task `id`: (item_update args, item_move args).
fn draft_commands(id: &str, old: &Draft, new: &Draft) -> (Map<String, Value>, Option<Value>) {
    let mut args = Map::new();
    args.insert("id".into(), json!(id));
    if new.due != old.due {
        args.insert("due".into(), new.due.as_ref().map(DueEdit::to_json).unwrap_or(Value::Null));
    }
    if new.deadline != old.deadline {
        args.insert("deadline".into(), new.deadline.map(|d| json!({ "date": d.format("%Y-%m-%d").to_string() })).unwrap_or(Value::Null));
    }
    if new.priority != old.priority {
        args.insert("priority".into(), json!(new.priority));
    }
    if new.labels != old.labels {
        args.insert("labels".into(), json!(new.labels));
    }
    let mv = if new.section_id != old.section_id && new.section_id.is_some() {
        Some(json!({ "id": id, "section_id": new.section_id }))
    } else if new.project_id != old.project_id || new.section_id != old.section_id {
        Some(json!({ "id": id, "project_id": new.project_id }))
    } else {
        None
    };
    (args, mv)
}

/// Show a draft on the local copy right away, before Todoist confirms it.
fn apply_draft(it: &mut Item, d: &Draft) {
    it.project_id = d.project_id.clone();
    it.section_id = d.section_id.clone();
    it.priority = d.priority;
    it.labels = d.labels.clone();
    it.deadline = d.deadline.map(|d| Deadline { date: d.format("%Y-%m-%d").to_string() });
    match &d.due {
        None => it.due = None,
        Some(DueEdit::At { recur, .. }) => {
            let v = d.due.as_ref().unwrap().to_json();
            let mut due = Due { date: v["date"].as_str().unwrap_or_default().to_string(), ..Default::default() };
            if let Some(r) = recur {
                due.string = r.clone();
                due.is_recurring = true;
            }
            it.due = Some(due);
        }
        // Todoist parses text dates; the next sync shows the result.
        Some(DueEdit::Text(_)) => {}
    }
}

pub struct Editor {
    id: String,
    content: String,
    description: String,
    saved_text: (String, String),
    draft: Draft,
    saved: Draft,
    new_sub: String,
    adding_sub: bool,
    pop: Option<Pop>,
}

impl Editor {
    pub fn new(it: &Item, store: &Store) -> Self {
        let draft = Draft::load(it, store);
        Self {
            id: it.id.clone(),
            content: it.content.clone(),
            description: it.description.clone(),
            saved_text: (it.content.clone(), it.description.clone()),
            saved: draft.clone(),
            draft,
            new_sub: String::new(),
            adding_sub: false,
            pop: None,
        }
    }

    /// Send whatever changed since the last save. Todoist's dialog saves as you go.
    fn save(&mut self, app: &mut App) {
        let (mut args, mv) = draft_commands(&self.id, &self.saved, &self.draft);
        let content = self.content.trim().to_string();
        if !content.is_empty() && content != self.saved_text.0 {
            args.insert("content".into(), json!(content));
        }
        if self.description != self.saved_text.1 {
            args.insert("description".into(), json!(self.description));
        }
        let rem = pickers::reminder_commands(&self.id, &self.saved.reminders, &mut self.draft.reminders);
        if !rem.is_empty() {
            app.store.reminders.retain(|_, r| r.item_id != self.id || self.draft.reminders.iter().any(|d| d.id.as_ref() == Some(&r.id)) || r.kind == "location");
            for d in &self.draft.reminders {
                let id = d.id.clone().unwrap_or_default();
                app.store.reminders.entry(id.clone()).or_insert_with(|| d.to_reminder(&id, &self.id));
            }
            app.send(crate::api::Job::Commands(rem));
            self.saved.reminders = self.draft.reminders.clone();
        }
        if args.len() == 1 && mv.is_none() {
            return;
        }
        if let Some(it) = app.store.items.get_mut(&self.id) {
            apply_draft(it, &self.draft);
            if !content.is_empty() {
                it.content = content.clone();
            }
            it.description = self.description.clone();
        }
        app.update_task(&self.id, args, mv);
        self.saved = self.draft.clone();
        self.saved_text = (if content.is_empty() { self.saved_text.0.clone() } else { content }, self.description.clone());
    }
}

fn open_pop(slot: &mut Option<Pop>, field: Field, anchor: Rect, draft: &Draft) {
    if slot.as_ref().is_some_and(|p| p.field == field) {
        *slot = None;
    } else {
        *slot = Some(Pop::new(field, anchor, draft, App::today()));
    }
}

fn editor_ui(app: &mut App, ctx: &egui::Context, out: &mut Vec<Action>) {
    let today = App::today();
    let Some(mut ed) = app.editor.take() else { return };
    let Some(item) = app.store.items.get(&ed.id).cloned() else { return };
    let mut close = false;
    let screen = ctx.content_rect();
    let w = (screen.width() - 80.0).min(880.0);
    let h = (screen.height() - 120.0).min(640.0);
    let side_w = 260.0;

    let modal = egui::Modal::new(egui::Id::unique("editor"))
        .frame(egui::Frame::new().fill(bg()).corner_radius(10.0).stroke(Stroke::new(1.0, line())))
        .show(ctx, |ui| {
            ui.set_width(w);
            ui.set_height(h);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let store = &app.store;

            // Header: project, close.
            let (head, _) = ui.allocate_exact_size(Vec2::new(w, 46.0), Sense::hover());
            let (ic, col, name) = pickers::project_value(store, &Draft::from_item(&item));
            theme::icon(ui.painter(), Rect::from_center_size(Pos2::new(head.left() + 24.0, head.center().y), Vec2::splat(14.0)), ic, col);
            ui.painter().text(Pos2::new(head.left() + 38.0, head.center().y), Align2::LEFT_CENTER, name, bold_font(13.0), text());
            let xr = Rect::from_center_size(Pos2::new(head.right() - 26.0, head.center().y), Vec2::splat(28.0));
            let xresp = ui.interact(xr, ui.scope_id().with("close"), Sense::click());
            if xresp.hovered() {
                ui.painter().rect_filled(xr, 6.0, hover());
            }
            theme::icon(ui.painter(), xr, Icon::Close, muted());
            if xresp.on_hover_text("Close (Esc)").clicked() {
                close = true;
            }
            ui.painter().line_segment([head.left_bottom(), head.right_bottom()], Stroke::new(1.0, line()));

            let body = Rect::from_min_size(Pos2::new(head.left(), head.bottom()), Vec2::new(w, h - 46.0));
            let side = Rect::from_min_max(Pos2::new(body.right() - side_w, body.top()), body.max);
            ui.painter().rect_filled(side, egui::CornerRadius { nw: 0, ne: 0, sw: 0, se: 10 }, sidebar());

            // Left: title, description, sub-tasks.
            let left = Rect::from_min_max(body.min + Vec2::new(20.0, 16.0), Pos2::new(side.left() - 20.0, body.bottom() - 10.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                egui::ScrollArea::vertical().id_salt("ed_left").auto_shrink([false, false]).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(26.0, 30.0), Sense::click());
                        theme::check_circle(ui.painter(), r.center(), 10.0, ed.draft.priority, resp.hovered(), false);
                        if resp.on_hover_text("Complete").clicked() {
                            out.push(Action::Complete(ed.id.clone()));
                            close = true;
                        }
                        ui.add(egui::TextEdit::multiline(&mut ed.content).font(bold_font(20.0)).desired_rows(1).desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(egui::Margin::symmetric(0, 2)));
                    });
                    ui.horizontal_top(|ui| {
                        ui.add_space(34.0);
                        ui.add(egui::TextEdit::multiline(&mut ed.description).hint_text("Description").desired_rows(1).desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(egui::Margin::ZERO));
                    });
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        ui.vertical(|ui| {
                            let subs: Vec<Item> = store.subtasks(&ed.id).into_iter().cloned().collect();
                            if !subs.is_empty() {
                                ui.label(RichText::new(format!("Sub-tasks  {}", subs.len())).font(bold_font(13.0)));
                                for s in &subs {
                                    widgets::task_row(ui, store, s, today, &RowOpts { show_project: false, compact: true, ..Default::default() }, out);
                                }
                            }
                            if ed.adding_sub {
                                let r = ui.add(egui::TextEdit::singleline(&mut ed.new_sub).hint_text("Sub-task name").desired_width(f32::INFINITY).margin(egui::Margin::symmetric(8, 6)));
                                r.request_focus();
                                if r.lost_focus() {
                                    if ui.input(|i| i.key_pressed(Key::Enter)) && !ed.new_sub.trim().is_empty() {
                                        let content = std::mem::take(&mut ed.new_sub).trim().to_string();
                                        out.push(Action::AddSubtask { parent: ed.id.clone(), content });
                                    } else {
                                        ed.adding_sub = false;
                                    }
                                }
                            } else {
                                let (r, resp) = ui.allocate_exact_size(Vec2::new(140.0, 30.0), Sense::click());
                                let col = if resp.hovered() { accent_text() } else { muted() };
                                theme::icon(ui.painter(), Rect::from_center_size(Pos2::new(r.left() + 8.0, r.center().y), Vec2::splat(12.0)), Icon::Plus, col);
                                ui.painter().text(Pos2::new(r.left() + 22.0, r.center().y), Align2::LEFT_CENTER, "Add sub-task", bold_font(13.0), col);
                                if resp.clicked() {
                                    ed.adding_sub = true;
                                }
                            }
                        });
                    });
                });
            });

            // Right: properties.
            let inner = side.shrink2(Vec2::new(16.0, 14.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                let d = &ed.draft;
                let open = ed.pop.as_ref().map(|p| p.field);
                let rows = [
                    (Field::Project, "Project", Some(pickers::project_value(store, d))),
                    (Field::Date, "Date", d.due.as_ref().map(|x| { let (t, c) = x.label(today); (Icon::Calendar, c, t) })),
                    (Field::Deadline, "Deadline", d.deadline.map(|x| { let (t, c) = widgets::deadline_label(x, today); (Icon::Deadline, c, t) })),
                    (Field::Reminders, "Reminders", pickers::reminders_text(&d.reminders).map(|t| (Icon::Alarm, text(), t))),
                    (Field::Priority, "Priority", Some((Icon::Flag, priority_color(d.priority), format!("P{}", 5 - d.priority)))),
                    (Field::Labels, "Labels", (!d.labels.is_empty()).then(|| (Icon::Tag, muted(), d.labels.join(", ")))),
                ];
                let mut clicked = None;
                for (f, label, value) in rows {
                    let r = pickers::side_row(ui, label, value, open == Some(f));
                    if r.clicked() {
                        clicked = Some((f, r.rect));
                    }
                }
                if let Some((f, rect)) = clicked {
                    open_pop(&mut ed.pop, f, rect, &ed.draft);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Delete task").color(RED)).frame(false)).clicked() {
                        out.push(Action::Delete(ed.id.clone()));
                        close = true;
                    }
                });
            });
        });

    if let Some(pop) = ed.pop.as_mut() {
        match pickers::show(ctx, &app.store, pop, &mut ed.draft, today) {
            PopResult::Open => {}
            PopResult::Changed => ed.save(app),
            PopResult::Close => {
                ed.pop = None;
                ed.save(app);
            }
        }
    } else if modal.should_close() {
        close = true;
    }
    if close {
        ed.save(app);
    } else {
        app.editor = Some(ed);
    }
}

pub struct Adder {
    text: String,
    description: String,
    draft: Draft,
    parent_id: Option<String>,
    menu: bool,
    pop: Option<Pop>,
    focus: bool,
}

impl Adder {
    pub fn new(d: AddDefaults, store: &Store) -> Self {
        Self {
            text: String::new(),
            description: String::new(),
            draft: Draft {
                project_id: d.project_id.unwrap_or_else(|| store.inbox_id().unwrap_or("").to_string()),
                section_id: d.section_id,
                due: d.due.map(|date| DueEdit::At { date, time: None, recur: None }),
                deadline: None,
                priority: 1,
                labels: d.labels,
                reminders: Vec::new(),
            },
            parent_id: d.parent_id,
            menu: false,
            pop: None,
            focus: true,
        }
    }
}

/// A rounded property button, like Todoist's quick add. A set property shows its value and a clear button.
/// Returns the button's response and whether the clear button was pressed.
fn chip(ui: &mut Ui, ic: Icon, col: Color32, text: &str, clear: bool) -> (egui::Response, bool) {
    let g = ui.painter().layout_no_wrap(text.to_string(), font(12.5), col);
    let size = Vec2::new(g.size().x + if clear { 50.0 } else { 34.0 }, 28.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let x = Rect::from_center_size(Pos2::new(r.right() - 14.0, r.center().y), Vec2::splat(16.0));
    let over_x = clear && resp.hover_pos().is_some_and(|p| x.contains(p));
    let p = ui.painter();
    p.rect(r, 6.0, if resp.hovered() && !over_x { hover() } else { popup() }, Stroke::new(1.0, line()), egui::StrokeKind::Inside);
    theme::icon(p, Rect::from_center_size(Pos2::new(r.left() + 14.0, r.center().y), Vec2::splat(14.0)), ic, col);
    p.galley(Pos2::new(r.left() + 26.0, r.center().y - g.size().y / 2.0), g, col);
    if clear {
        if over_x {
            p.rect_filled(x, 3.0, hover());
        }
        theme::icon(p, x.shrink(2.0), Icon::Close, if over_x { theme::text() } else { muted() });
    }
    let cleared = resp.clicked() && over_x;
    (resp, cleared)
}

fn menu_row(ui: &mut Ui, ic: Icon, name: &str, key: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, hover());
    }
    theme::icon(ui.painter(), Rect::from_center_size(Pos2::new(r.left() + 16.0, r.center().y), Vec2::splat(16.0)), ic, muted());
    ui.painter().text(Pos2::new(r.left() + 36.0, r.center().y), Align2::LEFT_CENTER, name, font(13.5), text());
    if !key.is_empty() {
        let kr = Rect::from_center_size(Pos2::new(r.right() - 16.0, r.center().y), Vec2::new(16.0, 16.0));
        ui.painter().rect_filled(kr, 3.0, raised());
        ui.painter().text(kr.center(), Align2::CENTER_CENTER, key, bold_font(10.5), muted());
    }
    resp.clicked()
}

fn adder_ui(app: &mut App, ctx: &egui::Context) {
    let today = App::today();
    let Some(mut ad) = app.adder.take() else { return };
    let mut close = false;
    let mut submit = false;
    let screen = ctx.content_rect();
    let w = (screen.width() - 80.0).min(600.0);
    let pos = Pos2::new(screen.center().x - w / 2.0 + 60.0, screen.top() + 100.0);
    let mut plus_rect = Rect::NOTHING;
    let mut open: Option<(Field, Rect)> = None;

    let area = egui::Area::new(egui::Id::unique("adder")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::new()
            .fill(popup())
            .stroke(Stroke::new(1.0, line()))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .shadow(egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(140) })
            .show(ui, |ui| {
                ui.set_width(w);
                let r = ui.add(egui::TextEdit::singleline(&mut ad.text).hint_text("Task name").font(bold_font(16.0)).frame(egui::Frame::NONE).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(4, 4)));
                if std::mem::take(&mut ad.focus) {
                    r.request_focus();
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    submit = true;
                }
                ui.add(egui::TextEdit::multiline(&mut ad.description).hint_text("Description").frame(egui::Frame::NONE).desired_rows(1).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(4, 0)));
                ui.add_space(6.0);
                // Property buttons. Each opens its picker; a set one shows its value and a clear button.
                let d = ad.draft.clone();
                let mut clear: Option<Field> = None;
                ui.horizontal(|ui| {
                    let actions_w = 76.0;
                    let chips_w = ui.available_width() - actions_w;
                    ui.allocate_ui(Vec2::new(chips_w, 0.0), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                            let (r, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                            plus_rect = r;
                            ui.painter().rect_filled(r, 6.0, if resp.hovered() || ad.menu { raised_hover() } else { raised() });
                            theme::icon(ui.painter(), r, Icon::Plus, text());
                            if resp.on_hover_text("More").clicked() {
                                ad.menu = !ad.menu;
                                ad.pop = None;
                            }
                            let mut put = |ui: &mut Ui, f: Field, ic: Icon, col: Color32, text: &str, set: bool| {
                                let (r, x) = chip(ui, ic, if set { col } else { muted() }, text, set);
                                if x {
                                    clear = Some(f);
                                } else if r.clicked() {
                                    open = Some((f, r.rect));
                                }
                            };
                            let (ic, col, name) = pickers::project_value(&app.store, &d);
                            put(ui, Field::Project, ic, col, &name, false);
                            match &d.due {
                                Some(due) => {
                                    let (t, c) = due.label(today);
                                    put(ui, Field::Date, Icon::Calendar, c, &t, true);
                                }
                                None => put(ui, Field::Date, Icon::Calendar, muted(), "Date", false),
                            }
                            let rem = pickers::reminders_text(&d.reminders);
                            put(ui, Field::Reminders, Icon::Alarm, text(), rem.as_deref().unwrap_or("Reminders"), rem.is_some());
                            if d.priority > 1 {
                                put(ui, Field::Priority, Icon::Flag, priority_color(d.priority), &format!("P{}", 5 - d.priority), true);
                            } else {
                                put(ui, Field::Priority, Icon::Flag, muted(), "Priority", false);
                            }
                            match d.deadline {
                                Some(dl) => {
                                    let (t, c) = widgets::deadline_label(dl, today);
                                    put(ui, Field::Deadline, Icon::Deadline, c, &t, true);
                                }
                                None => put(ui, Field::Deadline, Icon::Deadline, muted(), "Deadline", false),
                            }
                            if d.labels.is_empty() {
                                put(ui, Field::Labels, Icon::Tag, muted(), "Labels", false);
                            } else {
                                put(ui, Field::Labels, Icon::Tag, text(), &d.labels.join(", "), true);
                            }
                        });
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                        let ready = !ad.text.trim().is_empty();
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
                        let fill = if ready { if resp.hovered() { accent().gamma_multiply(1.15) } else { accent() } } else { accent().gamma_multiply(0.45) };
                        ui.painter().rect_filled(r, 6.0, fill);
                        theme::icon(ui.painter(), r, Icon::Send, Color32::WHITE);
                        if resp.on_hover_text("Add task (Enter)").clicked() {
                            submit = true;
                        }
                        if widgets::icon_button(ui, Icon::Close, "Close (Esc)").clicked() {
                            close = true;
                        }
                    });
                });
                match clear {
                    Some(Field::Date) => ad.draft.due = None,
                    Some(Field::Reminders) => ad.draft.reminders.clear(),
                    Some(Field::Priority) => ad.draft.priority = 1,
                    Some(Field::Deadline) => ad.draft.deadline = None,
                    Some(Field::Labels) => ad.draft.labels.clear(),
                    _ => {}
                }
            });
    });

    // The "+" menu.
    if ad.menu {
        let mr = egui::Area::new(egui::Id::unique("adder_menu")).order(egui::Order::Foreground).fixed_pos(plus_rect.left_bottom() + Vec2::new(0.0, 6.0)).show(ctx, |ui| {
            egui::Frame::new().fill(popup()).stroke(Stroke::new(1.0, line())).corner_radius(8.0).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(270.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                let rows = [
                    (Icon::Hash, "Project", "#", Field::Project),
                    (Icon::Calendar, "Date", "", Field::Date),
                    (Icon::Alarm, "Reminders", "", Field::Reminders),
                    (Icon::Flag, "Priority", "!", Field::Priority),
                    (Icon::Deadline, "Deadline", "{", Field::Deadline),
                    (Icon::Tag, "Labels", "@", Field::Labels),
                ];
                for (ic, name, key, field) in rows {
                    if menu_row(ui, ic, name, key) {
                        ad.menu = false;
                        open = Some((field, Rect::from_min_size(plus_rect.left_bottom(), Vec2::ZERO)));
                    }
                }
            });
        });
        let outside = ctx.input(|i| i.pointer.any_pressed())
            && ctx.pointer_interact_pos().is_some_and(|p| !mr.response.rect.contains(p) && !plus_rect.contains(p));
        if outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
            ad.menu = false;
        }
    }

    if let Some((f, r)) = open {
        open_pop(&mut ad.pop, f, r, &ad.draft);
    }
    let mut pop_open = false;
    if let Some(pop) = ad.pop.as_mut() {
        pop_open = true;
        if let PopResult::Close = pickers::show(ctx, &app.store, pop, &mut ad.draft, today) {
            ad.pop = None;
            ad.focus = true;
        }
    } else if !ad.menu && ctx.input(|i| i.key_pressed(Key::Escape)) {
        close = true;
    }
    // A click outside an empty quick add closes it, as in Todoist.
    if !pop_open && !ad.menu && ad.text.trim().is_empty() && ctx.input(|i| i.pointer.any_pressed()) && ctx.pointer_interact_pos().is_some_and(|p| !area.response.rect.contains(p)) {
        close = true;
    }

    if submit && !ad.text.trim().is_empty() {
        let d = &ad.draft;
        let extras = AddExtras {
            description: ad.description.trim().to_string(),
            project_id: Some(d.project_id.clone()).filter(|p| !p.is_empty()),
            section_id: d.section_id.clone(),
            parent_id: ad.parent_id.clone(),
            priority: (d.priority > 1).then_some(d.priority),
            due: d.due.as_ref().map(DueEdit::to_json),
            deadline: d.deadline.map(|x| x.format("%Y-%m-%d").to_string()),
            labels: d.labels.clone(),
            reminders: d.reminders.iter().map(|r| r.args("")).collect(),
        };
        app.add_task(ad.text.trim().to_string(), extras);
        // Stay open for the next task, like Todoist's quick add.
        ad.text.clear();
        ad.description.clear();
        ad.focus = true;
    }
    if !close {
        app.adder = Some(ad);
    }
}

/// Parse "15:30", "3pm", "3:30 PM", or "9".
pub fn parse_time(s: &str) -> Option<NaiveTime> {
    let t = s.trim().to_lowercase().replace(' ', "");
    let (body, pm) = if let Some(b) = t.strip_suffix("pm") {
        (b, Some(true))
    } else if let Some(b) = t.strip_suffix("am") {
        (b, Some(false))
    } else {
        (t.as_str(), None)
    };
    let (h, m) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (body.parse::<u32>().ok()?, 0),
    };
    let h = match pm {
        Some(_) if h == 0 || h > 12 => return None,
        Some(true) => h % 12 + 12,
        Some(false) => h % 12,
        None => h,
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

pub struct EventEditor {
    /// Key of the event being changed. `None` when making a new one.
    key: Option<String>,
    cal: String,
    title: String,
    date: String,
    start: String,
    end: String,
    all_day: bool,
    /// Days from the start date to the end date (for all-day events: the days covered).
    span_days: i64,
    error: String,
    focus: bool,
}

impl EventEditor {
    pub fn edit(e: &Event) -> Self {
        let span = if e.all_day { (e.end.date() - e.start.date()).num_days().max(1) } else { (e.end.date() - e.start.date()).num_days() };
        Self {
            key: Some(e.key()),
            cal: e.cal.clone(),
            title: e.title.clone(),
            date: e.start.date().to_string(),
            start: fmt_hm(e.start.time()),
            end: fmt_hm(e.end.time()),
            all_day: e.all_day,
            span_days: span,
            error: String::new(),
            focus: true,
        }
    }

    pub fn new_on(date: NaiveDate, store: &Store) -> Self {
        let cal = store.gcals.iter().filter(|c| c.writable).max_by_key(|c| c.primary).map(|c| c.id.clone()).unwrap_or_default();
        let now = chrono::Local::now().time();
        let start = NaiveTime::from_hms_opt((now.hour() + 1).min(22), 0, 0).unwrap_or(NaiveTime::MIN);
        Self {
            key: None,
            cal,
            title: String::new(),
            date: date.to_string(),
            start: fmt_hm(start),
            end: fmt_hm(start + Duration::hours(1)),
            all_day: false,
            span_days: 0,
            error: String::new(),
            focus: true,
        }
    }

    /// The start and end this form describes.
    pub fn times(&self) -> Result<(NaiveDateTime, NaiveDateTime), String> {
        let date = NaiveDate::parse_from_str(self.date.trim(), "%Y-%m-%d").map_err(|_| "Date must look like 2026-11-02".to_string())?;
        if self.all_day {
            return Ok((date.and_time(NaiveTime::MIN), (date + Duration::days(self.span_days.max(1))).and_time(NaiveTime::MIN)));
        }
        let s = parse_time(&self.start).ok_or("Start time is not valid")?;
        let e = parse_time(&self.end).ok_or("End time is not valid")?;
        let start = date.and_time(s);
        let end = (date + Duration::days(self.span_days)).and_time(e);
        if end <= start {
            return Err("The event must end after it starts".into());
        }
        Ok((start, end))
    }
}

fn fmt_hm(t: NaiveTime) -> String {
    t.format("%H:%M").to_string()
}

fn event_ui(app: &mut App, ctx: &egui::Context) {
    let Some(mut ed) = app.event_editor.take() else { return };
    let mut close = false;
    let mut save = false;
    let mut delete = false;
    let writable: Vec<(String, String)> = app.store.gcals.iter().filter(|c| c.writable).map(|c| (c.id.clone(), c.name.clone())).collect();
    let modal = egui::Modal::new(egui::Id::unique("event")).frame(egui::Frame::new().fill(popup()).corner_radius(12.0).inner_margin(20.0)).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(RichText::new(if ed.key.is_some() { "Edit event" } else { "New event" }).font(bold_font(18.0)));
        ui.add_space(10.0);
        let title = ui.add(egui::TextEdit::singleline(&mut ed.title).hint_text("Title").desired_width(f32::INFINITY));
        if std::mem::take(&mut ed.focus) {
            title.request_focus();
        }
        ui.add_space(8.0);
        if ed.key.is_none() && writable.len() > 1 {
            let name = writable.iter().find(|c| c.0 == ed.cal).map(|c| c.1.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("event_cal").selected_text(name).width(260.0).show_ui(ui, |ui| {
                for (id, n) in &writable {
                    ui.selectable_value(&mut ed.cal, id.clone(), n);
                }
            });
            ui.add_space(8.0);
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut ed.date).desired_width(100.0));
            ui.checkbox(&mut ed.all_day, "All day");
        });
        if !ed.all_day {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut ed.start).desired_width(70.0));
                ui.label("to");
                ui.add(egui::TextEdit::singleline(&mut ed.end).desired_width(70.0));
            });
        }
        if !ed.error.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new(&ed.error).color(RED));
        }
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ed.key.is_some() && ui.button("Delete").clicked() {
                delete = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("Save").color(Color32::WHITE)).fill(accent())).clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            save = true;
        }
    });
    if modal.should_close() {
        close = true;
    }
    if save {
        match ed.times() {
            _ if ed.title.trim().is_empty() => ed.error = "Add a title".into(),
            _ if ed.cal.is_empty() && ed.key.is_none() => ed.error = "No calendar can take new events".into(),
            Err(e) => ed.error = e,
            Ok((start, end)) => {
                let title = ed.title.trim().to_string();
                match &ed.key {
                    None => app.send(crate::api::Job::GoogleCreate { cal: ed.cal.clone(), title, start, end, all_day: ed.all_day }),
                    Some(key) => {
                        if let Some(e) = app.store.gevents.iter_mut().find(|e| &e.key() == key) {
                            (e.title, e.start, e.end, e.all_day) = (title.clone(), start, end, ed.all_day);
                            let job = crate::api::Job::GoogleUpdate { cal: e.cal.clone(), id: e.id.clone(), title: Some(title), start, end, all_day: ed.all_day };
                            app.send(job);
                            app.rebuild_events();
                        }
                    }
                }
                close = true;
            }
        }
    }
    if delete {
        if let Some(key) = &ed.key {
            if let Some(e) = app.store.gevents.iter().find(|e| &e.key() == key).cloned() {
                app.send(crate::api::Job::GoogleDelete { cal: e.cal, id: e.id });
                app.store.gevents.retain(|x| &x.key() != key);
                app.rebuild_events();
            }
        }
        close = true;
    }
    if !close {
        app.event_editor = Some(ed);
    }
}

pub fn show(app: &mut App, ctx: &egui::Context, out: &mut Vec<Action>) {
    editor_ui(app, ctx, out);
    adder_ui(app, ctx);
    event_ui(app, ctx);
    crate::settings::show(app, ctx);
}

#[cfg(test)]
mod event_tests {
    use super::*;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn times_are_read_in_common_forms() {
        assert_eq!(parse_time("15:30"), Some(t(15, 30)));
        assert_eq!(parse_time("3pm"), Some(t(15, 0)));
        assert_eq!(parse_time("3:30 PM"), Some(t(15, 30)));
        assert_eq!(parse_time("12am"), Some(t(0, 0)));
        assert_eq!(parse_time("12pm"), Some(t(12, 0)));
        assert_eq!(parse_time("9"), Some(t(9, 0)));
        assert_eq!(parse_time("25:00"), None);
        assert_eq!(parse_time("13pm"), None);
        assert_eq!(parse_time("soon"), None);
    }

    #[test]
    fn form_builds_times_and_checks_them() {
        let mut e = EventEditor::new_on(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap(), &Store::default());
        e.start = "10:00".into();
        e.end = "11:30".into();
        let (s, en) = e.times().unwrap();
        assert_eq!(en - s, Duration::minutes(90));
        e.end = "09:00".into();
        assert!(e.times().is_err());
        e.date = "tomorrow".into();
        assert!(e.times().is_err());
        e.date = "2026-11-02".into();
        e.all_day = true;
        let (s, en) = e.times().unwrap();
        assert_eq!(en - s, Duration::days(1));
    }

    #[test]
    fn editing_keeps_multi_day_span() {
        let d = |x: &str| NaiveDate::parse_from_str(x, "%Y-%m-%d").unwrap().and_time(NaiveTime::MIN);
        let ev = Event::new("Trip".into(), d("2026-11-03"), d("2026-11-06"), true);
        let (s, en) = EventEditor::edit(&ev).times().unwrap();
        assert_eq!((s, en), (d("2026-11-03"), d("2026-11-06")));
    }
}

