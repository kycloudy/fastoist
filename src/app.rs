//! App state, sidebar, navigation, and how user actions reach Todoist.

use crate::api::{self, add_command, command, AddExtras, Job, Msg};
use crate::dialogs::{Adder, Editor, Settings};
use crate::model::{Due, Item, Store};
use crate::theme::{self, Icon, *};
use crate::tray::{self, Cmd, Tray};
use crate::widgets::{self, Action, AddDefaults};
use chrono::{Datelike, Local, NaiveDate, NaiveTime};
use eframe::egui::{self, Align2, Color32, Key, Pos2, Rect, Sense, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum View {
    Inbox,
    Today,
    Upcoming,
    FiltersLabels,
    Search,
    Completed,
    Project(String),
    Filter(String),
    Label(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Layout {
    List,
    Week,
    #[default]
    Month,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Group {
    None,
    #[default]
    Project,
    Priority,
    Date,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub token: String,
    pub calendars: Vec<String>,
    pub layout: Layout,
    pub plan_open: bool,
    pub today_group: Group,
    pub sidebar_open: bool,
    pub collapsed: HashSet<String>,
    pub last_view: Option<View>,
    pub stay_in_tray: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            token: String::new(),
            calendars: Vec::new(),
            layout: Layout::Month,
            plan_open: true,
            today_group: Group::Project,
            sidebar_open: true,
            collapsed: HashSet::new(),
            last_view: None,
            stay_in_tray: true,
        }
    }
}

fn data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Fastoist")
}

fn load<T: for<'de> Deserialize<'de> + Default>(name: &str) -> T {
    std::fs::read(data_dir().join(name)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_bytes(name: &str, bytes: Vec<u8>) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let tmp = dir.join(format!("{name}.tmp"));
    if std::fs::write(&tmp, bytes).is_ok() {
        let _ = std::fs::rename(tmp, dir.join(name));
    }
}

pub struct Toast {
    pub text: String,
    pub error: bool,
    pub undo: Option<Item>,
    pub until: Instant,
}

pub struct App {
    pub store: Store,
    pub cfg: Config,
    jobs: Sender<Job>,
    msgs: Receiver<Msg>,
    pub view: View,
    back: Vec<View>,
    fwd: Vec<View>,
    busy: bool,
    pub toast: Option<Toast>,
    pub anchor: NaiveDate,
    pub plan_project: Option<String>,
    pub plan_label: Option<String>,
    pub scroll_week: bool,
    pub search: String,
    pub focus_search: bool,
    pub editor: Option<Editor>,
    pub adder: Option<Adder>,
    pub settings: Option<Settings>,
    pub pending_filters: HashSet<String>,
    dirty: bool,
    last_save: Instant,
    completed_range: Option<(NaiveDate, NaiveDate)>,
    title: String,
    tray: Option<Tray>,
    tray_at: Option<Instant>,
    hidden: bool,
    quitting: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        // Dark title bar on Windows 10 and 11.
        cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
        let mut cfg: Config = load("config.json");
        if let Ok(t) = std::env::var("TODOIST_TOKEN") {
            if cfg.token.is_empty() {
                cfg.token = t;
            }
        }
        let store: Store = load("cache.json");
        let (jtx, jrx) = channel();
        let (mtx, mrx) = channel();
        let ctx = cc.egui_ctx.clone();
        api::spawn(cfg.token.clone(), store.sync_token.clone(), jrx, mtx, move || ctx.request_repaint());
        let view = cfg.last_view.clone().unwrap_or(View::Today);
        let app = Self {
            store,
            jobs: jtx,
            msgs: mrx,
            view,
            back: Vec::new(),
            fwd: Vec::new(),
            busy: false,
            toast: None,
            anchor: Local::now().date_naive(),
            plan_project: None,
            plan_label: None,
            scroll_week: true,
            search: String::new(),
            focus_search: false,
            editor: None,
            adder: None,
            settings: None,
            pending_filters: HashSet::new(),
            dirty: false,
            last_save: Instant::now(),
            completed_range: None,
            title: String::new(),
            tray: native_window(cc).and_then(|h| Tray::new(&cc.egui_ctx, h)),
            tray_at: None,
            hidden: false,
            quitting: false,
            cfg,
        };
        if !app.cfg.token.is_empty() {
            app.send(Job::Sync);
            app.send(Job::Calendars(app.cfg.calendars.clone()));
        }
        app
    }

    pub fn send(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    pub fn today() -> NaiveDate {
        Local::now().date_naive()
    }

    pub fn set_token(&mut self, token: String) {
        self.cfg.token = token.trim().to_string();
        self.store = Store::default();
        self.send(Job::SetToken(self.cfg.token.clone()));
        self.save_config();
    }

    pub fn set_calendars(&mut self, urls: Vec<String>) {
        self.cfg.calendars = urls;
        self.save_config();
        if self.cfg.calendars.is_empty() {
            self.store.events.clear();
        } else {
            self.send(Job::Calendars(self.cfg.calendars.clone()));
        }
    }

    pub fn save_config(&self) {
        if let Ok(b) = serde_json::to_vec_pretty(&self.cfg) {
            save_bytes("config.json", b);
        }
    }

    pub fn go(&mut self, v: View) {
        if v != self.view {
            self.back.push(std::mem::replace(&mut self.view, v));
            self.fwd.clear();
            self.on_enter();
        }
    }

    fn on_enter(&mut self) {
        match &self.view {
            View::Filter(id) => {
                if let Some(f) = self.store.filters.get(id) {
                    let q = f.query.clone();
                    self.refresh_filter(q);
                }
            }
            View::Upcoming => {
                self.anchor = Self::today();
                self.scroll_week = true;
            }
            View::Search => self.focus_search = true,
            View::Completed => {
                let today = Self::today();
                self.want_completed(today - chrono::Duration::days(30), today);
            }
            _ => {}
        }
        self.cfg.last_view = Some(self.view.clone());
    }

    fn refresh_filter(&mut self, query: String) {
        if self.pending_filters.insert(query.clone()) {
            self.send(Job::Filter(query));
        }
    }

    fn refresh_filters(&mut self) {
        let mut qs: Vec<String> = self.store.filters.values().filter(|f| f.is_favorite).map(|f| f.query.clone()).collect();
        if let View::Filter(id) = &self.view {
            if let Some(f) = self.store.filters.get(id) {
                qs.push(f.query.clone());
            }
        }
        for q in qs {
            self.refresh_filter(q);
        }
    }

    /// Ask for completed tasks in the visible calendar range, once per range.
    pub fn want_completed(&mut self, since: NaiveDate, until: NaiveDate) {
        if self.completed_range != Some((since, until)) {
            self.completed_range = Some((since, until));
            self.send(Job::Completed { since, until });
        }
    }

    fn handle_msgs(&mut self) {
        while let Ok(m) = self.msgs.try_recv() {
            match m {
                Msg::Synced(r) => {
                    let changed = r.full_sync || r.items.as_ref().is_some_and(|v| !v.is_empty());
                    self.store.apply(r);
                    self.dirty = true;
                    if changed {
                        self.refresh_filters();
                    }
                }
                Msg::FilterResult { query, items } => {
                    self.pending_filters.remove(&query);
                    let ids = items.iter().map(|i| i.id.clone()).collect();
                    for it in items {
                        self.store.items.entry(it.id.clone()).or_insert(it);
                    }
                    self.store.filter_results.insert(query, ids);
                    self.dirty = true;
                }
                Msg::Completed(items) => {
                    self.store.completed = items.into_iter().map(|i| (i.id.clone(), i)).collect();
                }
                Msg::Events(ev) => {
                    self.store.events = ev;
                    self.dirty = true;
                }
                Msg::QuickAddDone { temp_id } => {
                    self.store.items.remove(&temp_id);
                }
                Msg::Error(e) => {
                    self.pending_filters.clear();
                    self.toast = Some(Toast { text: e, error: true, undo: None, until: Instant::now() + Duration::from_secs(6) });
                }
                Msg::Busy(b) => self.busy = b,
            }
        }
    }

    fn save_cache(&mut self) {
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(3) {
            if let Ok(b) = serde_json::to_vec(&self.store) {
                std::thread::spawn(move || save_bytes("cache.json", b));
            }
            self.dirty = false;
            self.last_save = Instant::now();
        }
    }

    fn due_json(item: &Item, date: NaiveDate, time: Option<NaiveTime>) -> (Value, Due) {
        let date_str = match time {
            Some(t) => format!("{}T{}", date.format("%Y-%m-%d"), t.format("%H:%M:%S")),
            None => date.format("%Y-%m-%d").to_string(),
        };
        let mut due = item.due.clone().unwrap_or_default();
        due.date = date_str.clone();
        due.timezone = None;
        let v = if item.is_recurring() {
            json!({ "date": date_str, "string": due.string, "lang": due.lang.clone().unwrap_or_else(|| "en".into()), "is_recurring": true })
        } else {
            due.string = String::new();
            json!({ "date": date_str })
        };
        (v, due)
    }

    pub fn apply(&mut self, actions: Vec<Action>) {
        for a in actions {
            self.apply_one(a);
        }
    }

    fn apply_one(&mut self, a: Action) {
        let today = Self::today();
        match a {
            Action::Open(id) => {
                if let Some(it) = self.store.items.get(&id) {
                    self.editor = Some(Editor::new(it, &self.store));
                }
            }
            Action::Complete(id) => {
                let Some(it) = self.store.items.get(&id).cloned() else { return };
                self.send(Job::Commands(vec![command("item_close", json!({ "id": id }))]));
                if !it.is_recurring() {
                    let kids: Vec<String> = self.store.items.values().filter(|i| i.parent_id.as_deref() == Some(&id)).map(|i| i.id.clone()).collect();
                    for k in kids {
                        self.store.items.remove(&k);
                    }
                    self.store.items.remove(&id);
                    let mut done = it.clone();
                    done.checked = true;
                    self.store.completed.insert(id.clone(), done);
                }
                self.toast = Some(Toast { text: "1 task completed".into(), error: false, undo: Some(it), until: Instant::now() + Duration::from_secs(5) });
                self.dirty = true;
            }
            Action::Uncomplete(id) => {
                if let Some(mut it) = self.store.completed.remove(&id) {
                    self.send(Job::Commands(vec![command("item_uncomplete", json!({ "id": id }))]));
                    it.checked = false;
                    self.store.items.insert(id, it);
                }
            }
            Action::Delete(id) => {
                self.send(Job::Commands(vec![command("item_delete", json!({ "id": id }))]));
                self.store.items.retain(|_, i| i.id != id && i.parent_id.as_deref() != Some(&id));
                self.editor = None;
                self.dirty = true;
            }
            Action::Schedule { id, date, time } => {
                let Some(it) = self.store.items.get(&id).cloned() else { return };
                let (args, due) = match date {
                    None => (json!({ "id": id, "due": null }), None),
                    Some(d) => {
                        let t = match time {
                            Some(t) => t,
                            None => it.due_at().and_then(|a| a.time),
                        };
                        let (v, due) = Self::due_json(&it, d, t);
                        let mut args = json!({ "id": id, "due": v });
                        // Dropping a task on a time slot without a duration gives it Todoist's default length.
                        if matches!(time, Some(Some(_))) && it.duration.is_none() {
                            args["duration"] = json!({ "amount": 30, "unit": "minute" });
                        }
                        (args, Some(due))
                    }
                };
                self.send(Job::Commands(vec![command("item_update", args)]));
                if let Some(i) = self.store.items.get_mut(&id) {
                    i.due = due;
                }
                let _ = today;
                self.dirty = true;
            }
            Action::Priority(id, p) => {
                self.send(Job::Commands(vec![command("item_update", json!({ "id": id, "priority": p }))]));
                if let Some(i) = self.store.items.get_mut(&id) {
                    i.priority = p;
                }
            }
            Action::AddTask(d) => self.adder = Some(Adder::new(d)),
            Action::AddSubtask { parent, content } => {
                if let Some(p) = self.store.items.get(&parent).cloned() {
                    self.add_subtask(&p, content);
                }
            }
            Action::Go(v) => self.go(v),
            Action::ShowWeek(d) => {
                self.cfg.layout = Layout::Week;
                self.anchor = d;
                self.scroll_week = true;
            }
        }
    }

    fn undo(&mut self) {
        let Some(t) = self.toast.take() else { return };
        let Some(it) = t.undo else { return };
        if it.is_recurring() {
            let due = it.due.clone().unwrap_or_default();
            self.send(Job::Commands(vec![command("item_update", json!({ "id": it.id, "due": { "date": due.date, "string": due.string, "is_recurring": true } }))]));
        } else {
            self.send(Job::Commands(vec![command("item_uncomplete", json!({ "id": it.id }))]));
            self.store.completed.remove(&it.id);
        }
        self.store.items.insert(it.id.clone(), it);
    }

    /// Add a task through quick add, which understands dates, #project, @label, and p1-p4 in the text.
    pub fn add_task(&mut self, text: String, extras: AddExtras) {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let project = extras.project_id.clone().or_else(|| self.store.inbox_id().map(String::from)).unwrap_or_default();
        self.store.items.insert(
            temp_id.clone(),
            Item {
                id: temp_id.clone(),
                content: text.clone(),
                project_id: project,
                section_id: extras.section_id.clone(),
                parent_id: extras.parent_id.clone(),
                priority: extras.priority.unwrap_or(1),
                child_order: i64::MAX,
                ..Default::default()
            },
        );
        self.send(Job::QuickAdd { text, temp_id, extras });
    }

    /// Add a plain sub-task without quick-add parsing.
    pub fn add_subtask(&mut self, parent: &Item, content: String) {
        let temp_id = uuid::Uuid::new_v4().to_string();
        self.send(Job::Commands(vec![add_command(&temp_id, json!({ "content": content, "parent_id": parent.id, "project_id": parent.project_id }))]));
        self.store.items.insert(
            temp_id.clone(),
            Item { id: temp_id, content, project_id: parent.project_id.clone(), section_id: parent.section_id.clone(), parent_id: Some(parent.id.clone()), priority: 1, child_order: i64::MAX, ..Default::default() },
        );
    }

    pub fn update_task(&mut self, id: &str, args: serde_json::Map<String, Value>, mv: Option<Value>) {
        let mut cmds = Vec::new();
        if args.len() > 1 {
            cmds.push(command("item_update", Value::Object(args)));
        }
        if let Some(m) = mv {
            cmds.push(command("item_move", m));
        }
        if !cmds.is_empty() {
            self.send(Job::Commands(cmds));
        }
        let _ = id;
        self.dirty = true;
    }

    fn view_title(&self) -> String {
        match &self.view {
            View::Inbox => "Inbox".into(),
            View::Today => "Today".into(),
            View::Upcoming => "Upcoming".into(),
            View::FiltersLabels => "Filters & Labels".into(),
            View::Search => "Search".into(),
            View::Completed => "Completed".into(),
            View::Project(id) => self.store.project_name(id).to_string(),
            View::Filter(id) => self.store.filters.get(id).map(|f| f.name.clone()).unwrap_or_default(),
            View::Label(n) => n.clone(),
        }
    }

    fn keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let modal = self.editor.is_some() || self.adder.is_some() || self.settings.is_some();
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::CTRL, Key::K) && !modal {
                self.go_search();
            }
            if i.consume_key(egui::Modifiers::CTRL, Key::R) {
                self.send(Job::Sync);
            }
            if i.consume_key(egui::Modifiers::CTRL, Key::Z) && !typing {
                self.undo();
            }
            if i.consume_key(egui::Modifiers::ALT, Key::ArrowLeft) {
                self.nav_back();
            }
            if i.consume_key(egui::Modifiers::ALT, Key::ArrowRight) {
                self.nav_fwd();
            }
            if typing || modal {
                return;
            }
            if i.consume_key(egui::Modifiers::NONE, Key::Q) || i.consume_key(egui::Modifiers::NONE, Key::A) {
                self.adder = Some(Adder::new(self.context_defaults()));
            }
            if i.consume_key(egui::Modifiers::NONE, Key::Slash) || i.consume_key(egui::Modifiers::NONE, Key::F) {
                self.go_search();
            }
            if i.consume_key(egui::Modifiers::NONE, Key::M) {
                self.cfg.sidebar_open = !self.cfg.sidebar_open;
            }
        });
    }

    /// Handle tray clicks and hotkeys, keep the menu current, and send a closed window to the tray.
    fn tray_tick(&mut self, ctx: &egui::Context) {
        let Some(tray) = &self.tray else { return };
        for cmd in tray.poll() {
            if cmd.needs_window() {
                self.hidden = false;
            }
            self.run_cmd(cmd, ctx);
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.cfg.stay_in_tray && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.save_config();
            if let Some(t) = &self.tray {
                t.hide();
            }
            self.hidden = true;
        }
        let due = self.tray_at.is_none_or(|t| t.elapsed() > Duration::from_secs(2));
        if due {
            let today = Self::today();
            let entries = tray::model(&self.store, today, self.cfg.stay_in_tray);
            let tip = tray::tooltip(&self.store, today);
            if let Some(t) = &mut self.tray {
                t.update(entries, tip);
            }
            self.tray_at = Some(Instant::now());
        }
        if self.hidden {
            // Keep the menu current while no window shows.
            ctx.request_repaint_after(Duration::from_secs(10));
        }
    }

    fn run_cmd(&mut self, cmd: Cmd, ctx: &egui::Context) {
        match cmd {
            Cmd::AddTask => self.adder = Some(Adder::new(AddDefaults::default())),
            Cmd::Show => {}
            Cmd::OpenTask(id) => self.apply(vec![Action::Open(id)]),
            Cmd::Go(v) => self.go(v),
            Cmd::Settings => self.settings = Some(Settings::new(&self.cfg)),
            Cmd::ToggleStay => {
                self.cfg.stay_in_tray = !self.cfg.stay_in_tray;
                self.save_config();
                self.tray_at = None;
            }
            Cmd::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn go_search(&mut self) {
        self.go(View::Search);
        self.focus_search = true;
    }

    fn nav_back(&mut self) {
        if let Some(v) = self.back.pop() {
            self.fwd.push(std::mem::replace(&mut self.view, v));
            self.on_enter();
        }
    }

    fn nav_fwd(&mut self) {
        if let Some(v) = self.fwd.pop() {
            self.back.push(std::mem::replace(&mut self.view, v));
            self.on_enter();
        }
    }

    /// Defaults for a new task based on the open view.
    pub fn context_defaults(&self) -> AddDefaults {
        match &self.view {
            View::Today => AddDefaults { due: "today".into(), ..Default::default() },
            View::Project(id) => AddDefaults { project_id: Some(id.clone()), ..Default::default() },
            View::Label(l) => AddDefaults { labels: vec![l.clone()], ..Default::default() },
            _ => AddDefaults::default(),
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, out: &mut Vec<Action>) {
        let today = Self::today();
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let name = if self.store.user.full_name.is_empty() { "Todoist".to_string() } else { self.store.user.full_name.clone() };
            let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 12.0, Color32::from_rgb(0x5a, 0x5a, 0x5a));
            let initial = name.chars().next().unwrap_or('T').to_uppercase().to_string();
            ui.painter().text(r.center(), Align2::CENTER_CENTER, initial, bold_font(12.0), TEXT);
            ui.label(egui::RichText::new(name).font(bold_font(14.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, Icon::Sidebar, "Close sidebar (M)").clicked() {
                    self.cfg.sidebar_open = false;
                }
                let sync = widgets::icon_button(ui, Icon::Sync, if self.busy { "Syncing…" } else { "Sync now (Ctrl+R)" });
                if self.busy {
                    ui.painter().circle_filled(sync.rect.right_top() + Vec2::new(-6.0, 6.0), 3.0, GREEN);
                }
                if sync.clicked() {
                    self.send(Job::Sync);
                    self.send(Job::Calendars(self.cfg.calendars.clone()));
                    self.completed_range = None;
                }
            });
        });
        ui.add_space(8.0);

        // Add task
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 6.0, HOVER);
        }
        let c = Pos2::new(rect.left() + 16.0, rect.center().y);
        ui.painter().circle_filled(c, 10.0, ACCENT);
        theme::icon(ui.painter(), Rect::from_center_size(c, Vec2::splat(12.0)), Icon::Plus, Color32::WHITE);
        ui.painter().text(Pos2::new(rect.left() + 34.0, rect.center().y), Align2::LEFT_CENTER, "Add task", bold_font(14.0), ACCENT_TEXT);
        if resp.on_hover_text("Q").clicked() {
            out.push(Action::AddTask(self.context_defaults()));
        }

        let inbox_id = self.store.inbox_id().unwrap_or("").to_string();
        let rows: [(Icon, &str, View, usize); 5] = [
            (Icon::Search, "Search", View::Search, 0),
            (Icon::Inbox, "Inbox", View::Inbox, self.store.project_count(&inbox_id)),
            (Icon::Today(today.day()), "Today", View::Today, self.store.today_count(today)),
            (Icon::Upcoming, "Upcoming", View::Upcoming, 0),
            (Icon::Filters, "Filters & Labels", View::FiltersLabels, 0),
        ];
        for (ic, label, v, n) in rows {
            let sel = self.view == v;
            if widgets::nav_row(ui, ic, MUTED, label, n, sel, 0.0).clicked() {
                out.push(Action::Go(v));
            }
        }

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // Favorites
            let mut favs: Vec<(Icon, Color32, String, View, usize)> = Vec::new();
            for (p, _) in self.store.project_tree() {
                if p.is_favorite {
                    favs.push((Icon::Hash, named_color(&p.color), p.name.clone(), View::Project(p.id.clone()), self.store.project_count(&p.id)));
                }
            }
            for f in self.store.sorted_filters() {
                if f.is_favorite {
                    let n = self.store.filter_results.get(&f.query).map(|v| v.iter().filter(|id| self.store.items.contains_key(*id)).count()).unwrap_or(0);
                    favs.push((Icon::Drop, named_color(&f.color), f.name.clone(), View::Filter(f.id.clone()), n));
                }
            }
            for l in self.store.sorted_labels() {
                if l.is_favorite {
                    let n = self.store.items.values().filter(|i| i.labels.contains(&l.name)).count();
                    favs.push((Icon::Tag, named_color(&l.color), l.name.clone(), View::Label(l.name.clone()), n));
                }
            }
            if !favs.is_empty() {
                section_title(ui, "Favorites");
                for (ic, col, name, v, n) in favs {
                    let sel = self.view == v;
                    if widgets::nav_row(ui, ic, col, &name, n, sel, 0.0).clicked() {
                        out.push(Action::Go(v));
                    }
                }
            }

            section_title(ui, "My Projects");
            let tree: Vec<(String, String, String, usize, bool)> = self
                .store
                .project_tree()
                .into_iter()
                .map(|(p, d)| (p.id.clone(), p.name.clone(), p.color.clone(), d, self.store.has_children(&p.id)))
                .collect();
            let mut hidden_below: Option<usize> = None;
            for (id, name, color, depth, parent) in tree {
                if let Some(d) = hidden_below {
                    if depth > d {
                        continue;
                    }
                    hidden_below = None;
                }
                let collapsed = self.cfg.collapsed.contains(&id);
                let v = View::Project(id.clone());
                let sel = self.view == v;
                let resp = widgets::nav_row(ui, Icon::Hash, named_color(&color), &name, self.store.project_count(&id), sel, depth as f32 * 20.0);
                if parent {
                    let chev = Rect::from_center_size(Pos2::new(resp.rect.right() - 34.0, resp.rect.center().y), Vec2::splat(18.0));
                    let hit = ui.interact(chev, ui.scope_id().with(("chev", &id)), Sense::click());
                    if resp.hovered() || hit.hovered() {
                        theme::icon(ui.painter(), chev, if collapsed { Icon::Right } else { Icon::Down }, MUTED);
                    }
                    if hit.clicked() {
                        if collapsed {
                            self.cfg.collapsed.remove(&id);
                        } else {
                            self.cfg.collapsed.insert(id.clone());
                        }
                        self.save_config();
                        continue;
                    }
                    if collapsed {
                        hidden_below = Some(depth);
                    }
                }
                if resp.clicked() {
                    out.push(Action::Go(v));
                }
            }
            ui.add_space(20.0);
        });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if !self.cfg.sidebar_open && widgets::icon_button(ui, Icon::Sidebar, "Open sidebar (M)").clicked() {
                self.cfg.sidebar_open = true;
            }
            ui.add_enabled_ui(!self.back.is_empty(), |ui| {
                if widgets::icon_button(ui, Icon::Left, "Back (Alt+Left)").clicked() {
                    self.nav_back();
                }
            });
            ui.add_enabled_ui(!self.fwd.is_empty(), |ui| {
                if widgets::icon_button(ui, Icon::Right, "Forward (Alt+Right)").clicked() {
                    self.nav_fwd();
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if widgets::icon_button(ui, Icon::Gear, "Settings").clicked() {
                    self.settings = Some(Settings::new(&self.cfg));
                }
                if self.view == View::Upcoming {
                    let label = match self.cfg.layout {
                        Layout::List => "List",
                        Layout::Week => "Week",
                        Layout::Month => "Month",
                    };
                    ui.menu_button(format!("Display: {label}"), |ui| {
                        for (l, n) in [(Layout::List, "List"), (Layout::Week, "Week"), (Layout::Month, "Month")] {
                            if ui.selectable_label(self.cfg.layout == l, n).clicked() {
                                self.cfg.layout = l;
                                self.scroll_week = true;
                                self.save_config();
                                ui.close();
                            }
                        }
                    });
                } else if self.view == View::Today {
                    let label = match self.cfg.today_group {
                        Group::None => "None",
                        Group::Project => "Project",
                        Group::Priority => "Priority",
                        Group::Date => "Date",
                    };
                    ui.menu_button(format!("Group: {label}"), |ui| {
                        for (g, n) in [(Group::None, "None"), (Group::Project, "Project"), (Group::Priority, "Priority")] {
                            if ui.selectable_label(self.cfg.today_group == g, n).clicked() {
                                self.cfg.today_group = g;
                                self.save_config();
                                ui.close();
                            }
                        }
                    });
                }
            });
        });
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some(t) = &self.toast else { return };
        if Instant::now() > t.until {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        let mut undo = false;
        let mut close = false;
        egui::Area::new(egui::Id::unique("toast")).anchor(Align2::LEFT_BOTTOM, Vec2::new(20.0, -20.0)).show(ctx, |ui| {
            egui::Frame::new().fill(Color32::from_rgb(0x33, 0x33, 0x33)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&t.text).color(if t.error { RED } else { TEXT }));
                    if t.undo.is_some() && ui.link(egui::RichText::new("Undo").color(ACCENT_TEXT)).clicked() {
                        undo = true;
                    }
                    if widgets::icon_button(ui, Icon::Close, "Dismiss").clicked() {
                        close = true;
                    }
                });
            });
        });
        if undo {
            self.undo();
        } else if close {
            self.toast = None;
        }
    }

    fn sign_in(&mut self, ui: &mut egui::Ui) {
        let mut token = ui.data_mut(|d| d.get_temp::<String>(egui::Id::unique("tok")).unwrap_or_default());
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.label(egui::RichText::new("Fastoist").font(bold_font(28.0)));
            ui.add_space(16.0);
            ui.add(egui::TextEdit::singleline(&mut token).password(true).hint_text("Todoist API token").desired_width(360.0));
            ui.add_space(8.0);
            let go = ui.add_enabled(!token.trim().is_empty(), egui::Button::new(egui::RichText::new("Connect").color(Color32::WHITE)).fill(ACCENT));
            ui.add_space(8.0);
            ui.hyperlink_to("Get your token", "https://app.todoist.com/app/settings/integrations/developer");
            if go.clicked() || (ui.input(|i| i.key_pressed(Key::Enter)) && !token.trim().is_empty()) {
                self.set_token(token.clone());
                token.clear();
            }
        });
        ui.data_mut(|d| d.insert_temp(egui::Id::unique("tok"), token));
    }
}

/// The native window handle, which the tray needs to show and hide the window.
fn native_window(cc: &eframe::CreationContext<'_>) -> Option<usize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as usize),
        _ => None,
    }
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(title).font(bold_font(13.0)).color(MUTED));
    });
    ui.add_space(2.0);
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_msgs();
        self.tray_tick(&ctx);
        self.keys(&ctx);
        let title = format!("{} – Fastoist", self.view_title());
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        let mut out = Vec::new();
        if self.cfg.token.is_empty() {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show(ui, |ui| self.sign_in(ui));
            return;
        }

        if self.cfg.sidebar_open {
            egui::Panel::left("sidebar")
                .resizable(true)
                .default_size(280.0)
                .min_size(220.0)
                .max_size(420.0)
                .frame(egui::Frame::new().fill(SIDEBAR).inner_margin(egui::Margin::symmetric(10, 4)))
                .show(ui, |ui| self.sidebar(ui, &mut out));
        }

        if self.view == View::Upcoming && self.cfg.plan_open {
            egui::Panel::right("plan")
                .resizable(true)
                .default_size(340.0)
                .min_size(260.0)
                .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(14, 8)).stroke(egui::Stroke::new(1.0, LINE)))
                .show(ui, |ui| crate::upcoming::plan_panel(self, ui, &mut out));
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(0, 6))).show(ui, |ui| {
            self.top_bar(ui);
            match self.view.clone() {
                View::Upcoming => crate::upcoming::show(self, ui, &mut out),
                v => crate::views::show(self, &v, ui, &mut out),
            }
        });

        crate::dialogs::show(self, &ctx, &mut out);
        self.apply(out);
        self.toast_ui(&ctx);
        self.save_cache();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_config();
        if let Ok(b) = serde_json::to_vec(&self.store) {
            save_bytes("cache.json", b);
        }
    }
}
