//! Task editor, quick add, and settings dialogs.

use crate::api::AddExtras;
use crate::app::{App, Config};
use crate::model::{Item, Store};
use crate::theme::{self, Icon, *};
use crate::widgets::{self, Action, AddDefaults, RowOpts};
use eframe::egui::{self, Color32, Key, RichText, Ui, Vec2};
use serde_json::{json, Map, Value};

pub struct Editor {
    orig: Item,
    content: String,
    description: String,
    due: String,
    deadline: String,
    priority: u8,
    labels: Vec<String>,
    project_id: String,
    section_id: Option<String>,
    new_sub: String,
}

impl Editor {
    pub fn new(it: &Item, _store: &Store) -> Self {
        let due = match &it.due {
            Some(d) if !d.string.is_empty() => d.string.clone(),
            Some(d) => d.date.replace('T', " "),
            None => String::new(),
        };
        Self {
            orig: it.clone(),
            content: it.content.clone(),
            description: it.description.clone(),
            due,
            deadline: it.deadline.as_ref().map(|d| d.date.clone()).unwrap_or_default(),
            priority: it.priority.max(1),
            labels: it.labels.clone(),
            project_id: it.project_id.clone(),
            section_id: it.section_id.clone(),
            new_sub: String::new(),
        }
    }

    fn orig_due(&self) -> String {
        Editor::new(&self.orig, &Store::default()).due
    }
}

pub struct Adder {
    text: String,
    description: String,
    due: String,
    priority: u8,
    project_id: Option<String>,
    section_id: Option<String>,
    parent_id: Option<String>,
    labels: Vec<String>,
    focus: bool,
}

impl Adder {
    pub fn new(d: AddDefaults) -> Self {
        Self {
            text: String::new(),
            description: String::new(),
            due: d.due,
            priority: 1,
            project_id: d.project_id,
            section_id: d.section_id,
            parent_id: d.parent_id,
            labels: d.labels,
            focus: true,
        }
    }
}

pub struct Settings {
    token: String,
    calendars: String,
}

impl Settings {
    pub fn new(cfg: &Config) -> Self {
        Self { token: cfg.token.clone(), calendars: cfg.calendars.join("\n") }
    }
}

fn priority_picker(ui: &mut Ui, p: &mut u8) {
    let label = format!("P{}", 5 - *p);
    egui::ComboBox::from_id_salt(ui.scope_id().with("prio"))
        .selected_text(RichText::new(label).color(priority_color(*p)))
        .width(70.0)
        .show_ui(ui, |ui| {
            for v in [4u8, 3, 2, 1] {
                ui.selectable_value(p, v, RichText::new(format!("Priority {}", 5 - v)).color(priority_color(v)));
            }
        });
}

fn project_picker(ui: &mut Ui, store: &Store, project: &mut Option<String>, section: &mut Option<String>) {
    let inbox = store.inbox_id().unwrap_or("").to_string();
    let current = project.clone().unwrap_or_else(|| inbox.clone());
    let mut text = if current == inbox { "Inbox".to_string() } else { store.project_name(&current).to_string() };
    if let Some(s) = section.as_ref().and_then(|s| store.sections.get(s)) {
        text = format!("{text} / {}", s.name);
    }
    egui::ComboBox::from_id_salt(ui.scope_id().with("proj")).selected_text(format!("# {text}")).width(200.0).show_ui(ui, |ui| {
        if ui.selectable_label(current == inbox && section.is_none(), "Inbox").clicked() {
            *project = Some(inbox.clone());
            *section = None;
        }
        for (p, d) in store.project_tree() {
            let pad = "    ".repeat(d);
            if ui.selectable_label(current == p.id && section.is_none(), RichText::new(format!("{pad}# {}", p.name)).color(TEXT)).clicked() {
                *project = Some(p.id.clone());
                *section = None;
            }
            for s in store.sections_of(&p.id) {
                if ui.selectable_label(section.as_deref() == Some(&s.id), format!("{pad}      {}", s.name)).clicked() {
                    *project = Some(p.id.clone());
                    *section = Some(s.id.clone());
                }
            }
        }
    });
}

fn label_picker(ui: &mut Ui, store: &Store, labels: &mut Vec<String>) {
    let text = if labels.is_empty() { "Labels".to_string() } else { labels.join(", ") };
    ui.menu_button(text, |ui| {
        ui.set_min_width(180.0);
        for l in store.sorted_labels() {
            let mut on = labels.contains(&l.name);
            if ui.checkbox(&mut on, RichText::new(&l.name).color(named_color(&l.color))).changed() {
                if on {
                    labels.push(l.name.clone());
                } else {
                    labels.retain(|x| x != &l.name);
                }
            }
        }
    });
}

fn date_buttons(ui: &mut Ui, due: &mut String) {
    for (label, v) in [("Today", "today"), ("Tomorrow", "tomorrow"), ("Next week", "next monday"), ("No date", "")] {
        if ui.small_button(label).clicked() {
            *due = v.to_string();
        }
    }
}

fn editor_ui(app: &mut App, ctx: &egui::Context, out: &mut Vec<Action>) {
    let today = App::today();
    let Some(mut ed) = app.editor.take() else { return };
    // The task may have been completed or deleted elsewhere.
    if !app.store.items.contains_key(&ed.orig.id) {
        return;
    }
    let mut close = false;
    let mut save = false;
    let modal = egui::Modal::new(egui::Id::unique("editor")).frame(egui::Frame::new().fill(POPUP).corner_radius(12.0).inner_margin(20.0)).show(ctx, |ui| {
        ui.set_width(ctx.content_rect().width().min(820.0) - 80.0);
        let store = &app.store;
        ui.horizontal(|ui| {
            let pname = if store.inbox_id() == Some(&ed.orig.project_id) { "Inbox" } else { store.project_name(&ed.orig.project_id) };
            ui.label(RichText::new(format!("# {pname}")).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, Icon::Close, "Close (Esc)").clicked() {
                    close = true;
                }
            });
        });
        ui.separator();
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            ui.horizontal_top(|ui| {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::click());
                theme::check_circle(ui.painter(), r.center(), 10.0, ed.priority, resp.hovered(), false);
                if resp.clicked() {
                    out.push(Action::Complete(ed.orig.id.clone()));
                    close = true;
                }
                ui.add(egui::TextEdit::multiline(&mut ed.content).font(bold_font(18.0)).desired_rows(1).desired_width(f32::INFINITY).frame(egui::Frame::NONE));
            });
            ui.add(egui::TextEdit::multiline(&mut ed.description).hint_text("Description").desired_rows(3).desired_width(f32::INFINITY));
            ui.add_space(10.0);
            let subs: Vec<Item> = store.subtasks(&ed.orig.id).into_iter().cloned().collect();
            ui.label(RichText::new(format!("Sub-tasks  {}", subs.len())).font(bold_font(13.0)));
            for s in &subs {
                widgets::task_row(ui, store, s, today, &RowOpts { show_project: false, compact: true, ..Default::default() }, out);
            }
            let r = ui.add(egui::TextEdit::singleline(&mut ed.new_sub).hint_text("+ Add sub-task").desired_width(f32::INFINITY));
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !ed.new_sub.trim().is_empty() {
                let content = std::mem::take(&mut ed.new_sub);
                out.push(Action::AddSubtask { parent: ed.orig.id.clone(), content });
                r.request_focus();
            }

            let ui = &mut cols[1];
            let field = |ui: &mut Ui, name: &str| {
                ui.add_space(6.0);
                ui.label(RichText::new(name).size(12.0).color(MUTED));
            };
            field(ui, "Project");
            let mut proj = Some(ed.project_id.clone());
            project_picker(ui, store, &mut proj, &mut ed.section_id);
            if let Some(p) = proj {
                ed.project_id = p;
            }
            field(ui, "Date");
            ui.add(egui::TextEdit::singleline(&mut ed.due).hint_text("e.g. tomorrow 3pm, every fri").desired_width(f32::INFINITY));
            ui.horizontal_wrapped(|ui| date_buttons(ui, &mut ed.due));
            field(ui, "Deadline");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut ed.deadline).hint_text("YYYY-MM-DD").desired_width(120.0));
                if !ed.deadline.is_empty() && ui.small_button("Clear").clicked() {
                    ed.deadline.clear();
                }
            });
            field(ui, "Priority");
            priority_picker(ui, &mut ed.priority);
            field(ui, "Labels");
            label_picker(ui, store, &mut ed.labels);
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("Delete").color(RED)).clicked() {
                out.push(Action::Delete(ed.orig.id.clone()));
                close = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("Save").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.ctrl) {
            save = true;
        }
    });
    if modal.should_close() {
        close = true;
    }
    if save {
        save_editor(app, &ed);
        close = true;
    }
    if !close {
        app.editor = Some(ed);
    }
}

fn save_editor(app: &mut App, ed: &Editor) {
    let o = &ed.orig;
    let mut args = Map::new();
    args.insert("id".into(), json!(o.id));
    let it = app.store.items.get_mut(&o.id);
    let Some(it) = it else { return };
    if ed.content.trim() != o.content {
        args.insert("content".into(), json!(ed.content.trim()));
        it.content = ed.content.trim().to_string();
    }
    if ed.description != o.description {
        args.insert("description".into(), json!(ed.description));
        it.description = ed.description.clone();
    }
    if ed.priority != o.priority {
        args.insert("priority".into(), json!(ed.priority));
        it.priority = ed.priority;
    }
    if ed.labels != o.labels {
        args.insert("labels".into(), json!(ed.labels));
        it.labels = ed.labels.clone();
    }
    if ed.due.trim() != ed.orig_due() {
        if ed.due.trim().is_empty() {
            args.insert("due".into(), Value::Null);
            it.due = None;
        } else {
            args.insert("due".into(), json!({ "string": ed.due.trim() }));
        }
    }
    let old_deadline = o.deadline.as_ref().map(|d| d.date.clone()).unwrap_or_default();
    if ed.deadline.trim() != old_deadline {
        if ed.deadline.trim().is_empty() {
            args.insert("deadline".into(), Value::Null);
            it.deadline = None;
        } else {
            args.insert("deadline".into(), json!({ "date": ed.deadline.trim() }));
            it.deadline = Some(crate::model::Deadline { date: ed.deadline.trim().to_string() });
        }
    }
    let mv = if ed.section_id != o.section_id && ed.section_id.is_some() {
        it.project_id = ed.project_id.clone();
        it.section_id = ed.section_id.clone();
        Some(json!({ "id": o.id, "section_id": ed.section_id }))
    } else if ed.project_id != o.project_id || (ed.section_id.is_none() && o.section_id.is_some()) {
        it.project_id = ed.project_id.clone();
        it.section_id = None;
        Some(json!({ "id": o.id, "project_id": ed.project_id }))
    } else {
        None
    };
    app.update_task(&o.id, args, mv);
}

fn adder_ui(app: &mut App, ctx: &egui::Context) {
    let Some(mut ad) = app.adder.take() else { return };
    let mut close = false;
    let mut submit = false;
    let modal = egui::Modal::new(egui::Id::unique("adder")).frame(egui::Frame::new().fill(POPUP).corner_radius(12.0).inner_margin(16.0)).show(ctx, |ui| {
        ui.set_width(ctx.content_rect().width().min(640.0) - 60.0);
        let store = &app.store;
        let r = ui.add(egui::TextEdit::singleline(&mut ad.text).hint_text("Task name").font(bold_font(16.0)).frame(egui::Frame::NONE).desired_width(f32::INFINITY));
        if std::mem::take(&mut ad.focus) {
            r.request_focus();
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            submit = true;
        }
        ui.add(egui::TextEdit::multiline(&mut ad.description).hint_text("Description").frame(egui::Frame::NONE).desired_rows(1).desired_width(f32::INFINITY));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::TextEdit::singleline(&mut ad.due).hint_text("Date").desired_width(150.0));
            priority_picker(ui, &mut ad.priority);
            label_picker(ui, store, &mut ad.labels);
        });
        ui.separator();
        ui.horizontal(|ui| {
            project_picker(ui, store, &mut ad.project_id, &mut ad.section_id);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ok = !ad.text.trim().is_empty();
                if ui.add_enabled(ok, egui::Button::new(RichText::new("Add task").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                    submit = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
    });
    if modal.should_close() {
        close = true;
    }
    if submit && !ad.text.trim().is_empty() {
        let extras = AddExtras {
            description: ad.description.trim().to_string(),
            project_id: ad.project_id.clone(),
            section_id: ad.section_id.clone(),
            parent_id: ad.parent_id.clone(),
            priority: (ad.priority > 1).then_some(ad.priority),
            due_string: ad.due.clone(),
            deadline: String::new(),
            labels: ad.labels.clone(),
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

fn settings_ui(app: &mut App, ctx: &egui::Context) {
    let Some(mut s) = app.settings.take() else { return };
    let mut close = false;
    let mut save = false;
    let modal = egui::Modal::new(egui::Id::unique("settings")).frame(egui::Frame::new().fill(POPUP).corner_radius(12.0).inner_margin(20.0)).show(ctx, |ui| {
        ui.set_width(520.0);
        ui.label(RichText::new("Settings").font(bold_font(18.0)));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label("Todoist API token");
            ui.hyperlink_to(RichText::new("Get token").size(12.0), "https://app.todoist.com/app/settings/integrations/developer");
        });
        ui.add(egui::TextEdit::singleline(&mut s.token).password(true).desired_width(f32::INFINITY));
        ui.add_space(10.0);
        ui.label("Calendar feeds (iCal URLs, one per line)");
        ui.add(egui::TextEdit::multiline(&mut s.calendars).hint_text("https://calendar.google.com/calendar/ical/…/basic.ics").desired_rows(3).desired_width(f32::INFINITY));
        ui.add_space(10.0);
        if ui.checkbox(&mut app.cfg.stay_in_tray, "Stay running in tray").changed() {
            app.save_config();
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Full resync").clicked() {
                app.send(crate::api::Job::FullSync);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("Save").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
    });
    if modal.should_close() {
        close = true;
    }
    if save {
        let urls: Vec<String> = s.calendars.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        if s.token.trim() != app.cfg.token {
            app.set_token(s.token.clone());
        }
        if urls != app.cfg.calendars {
            app.set_calendars(urls);
        }
        close = true;
    }
    if !close {
        app.settings = Some(s);
    }
}

pub fn show(app: &mut App, ctx: &egui::Context, out: &mut Vec<Action>) {
    editor_ui(app, ctx, out);
    adder_ui(app, ctx);
    settings_ui(app, ctx);
}
