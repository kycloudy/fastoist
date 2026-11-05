//! Todoist API v1 client and the background worker that talks to it.
//!
//! The UI never blocks on the network: it sends `Job`s to one worker thread and
//! applies the `Msg`s that come back.

use crate::gcal::{self, Creds, Session};
use crate::ics;
use crate::model::{Event, GCal, Item, SyncResponse};
use chrono::{Duration, Local, NaiveDate, NaiveDateTime};
use serde_json::{json, Value};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration as StdDuration;

const BASE: &str = "https://api.todoist.com/api/v1";
const AUTO_SYNC: StdDuration = StdDuration::from_secs(60);

/// Fields set in the add dialog that quick add does not take directly.
#[derive(Clone, Debug, Default)]
pub struct AddExtras {
    pub description: String,
    pub project_id: Option<String>,
    pub section_id: Option<String>,
    pub parent_id: Option<String>,
    pub priority: Option<u8>,
    pub due: Option<Value>,
    pub deadline: Option<String>,
    pub labels: Vec<String>,
    /// `reminder_add` arguments; the new task's id is filled in once it exists.
    pub reminders: Vec<Value>,
}

pub enum Job {
    SetToken(String),
    FullSync,
    Sync,
    Commands(Vec<Value>),
    QuickAdd { text: String, temp_id: String, extras: AddExtras },
    Filter(String),
    Completed { since: NaiveDate, until: NaiveDate },
    Calendars(Vec<String>),
    /// Sign in to Google (or out, with `None`) and load calendars and events.
    GoogleSet { creds: Option<Creds>, hidden: Vec<String> },
    GoogleSync { hidden: Vec<String> },
    GoogleCreate { cal: String, title: String, start: NaiveDateTime, end: NaiveDateTime, all_day: bool },
    GoogleUpdate { cal: String, id: String, title: Option<String>, start: NaiveDateTime, end: NaiveDateTime, all_day: bool },
    GoogleDelete { cal: String, id: String },
}

pub enum Msg {
    Synced(SyncResponse),
    FilterResult { query: String, items: Vec<Item> },
    Completed(Vec<Item>),
    Events(Vec<Event>),
    GoogleCalendars(Vec<GCal>),
    GoogleEvents(Vec<Event>),
    /// Google refused the saved sign-in.
    GoogleExpired,
    /// The browser sign-in finished.
    GoogleSignedIn(Result<Creds, String>),
    QuickAddDone { temp_id: String },
    Error(String),
    Busy(bool),
}

pub fn command(kind: &str, args: Value) -> Value {
    json!({ "type": kind, "uuid": uuid::Uuid::new_v4().to_string(), "args": args })
}

pub fn add_command(temp_id: &str, args: Value) -> Value {
    json!({ "type": "item_add", "uuid": uuid::Uuid::new_v4().to_string(), "temp_id": temp_id, "args": args })
}

struct Client {
    agent: ureq::Agent,
    token: String,
    sync_token: String,
    google: Option<Session>,
    hidden: Vec<String>,
    /// Auto-sync ticks since Google was last read.
    g_ticks: u32,
}

type Res<T> = Result<T, String>;

impl Client {
    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    fn sync(&mut self, commands: &[Value]) -> Res<SyncResponse> {
        let token = if self.sync_token.is_empty() { "*".to_string() } else { self.sync_token.clone() };
        let cmds = serde_json::to_string(commands).unwrap_or_default();
        let mut form = vec![
            ("sync_token", token.as_str()),
            ("resource_types", r#"["items","projects","sections","labels","filters","reminders","user"]"#),
        ];
        if !commands.is_empty() {
            form.push(("commands", cmds.as_str()));
        }
        let r: SyncResponse = self
            .agent
            .post(format!("{BASE}/sync"))
            .header("Authorization", &self.auth())
            .send_form(form)
            .map_err(http_err)?
            .body_mut()
            .with_config()
            .limit(256 * 1024 * 1024)
            .read_json()
            .map_err(|e| e.to_string())?;
        if let Some(t) = &r.sync_token {
            self.sync_token = t.clone();
        }
        Ok(r)
    }

    fn get_paged(&self, path: &str, params: &[(&str, String)], key: &str) -> Res<Vec<Item>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut req = self.agent.get(format!("{BASE}/{path}")).header("Authorization", &self.auth());
            for (k, v) in params {
                req = req.query(*k, v);
            }
            req = req.query("limit", "200");
            if let Some(c) = &cursor {
                req = req.query("cursor", c);
            }
            let v: Value = req.call().map_err(http_err)?.body_mut().read_json().map_err(|e| e.to_string())?;
            if let Some(arr) = v.get(key).and_then(Value::as_array) {
                out.extend(arr.iter().filter_map(|x| serde_json::from_value(x.clone()).ok()));
            }
            match v.get("next_cursor").and_then(Value::as_str) {
                Some(c) if !c.is_empty() => cursor = Some(c.to_string()),
                _ => return Ok(out),
            }
        }
    }

    fn quick_add(&self, text: &str) -> Res<Value> {
        self.agent
            .post(format!("{BASE}/tasks/quick"))
            .header("Authorization", &self.auth())
            .send_json(json!({ "text": text }))
            .map_err(http_err)?
            .body_mut()
            .read_json()
            .map_err(|e| e.to_string())
    }

    fn fetch_text(&self, url: &str) -> Res<String> {
        let url = url.trim().replacen("webcal://", "https://", 1);
        if !url.starts_with("https://") && !url.starts_with("http://") {
            // A local .ics file exported from a calendar app.
            return std::fs::read_to_string(&url).map_err(|e| format!("Calendar file {url}: {e}"));
        }
        self.agent
            .get(&url)
            .call()
            .map_err(http_err)?
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_string()
            .map_err(|e| e.to_string())
    }
}

fn http_err(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(401) | ureq::Error::StatusCode(403) => {
            "Todoist rejected the API token. Open Settings and paste a new token.".into()
        }
        ureq::Error::StatusCode(429) => "Todoist rate limit reached. Sync will retry shortly.".into(),
        other => format!("Network error: {other}"),
    }
}

/// Follow-up commands that apply the add dialog's fields to a task made by quick add.
fn extras_commands(task: &Value, x: &AddExtras, typed_project: bool) -> Vec<Value> {
    let Some(id) = task.get("id").and_then(Value::as_str) else { return vec![] };
    let mut cmds = Vec::new();
    let mut upd = serde_json::Map::new();
    upd.insert("id".into(), json!(id));
    if !x.description.is_empty() {
        upd.insert("description".into(), json!(x.description));
    }
    // A date or priority typed in the text wins over the dialog's fields.
    let typed_due = task.get("due").is_some_and(|d| !d.is_null());
    let typed_priority = task.get("priority").and_then(Value::as_u64).unwrap_or(1) > 1;
    if let Some(p) = x.priority.filter(|_| !typed_priority) {
        upd.insert("priority".into(), json!(p));
    }
    if let Some(due) = x.due.as_ref().filter(|_| !typed_due) {
        upd.insert("due".into(), due.clone());
    }
    if let Some(d) = &x.deadline {
        upd.insert("deadline".into(), json!({ "date": d }));
    }
    if !x.labels.is_empty() {
        let mut labels: Vec<String> = task
            .get("labels")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|l| l.as_str().map(String::from)).collect())
            .unwrap_or_default();
        for l in &x.labels {
            if !labels.contains(l) {
                labels.push(l.clone());
            }
        }
        upd.insert("labels".into(), json!(labels));
    }
    if upd.len() > 1 {
        cmds.push(command("item_update", Value::Object(upd)));
    }
    for r in &x.reminders {
        let mut args = r.clone();
        args["item_id"] = json!(id);
        cmds.push(command("reminder_add", args));
    }
    if let Some(parent) = &x.parent_id {
        cmds.push(command("item_move", json!({ "id": id, "parent_id": parent })));
    } else if let Some(section) = &x.section_id {
        cmds.push(command("item_move", json!({ "id": id, "section_id": section })));
    } else if let Some(project) = &x.project_id {
        // A #project typed in the text wins over the dialog's project.
        let landed = task.get("project_id").and_then(Value::as_str).unwrap_or("");
        if landed != project && !typed_project {
            cmds.push(command("item_move", json!({ "id": id, "project_id": project })));
        }
    }
    cmds
}

pub fn spawn(token: String, sync_token: String, rx: Receiver<Job>, tx: Sender<Msg>, wake: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(StdDuration::from_secs(60)))
            .user_agent("fastoist/0.1")
            .build()
            .into();
        let mut c = Client { agent, token, sync_token, google: None, hidden: Vec::new(), g_ticks: 0 };
        let send = |m: Msg| {
            let _ = tx.send(m);
            wake();
        };
        loop {
            let job = match rx.recv_timeout(AUTO_SYNC) {
                Ok(j) => j,
                Err(RecvTimeoutError::Timeout) => {
                    c.g_ticks += 1;
                    if c.google.is_some() && c.g_ticks >= 5 {
                        Job::GoogleSync { hidden: c.hidden.clone() }
                    } else {
                        Job::Sync
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return,
            };
            let google_job = matches!(job, Job::GoogleSet { .. } | Job::GoogleSync { .. } | Job::GoogleCreate { .. } | Job::GoogleUpdate { .. } | Job::GoogleDelete { .. });
            if c.token.is_empty() && !google_job && !matches!(job, Job::SetToken(_) | Job::Calendars(_)) {
                continue;
            }
            send(Msg::Busy(true));
            let result = run(&mut c, job, &rx, &send);
            if let Err(e) = result {
                send(Msg::Error(e));
            }
            send(Msg::Busy(false));
        }
    });
}

fn run(c: &mut Client, job: Job, rx: &Receiver<Job>, send: &impl Fn(Msg)) -> Res<()> {
    match job {
        Job::SetToken(t) => {
            c.token = t;
            c.sync_token.clear();
            send(Msg::Synced(c.sync(&[])?));
        }
        Job::FullSync => {
            c.sync_token.clear();
            send(Msg::Synced(c.sync(&[])?));
        }
        Job::Sync => send(Msg::Synced(c.sync(&[])?)),
        Job::Commands(mut cmds) => {
            // Batch every queued command into one request.
            let mut later = Vec::new();
            while let Ok(j) = rx.try_recv() {
                match j {
                    Job::Commands(more) => cmds.extend(more),
                    Job::Sync => {}
                    other => later.push(other),
                }
            }
            let r = c.sync(&cmds)?;
            let errors: Vec<String> = r
                .sync_status
                .values()
                .filter(|v| v.as_str() != Some("ok"))
                .map(|v| v.get("error").and_then(Value::as_str).unwrap_or("Todoist refused a change").to_string())
                .collect();
            send(Msg::Synced(r));
            if let Some(e) = errors.into_iter().next() {
                send(Msg::Error(e));
            }
            for j in later {
                run(c, j, rx, send)?;
            }
        }
        Job::QuickAdd { text, temp_id, extras } => {
            // Remove the placeholder only once the real task has arrived, so nothing flickers.
            let task = match c.quick_add(&text) {
                Ok(t) => t,
                Err(e) => {
                    send(Msg::QuickAddDone { temp_id });
                    return Err(e);
                }
            };
            let typed_project = text.split_whitespace().any(|w| w.starts_with('#') && w.len() > 1);
            let cmds = extras_commands(&task, &extras, typed_project);
            let synced = c.sync(&cmds);
            send(Msg::QuickAddDone { temp_id });
            send(Msg::Synced(synced?));
        }
        Job::Filter(query) => {
            let items = c.get_paged("tasks/filter", &[("query", query.clone())], "results")?;
            send(Msg::FilterResult { query, items });
        }
        Job::Completed { since, until } => {
            // The API accepts at most six weeks per request.
            let mut items = Vec::new();
            let mut from = since;
            while from <= until {
                let to = (from + Duration::days(41)).min(until);
                let params = [
                    ("since", format!("{}T00:00:00Z", from)),
                    ("until", format!("{}T23:59:59Z", to)),
                ];
                items.extend(c.get_paged("tasks/completed/by_due_date", &params, "items")?);
                from = to + Duration::days(1);
            }
            send(Msg::Completed(items));
        }
        Job::Calendars(urls) => {
            let today = Local::now().date_naive();
            let (from, to) = (today - Duration::days(120), today + Duration::days(400));
            let mut events = Vec::new();
            for url in urls.iter().filter(|u| !u.trim().is_empty()) {
                let text = c.fetch_text(url)?;
                events.extend(ics::parse(&text, from, to));
            }
            send(Msg::Events(events));
        }
        Job::GoogleSet { creds, hidden } => {
            if let Some(old) = c.google.take() {
                if creds.is_none() {
                    old.revoke(&c.agent);
                }
            }
            c.hidden = hidden;
            match creds {
                Some(creds) => {
                    c.google = Some(Session::new(creds));
                    google_load(c, send)?;
                }
                None => {
                    send(Msg::GoogleCalendars(Vec::new()));
                    send(Msg::GoogleEvents(Vec::new()));
                }
            }
        }
        Job::GoogleSync { hidden } => {
            c.hidden = hidden;
            google_load(c, send)?;
        }
        Job::GoogleCreate { cal, title, start, end, all_day } => {
            let r = with_google(c, send, |s, a| s.create(a, &cal, &title, start, end, all_day));
            google_load(c, send)?;
            r?;
        }
        Job::GoogleUpdate { cal, id, title, start, end, all_day } => {
            let r = with_google(c, send, |s, a| s.update(a, &cal, &id, title.as_deref(), start, end, all_day));
            google_load(c, send)?;
            r?;
        }
        Job::GoogleDelete { cal, id } => {
            let r = with_google(c, send, |s, a| s.delete(a, &cal, &id));
            google_load(c, send)?;
            r?;
        }
    }
    Ok(())
}

/// Run a Google call. When Google rejects the saved sign-in, tell the app to forget it.
fn with_google<T>(c: &mut Client, send: &impl Fn(Msg), f: impl FnOnce(&mut Session, &ureq::Agent) -> Res<T>) -> Res<T> {
    let Some(s) = c.google.as_mut() else { return Err("Not signed in to Google".into()) };
    let r = f(s, &c.agent);
    if r.as_ref().is_err_and(|e| e == gcal::EXPIRED) {
        c.google = None;
        send(Msg::GoogleExpired);
    }
    r
}

/// Fetch the calendar list and the events of every visible calendar.
fn google_load(c: &mut Client, send: &impl Fn(Msg)) -> Res<()> {
    c.g_ticks = 0;
    let hidden = c.hidden.clone();
    let today = Local::now().date_naive();
    let (from, to) = (today - Duration::days(60), today + Duration::days(240));
    let cals = with_google(c, send, |s, a| s.calendars(a))?;
    send(Msg::GoogleCalendars(cals.clone()));
    let mut events = Vec::new();
    for cal in cals.iter().filter(|k| !hidden.contains(&k.id)) {
        events.extend(with_google(c, send, |s, a| s.events(a, cal, from, to))?);
    }
    send(Msg::GoogleEvents(events));
    Ok(())
}
