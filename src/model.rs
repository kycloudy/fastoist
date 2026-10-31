//! Todoist data types and the local store that mirrors the account.

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Due {
    pub date: String,
    pub string: String,
    pub is_recurring: bool,
    pub timezone: Option<String>,
    pub lang: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Deadline {
    pub date: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct TaskDuration {
    pub amount: i64,
    pub unit: String,
}

impl TaskDuration {
    pub fn minutes(&self) -> i64 {
        if self.unit == "day" { self.amount * 1440 } else { self.amount }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Item {
    pub id: String,
    pub content: String,
    pub description: String,
    pub project_id: String,
    pub section_id: Option<String>,
    pub parent_id: Option<String>,
    #[serde(alias = "order")]
    pub child_order: i64,
    pub day_order: i64,
    pub priority: u8,
    pub due: Option<Due>,
    pub deadline: Option<Deadline>,
    pub duration: Option<TaskDuration>,
    pub labels: Vec<String>,
    #[serde(alias = "is_completed")]
    pub checked: bool,
    pub is_deleted: bool,
    pub completed_at: Option<String>,
    #[serde(alias = "is_collapsed")]
    pub collapsed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub color: String,
    pub parent_id: Option<String>,
    #[serde(alias = "order")]
    pub child_order: i64,
    #[serde(alias = "is_inbox_project")]
    pub inbox_project: bool,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_deleted: bool,
    #[serde(alias = "is_collapsed")]
    pub collapsed: bool,
    pub view_style: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Section {
    pub id: String,
    pub name: String,
    pub project_id: String,
    #[serde(alias = "order")]
    pub section_order: i64,
    pub is_archived: bool,
    pub is_deleted: bool,
    #[serde(alias = "is_collapsed")]
    pub collapsed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(alias = "order")]
    pub item_order: i64,
    pub is_favorite: bool,
    pub is_deleted: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Filter {
    pub id: String,
    pub name: String,
    pub query: String,
    pub color: String,
    #[serde(alias = "order")]
    pub item_order: i64,
    pub is_favorite: bool,
    pub is_deleted: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct User {
    pub full_name: String,
    pub email: String,
    pub inbox_project_id: String,
    /// 1 = Monday ... 7 = Sunday.
    pub start_day: u8,
}

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct SyncResponse {
    pub sync_token: Option<String>,
    pub full_sync: bool,
    pub items: Option<Vec<Item>>,
    pub projects: Option<Vec<Project>>,
    pub sections: Option<Vec<Section>>,
    pub labels: Option<Vec<Label>>,
    pub filters: Option<Vec<Filter>>,
    pub user: Option<User>,
    pub sync_status: HashMap<String, Value>,
    pub temp_id_mapping: HashMap<String, String>,
}

/// A calendar event from an iCal feed or from Google Calendar.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    pub title: String,
    pub start: NaiveDateTime,
    /// For all-day events this is the first day after the event.
    pub end: NaiveDateTime,
    pub all_day: bool,
    /// Google event id. Empty for iCal events.
    #[serde(default)]
    pub id: String,
    /// Google calendar id. Empty for iCal events.
    #[serde(default)]
    pub cal: String,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// True when Fastoist can move or change the event.
    #[serde(default)]
    pub editable: bool,
}

impl Event {
    pub fn new(title: String, start: NaiveDateTime, end: NaiveDateTime, all_day: bool) -> Self {
        Self { title, start, end, all_day, id: String::new(), cal: String::new(), color: None, editable: false }
    }

    /// Identifies a Google event across calendars.
    pub fn key(&self) -> String {
        format!("{}|{}", self.cal, self.id)
    }
}

/// A Google calendar the user can show.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GCal {
    pub id: String,
    pub name: String,
    pub color: [u8; 3],
    pub writable: bool,
    pub primary: bool,
}

/// Where a task's due date falls, in local time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DueAt {
    pub date: NaiveDate,
    pub time: Option<NaiveTime>,
}

impl Due {
    pub fn at(&self) -> Option<DueAt> {
        let s = self.date.as_str();
        if s.len() == 10 {
            let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
            return Some(DueAt { date, time: None });
        }
        if s.ends_with('Z') {
            let dt = DateTime::parse_from_rfc3339(s).ok()?.with_timezone(&Local).naive_local();
            return Some(DueAt { date: dt.date(), time: Some(dt.time()) });
        }
        let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
            .ok()?;
        Some(DueAt { date: dt.date(), time: Some(dt.time()) })
    }
}

impl Deadline {
    pub fn day(&self) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(&self.date, "%Y-%m-%d").ok()
    }
}

impl Item {
    pub fn due_at(&self) -> Option<DueAt> {
        self.due.as_ref().and_then(Due::at)
    }
    pub fn due_date(&self) -> Option<NaiveDate> {
        self.due_at().map(|d| d.date)
    }
    pub fn is_recurring(&self) -> bool {
        self.due.as_ref().is_some_and(|d| d.is_recurring)
    }
}

/// The local mirror of the account, persisted between runs so startup is instant.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    pub sync_token: String,
    pub user: User,
    pub items: HashMap<String, Item>,
    pub projects: HashMap<String, Project>,
    pub sections: HashMap<String, Section>,
    pub labels: HashMap<String, Label>,
    pub filters: HashMap<String, Filter>,
    /// Everything shown: iCal events plus visible Google events. Rebuilt, never saved.
    #[serde(skip)]
    pub events: Vec<Event>,
    pub ical: Vec<Event>,
    pub gevents: Vec<Event>,
    pub gcals: Vec<GCal>,
    /// Task ids that match each saved filter query, keyed by query.
    pub filter_results: HashMap<String, Vec<String>>,
    /// Completed tasks fetched for the calendar, keyed by id.
    pub completed: HashMap<String, Item>,
}

fn merge<T: Clone>(
    map: &mut HashMap<String, T>,
    list: Option<Vec<T>>,
    full: bool,
    id: impl Fn(&T) -> &str,
    gone: impl Fn(&T) -> bool,
) {
    let Some(list) = list else { return };
    if full {
        map.clear();
    }
    for v in list {
        let key = id(&v).to_string();
        if gone(&v) {
            map.remove(&key);
        } else {
            map.insert(key, v);
        }
    }
}

impl Store {
    pub fn apply(&mut self, r: SyncResponse) {
        for (temp, real) in &r.temp_id_mapping {
            if let Some(mut it) = self.items.remove(temp) {
                it.id = real.clone();
                self.items.entry(real.clone()).or_insert(it);
            }
        }
        let full = r.full_sync;
        merge(&mut self.items, r.items, full, |i| &i.id, |i| i.is_deleted || i.checked);
        merge(&mut self.projects, r.projects, full, |p| &p.id, |p| p.is_deleted || p.is_archived);
        merge(&mut self.sections, r.sections, full, |s| &s.id, |s| s.is_deleted || s.is_archived);
        merge(&mut self.labels, r.labels, full, |l| &l.id, |l| l.is_deleted);
        merge(&mut self.filters, r.filters, full, |f| &f.id, |f| f.is_deleted);
        if let Some(u) = r.user {
            self.user = u;
        }
        if let Some(t) = r.sync_token {
            self.sync_token = t;
        }
    }

    pub fn inbox_id(&self) -> Option<&str> {
        if !self.user.inbox_project_id.is_empty() {
            return Some(&self.user.inbox_project_id);
        }
        self.projects.values().find(|p| p.inbox_project).map(|p| p.id.as_str())
    }

    pub fn project_name(&self, id: &str) -> &str {
        self.projects.get(id).map(|p| p.name.as_str()).unwrap_or("")
    }

    /// Projects in sidebar order: depth-first by child_order, with depth.
    pub fn project_tree(&self) -> Vec<(&Project, usize)> {
        let mut out = Vec::new();
        self.push_children(None, 0, &mut out);
        out
    }

    fn push_children<'a>(&'a self, parent: Option<&str>, depth: usize, out: &mut Vec<(&'a Project, usize)>) {
        let mut kids: Vec<&Project> = self
            .projects
            .values()
            .filter(|p| !p.inbox_project && p.parent_id.as_deref() == parent)
            .collect();
        kids.sort_by_key(|p| p.child_order);
        for p in kids {
            out.push((p, depth));
            self.push_children(Some(&p.id), depth + 1, out);
        }
    }

    pub fn has_children(&self, project_id: &str) -> bool {
        self.projects.values().any(|p| p.parent_id.as_deref() == Some(project_id))
    }

    pub fn sections_of(&self, project_id: &str) -> Vec<&Section> {
        let mut v: Vec<&Section> = self.sections.values().filter(|s| s.project_id == project_id).collect();
        v.sort_by_key(|s| s.section_order);
        v
    }

    pub fn project_count(&self, project_id: &str) -> usize {
        self.items.values().filter(|i| i.project_id == project_id).count()
    }

    pub fn subtasks(&self, parent: &str) -> Vec<&Item> {
        let mut v: Vec<&Item> = self.items.values().filter(|i| i.parent_id.as_deref() == Some(parent)).collect();
        v.sort_by_key(|i| i.child_order);
        v
    }

    pub fn sorted_labels(&self) -> Vec<&Label> {
        let mut v: Vec<&Label> = self.labels.values().collect();
        v.sort_by_key(|l| l.item_order);
        v
    }

    pub fn sorted_filters(&self) -> Vec<&Filter> {
        let mut v: Vec<&Filter> = self.filters.values().collect();
        v.sort_by_key(|f| f.item_order);
        v
    }


    /// Overdue and due-today tasks.
    pub fn today_count(&self, today: NaiveDate) -> usize {
        self.items.values().filter(|i| i.due_date().is_some_and(|d| d <= today)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn due(date: &str) -> Due {
        Due { date: date.into(), ..Default::default() }
    }

    #[test]
    fn due_forms() {
        let d = due("2026-10-31").at().unwrap();
        assert_eq!(d.date, NaiveDate::from_ymd_opt(2026, 10, 31).unwrap());
        assert!(d.time.is_none());
        let t = due("2026-11-03T18:30:00").at().unwrap();
        assert_eq!(t.time, NaiveTime::from_hms_opt(18, 30, 0));
        assert!(due("2026-11-03T18:30:00Z").at().is_some());
    }

    #[test]
    fn incremental_sync_removes_completed_and_maps_temp_ids() {
        let mut s = Store::default();
        s.items.insert("tmp".into(), Item { id: "tmp".into(), content: "a".into(), ..Default::default() });
        s.items.insert("x".into(), Item { id: "x".into(), ..Default::default() });
        let r = SyncResponse {
            items: Some(vec![Item { id: "x".into(), checked: true, ..Default::default() }]),
            temp_id_mapping: [("tmp".to_string(), "real".to_string())].into(),
            sync_token: Some("t2".into()),
            ..Default::default()
        };
        s.apply(r);
        assert!(s.items.contains_key("real"));
        assert!(!s.items.contains_key("tmp"));
        assert!(!s.items.contains_key("x"));
        assert_eq!(s.sync_token, "t2");
    }
}
