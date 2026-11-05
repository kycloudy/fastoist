//! List views: Inbox, Today, projects, filters, labels, and search.

use crate::app::{App, Group, View};
use crate::model::{Item, Store};
use crate::theme::{self, Icon, *};
use crate::widgets::{self, task_row, Action, AddDefaults, RowOpts};
use chrono::NaiveDate;
use eframe::egui::{self, Align2, Pos2, Rect, Sense, Ui, Vec2};

const MAX_W: f32 = 820.0;

/// Center a column of at most MAX_W like Todoist's list views.
fn column(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
        let w = ui.available_width();
        let cw = (w - 64.0).min(MAX_W);
        let pad = ((w - cw) / 2.0).max(16.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(cw);
                add(ui);
                ui.add_space(80.0);
            });
        });
    });
}

fn heading(ui: &mut Ui, title: &str) {
    ui.add_space(14.0);
    ui.label(egui::RichText::new(title).font(bold_font(26.0)));
}

fn subtitle_count(ui: &mut Ui, n: usize) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
        theme::check_circle(ui.painter(), r.center(), 6.0, 1, true, false);
        ui.label(egui::RichText::new(format!("{n} task{}", if n == 1 { "" } else { "s" })).color(MUTED).size(13.0));
    });
}

/// Rows for top-level tasks with their sub-tasks nested below.
fn tree_rows(ui: &mut Ui, store: &Store, items: &[&Item], today: NaiveDate, depth: usize, out: &mut Vec<Action>) {
    for it in items {
        task_row(ui, store, it, today, &RowOpts { show_project: false, depth, ..Default::default() }, out);
        if !it.collapsed {
            let kids = store.subtasks(&it.id);
            if !kids.is_empty() {
                tree_rows(ui, store, &kids, today, depth + 1, out);
            }
        }
    }
}

fn project_view(app: &mut App, id: &str, ui: &mut Ui, out: &mut Vec<Action>) {
    let today = App::today();
    let store = &app.store;
    let title = if store.inbox_id() == Some(id) { "Inbox".to_string() } else { store.project_name(id).to_string() };
    let id = id.to_string();
    column(ui, "project", |ui| {
        heading(ui, &title);
        ui.add_space(8.0);
        let ids: std::collections::HashSet<&str> = store.items.keys().map(String::as_str).collect();
        let top = |section: Option<&str>| {
            let mut v: Vec<&Item> = store
                .items
                .values()
                .filter(|i| i.project_id == id && i.section_id.as_deref() == section && i.parent_id.as_deref().is_none_or(|p| !ids.contains(p)))
                .collect();
            v.sort_by_key(|i| i.child_order);
            v
        };
        let loose = top(None);
        tree_rows(ui, store, &loose, today, 0, out);
        widgets::add_row(ui, AddDefaults { project_id: Some(id.clone()), ..Default::default() }, out);
        for s in store.sections_of(&id) {
            let items = top(Some(&s.id));
            widgets::group_header(ui, &s.name, Some(items.len()));
            tree_rows(ui, store, &items, today, 0, out);
            widgets::add_row(ui, AddDefaults { project_id: Some(id.clone()), section_id: Some(s.id.clone()), ..Default::default() }, out);
        }
    });
}

fn flat_rows(ui: &mut Ui, store: &Store, items: &[&Item], today: NaiveDate, show_due: bool, out: &mut Vec<Action>) {
    for it in items {
        task_row(ui, store, it, today, &RowOpts { show_due, ..Default::default() }, out);
    }
}

fn today_view(app: &mut App, ui: &mut Ui, out: &mut Vec<Action>) {
    let today = App::today();
    let group = app.cfg.today_group;
    let store = &app.store;
    let mut overdue: Vec<&Item> = store.items.values().filter(|i| i.due_date().is_some_and(|d| d < today)).collect();
    let mut due: Vec<&Item> = store.items.values().filter(|i| i.due_date() == Some(today)).collect();
    overdue.sort_by(|a, b| widgets::date_sort(a, b));
    due.sort_by(|a, b| widgets::date_sort(a, b));
    let events: Vec<_> = store.events.iter().filter(|e| e.start.date() <= today && (e.end.date() > today || e.start.date() == today)).collect();

    column(ui, "today", |ui| {
        heading(ui, "Today");
        subtitle_count(ui, overdue.len() + due.len());
        if !events.is_empty() {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Events").font(bold_font(13.0)));
            for e in &events {
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
                widgets::event_chip(ui, r, e, out);
            }
        }
        if !overdue.is_empty() {
            ui.add_space(14.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
            ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, "Overdue", bold_font(14.0), TEXT);
            let btn = Rect::from_min_max(Pos2::new(rect.right() - 90.0, rect.top()), rect.right_bottom());
            let r = ui.interact(btn, ui.scope_id().with("resched"), Sense::click());
            ui.painter().text(btn.right_center(), Align2::RIGHT_CENTER, "Reschedule", font(13.0), if r.hovered() { ACCENT_TEXT } else { ACCENT });
            if r.clicked() {
                for it in &overdue {
                    out.push(Action::Schedule { id: it.id.clone(), date: Some(today), time: None });
                }
            }
            ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], egui::Stroke::new(1.0, LINE));
            flat_rows(ui, store, &overdue, today, true, out);
        }
        let defaults = AddDefaults { due: Some(today), ..Default::default() };
        match group {
            Group::Project => {
                let mut projects: Vec<&str> = due.iter().map(|i| i.project_id.as_str()).collect();
                let order: Vec<&str> = std::iter::once(store.inbox_id().unwrap_or(""))
                    .chain(store.project_tree().into_iter().map(|(p, _)| p.id.as_str()))
                    .collect();
                projects.sort_by_key(|p| order.iter().position(|o| o == p).unwrap_or(usize::MAX));
                projects.dedup();
                if !overdue.is_empty() && !projects.is_empty() {
                    ui.add_space(4.0);
                }
                for pid in projects {
                    let name = if store.inbox_id() == Some(pid) { "Inbox" } else { store.project_name(pid) };
                    let items: Vec<&Item> = due.iter().copied().filter(|i| i.project_id == pid).collect();
                    widgets::group_header(ui, name, None);
                    flat_rows(ui, store, &items, today, true, out);
                }
                widgets::add_row(ui, defaults, out);
            }
            Group::Priority => {
                for (p, name) in [(4u8, "Priority 1"), (3, "Priority 2"), (2, "Priority 3"), (1, "Priority 4")] {
                    let items: Vec<&Item> = due.iter().copied().filter(|i| i.priority == p).collect();
                    if !items.is_empty() {
                        widgets::group_header(ui, name, Some(items.len()));
                        flat_rows(ui, store, &items, today, true, out);
                    }
                }
                widgets::add_row(ui, defaults, out);
            }
            _ => {
                widgets::group_header(ui, &format!("{} · Today · {}", today.format("%b %-d"), today.format("%A")), None);
                flat_rows(ui, store, &due, today, true, out);
                widgets::add_row(ui, defaults, out);
            }
        }
    });
}

fn filter_view(app: &mut App, id: &str, ui: &mut Ui, out: &mut Vec<Action>) {
    let today = App::today();
    let Some(f) = app.store.filters.get(id) else { return };
    let loading = app.pending_filters.contains(&f.query);
    let store = &app.store;
    let mut items: Vec<&Item> = store
        .filter_results
        .get(&f.query)
        .map(|ids| ids.iter().filter_map(|i| store.items.get(i)).collect())
        .unwrap_or_default();
    items.sort_by(|a, b| widgets::date_sort(a, b));
    let (name, query) = (f.name.clone(), f.query.clone());
    column(ui, "filter", |ui| {
        heading(ui, &name);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&query).color(MUTED).size(13.0).monospace());
            if loading {
                ui.spinner();
            }
        });
        ui.add_space(8.0);
        flat_rows(ui, store, &items, today, true, out);
        widgets::add_row(ui, AddDefaults::default(), out);
    });
}

fn label_view(app: &mut App, name: &str, ui: &mut Ui, out: &mut Vec<Action>) {
    let today = App::today();
    let store = &app.store;
    let mut items: Vec<&Item> = store.items.values().filter(|i| i.labels.iter().any(|l| l == name)).collect();
    items.sort_by(|a, b| widgets::date_sort(a, b));
    column(ui, "label", |ui| {
        heading(ui, name);
        ui.add_space(8.0);
        flat_rows(ui, store, &items, today, true, out);
        widgets::add_row(ui, AddDefaults { labels: vec![name.to_string()], ..Default::default() }, out);
    });
}

fn filters_labels(app: &mut App, ui: &mut Ui, out: &mut Vec<Action>) {
    let store = &app.store;
    column(ui, "fl", |ui| {
        heading(ui, "Filters & Labels");
        widgets::group_header(ui, "My Filters", None);
        for f in store.sorted_filters() {
            let n = store.filter_results.get(&f.query).map(|v| v.iter().filter(|id| store.items.contains_key(*id)).count()).unwrap_or(0);
            if list_row(ui, Icon::Drop, named_color(&f.color), &f.name, n, &f.query).clicked() {
                out.push(Action::Go(View::Filter(f.id.clone())));
            }
        }
        widgets::group_header(ui, "Labels", None);
        for l in store.sorted_labels() {
            let n = store.items.values().filter(|i| i.labels.contains(&l.name)).count();
            if list_row(ui, Icon::Tag, named_color(&l.color), &l.name, n, "").clicked() {
                out.push(Action::Go(View::Label(l.name.clone())));
            }
        }
    });
}

fn list_row(ui: &mut Ui, ic: Icon, col: egui::Color32, name: &str, n: usize, detail: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 6.0, HOVER);
    }
    theme::icon(p, Rect::from_center_size(Pos2::new(rect.left() + 14.0, rect.center().y), Vec2::splat(16.0)), ic, col);
    p.text(Pos2::new(rect.left() + 34.0, rect.center().y), Align2::LEFT_CENTER, name, font(14.0), TEXT);
    if !detail.is_empty() {
        p.text(Pos2::new(rect.left() + 220.0, rect.center().y), Align2::LEFT_CENTER, detail, font(12.0), FAINT);
    }
    if n > 0 {
        p.text(Pos2::new(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, n.to_string(), font(12.0), FAINT);
    }
    p.line_segment([rect.left_bottom(), rect.right_bottom()], egui::Stroke::new(1.0, LINE));
    resp
}

fn search_view(app: &mut App, ui: &mut Ui, out: &mut Vec<Action>) {
    let today = App::today();
    let focus = std::mem::take(&mut app.focus_search);
    let store = &app.store;
    let search = &mut app.search;
    column(ui, "search", |ui| {
        ui.add_space(14.0);
        let r = ui.add(egui::TextEdit::singleline(search).hint_text("Search tasks, projects, labels").font(font(18.0)).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
        if focus {
            r.request_focus();
        }
        let q = search.trim().to_lowercase();
        if q.is_empty() {
            return;
        }
        let mut nav: Vec<(Icon, egui::Color32, String, View)> = Vec::new();
        for (p, _) in store.project_tree() {
            if p.name.to_lowercase().contains(&q) {
                nav.push((Icon::Hash, named_color(&p.color), p.name.clone(), View::Project(p.id.clone())));
            }
        }
        for f in store.sorted_filters() {
            if f.name.to_lowercase().contains(&q) {
                nav.push((Icon::Drop, named_color(&f.color), f.name.clone(), View::Filter(f.id.clone())));
            }
        }
        for l in store.sorted_labels() {
            if l.name.to_lowercase().contains(&q) {
                nav.push((Icon::Tag, named_color(&l.color), l.name.clone(), View::Label(l.name.clone())));
            }
        }
        if !nav.is_empty() {
            widgets::group_header(ui, "Navigation", None);
            for (ic, col, name, v) in nav {
                if list_row(ui, ic, col, &name, 0, "").clicked() {
                    out.push(Action::Go(v));
                }
            }
        }
        let mut items: Vec<&Item> = store.items.values().filter(|i| i.content.to_lowercase().contains(&q) || i.description.to_lowercase().contains(&q)).collect();
        items.sort_by(|a, b| widgets::date_sort(a, b));
        widgets::group_header(ui, "Tasks", Some(items.len()));
        let first = items.first().map(|i| i.id.clone());
        flat_rows(ui, store, &items[..items.len().min(200)], today, true, out);
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(id) = first {
                out.push(Action::Open(id));
            }
        }
    });
}

/// Tasks completed in the last 30 days, newest due date first. The circle brings one back.
fn completed_view(app: &mut App, ui: &mut Ui, out: &mut Vec<Action>) {
    let store = &app.store;
    let mut done: Vec<&Item> = store.completed.values().collect();
    done.sort_by(|a, b| b.due_date().cmp(&a.due_date()).then_with(|| a.content.cmp(&b.content)));
    column(ui, "completed", |ui| {
        heading(ui, "Completed");
        ui.add_space(8.0);
        for it in done {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
            let c = Pos2::new(rect.left() + 14.0, rect.center().y);
            theme::check_circle(ui.painter(), c, 8.0, it.priority, resp.hovered(), true);
            ui.painter().text(Pos2::new(rect.left() + 34.0, rect.center().y), Align2::LEFT_CENTER, &it.content, font(14.0), MUTED);
            if resp.on_hover_text("Mark as not done").clicked() {
                out.push(Action::Uncomplete(it.id.clone()));
            }
            ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], egui::Stroke::new(1.0, LINE));
        }
    });
}

pub fn show(app: &mut App, v: &View, ui: &mut Ui, out: &mut Vec<Action>) {
    match v {
        View::Inbox => {
            let id = app.store.inbox_id().unwrap_or("").to_string();
            project_view(app, &id, ui, out)
        }
        View::Today => today_view(app, ui, out),
        View::Project(id) => project_view(app, id, ui, out),
        View::Filter(id) => filter_view(app, id, ui, out),
        View::Label(n) => label_view(app, n, ui, out),
        View::FiltersLabels => filters_labels(app, ui, out),
        View::Search => search_view(app, ui, out),
        View::Completed => completed_view(app, ui, out),
        View::Upcoming => {}
    }
}
