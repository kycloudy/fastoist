//! System tray: the icon, its menu, global hotkeys, and the single-instance check.
//!
//! The menu content comes from `model`, which is plain data and works on every platform.
//! The Windows glue is in `platform`; other systems get a no-op `Tray`.

use crate::app::View;
use crate::model::{Item, Store};
use crate::widgets::{date_sort, fmt_time};
use chrono::{Duration, NaiveDate, NaiveTime};

/// Longest task or event title shown in the menu, in characters.
const MAX_TITLE: usize = 44;
/// Most rows listed under Today and under Tomorrow.
const MAX_ROWS: usize = 6;

pub const ADD_HOTKEY: &str = "Ctrl+Space";
pub const SHOW_HOTKEY: &str = "Win+Alt+S";

/// What a menu click or a hotkey asks the app to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    AddTask,
    Show,
    OpenTask(String),
    Go(View),
    Settings,
    ToggleStay,
    Quit,
}

impl Cmd {
    /// True when the command needs the main window on screen.
    pub fn needs_window(&self) -> bool {
        !matches!(self, Cmd::ToggleStay | Cmd::Quit)
    }

    fn from_id(id: &str) -> Option<Cmd> {
        Some(match id {
            "add" => Cmd::AddTask,
            "show" => Cmd::Show,
            "settings" => Cmd::Settings,
            "stay" => Cmd::ToggleStay,
            "quit" => Cmd::Quit,
            "go:inbox" => Cmd::Go(View::Inbox),
            "go:today" => Cmd::Go(View::Today),
            "go:upcoming" => Cmd::Go(View::Upcoming),
            "go:filters" => Cmd::Go(View::FiltersLabels),
            "go:completed" => Cmd::Go(View::Completed),
            _ => {
                if let Some(t) = id.strip_prefix("task:") {
                    Cmd::OpenTask(t.to_string())
                } else if let Some(p) = id.strip_prefix("go:p:") {
                    Cmd::Go(View::Project(p.to_string()))
                } else if let Some(f) = id.strip_prefix("go:f:") {
                    Cmd::Go(View::Filter(f.to_string()))
                } else if let Some(l) = id.strip_prefix("go:l:") {
                    Cmd::Go(View::Label(l.to_string()))
                } else {
                    return None;
                }
            }
        })
    }
}

/// One row of the tray menu. `id` is what comes back in `Cmd::from_id`.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Item { id: String, text: String },
    Header(String),
    Check { id: String, text: String, checked: bool },
    Sub { text: String, items: Vec<Entry> },
    Sep,
}

fn item(id: &str, text: &str) -> Entry {
    Entry::Item { id: id.into(), text: text.into() }
}

/// Windows menus read `&` as a shortcut marker, so double it. Long titles get cut.
fn label(s: &str) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = if s.chars().count() > MAX_TITLE { s.chars().take(MAX_TITLE - 1).chain(['…']).collect() } else { s };
    cut.replace('&', "&&")
}

/// A task or event on one menu day, with the sort keys that order it.
struct Row {
    time: Option<NaiveTime>,
    event: bool,
    entry: Entry,
}

fn day_rows(store: &Store, day: NaiveDate, include_overdue: bool) -> Vec<Entry> {
    let mut tasks: Vec<&Item> = store
        .items
        .values()
        .filter(|i| i.due_date().is_some_and(|d| d == day || (include_overdue && d < day)))
        .collect();
    tasks.sort_by(|a, b| date_sort(a, b));
    let mut rows: Vec<Row> = tasks
        .into_iter()
        .map(|t| {
            let time = t.due_at().and_then(|d| d.time);
            let text = match time {
                Some(tm) => format!("{} {}", fmt_time(tm), label(&t.content)),
                None => label(&t.content),
            };
            Row { time, event: false, entry: Entry::Item { id: format!("task:{}", t.id), text } }
        })
        .collect();
    for e in store.events.iter().filter(|e| {
        if e.all_day {
            e.start.date() <= day && day < e.end.date().max(e.start.date() + Duration::days(1))
        } else {
            e.start.date() == day
        }
    }) {
        let time = (!e.all_day).then(|| e.start.time());
        let text = match time {
            Some(tm) => format!("{} {}", fmt_time(tm), label(&e.title)),
            None => label(&e.title),
        };
        rows.push(Row { time, event: true, entry: Entry::Item { id: "go:upcoming".into(), text } });
    }
    // Timed rows by clock, then all-day events, then tasks without a time.
    rows.sort_by_key(|r| (r.time.is_none() && !r.event, r.time.is_none(), r.time));
    rows.into_iter().map(|r| r.entry).collect()
}

fn section(out: &mut Vec<Entry>, title: &str, rows: Vec<Entry>, more_view: &str) {
    if rows.is_empty() {
        return;
    }
    out.push(Entry::Header(title.into()));
    let extra = rows.len().saturating_sub(MAX_ROWS);
    out.extend(rows.into_iter().take(MAX_ROWS));
    if extra > 0 {
        out.push(Entry::Item { id: more_view.into(), text: format!("{extra} more…") });
    }
}

/// The favorites submenu: projects, filters, then labels, as in the sidebar.
fn favorites(store: &Store) -> Vec<Entry> {
    let mut v = Vec::new();
    for (p, _) in store.project_tree() {
        if p.is_favorite {
            v.push(Entry::Item { id: format!("go:p:{}", p.id), text: label(&p.name) });
        }
    }
    for f in store.sorted_filters() {
        if f.is_favorite {
            v.push(Entry::Item { id: format!("go:f:{}", f.id), text: label(&f.name) });
        }
    }
    for l in store.sorted_labels() {
        if l.is_favorite {
            v.push(Entry::Item { id: format!("go:l:{}", l.name), text: label(&l.name) });
        }
    }
    v
}

/// Build the whole menu for `today`.
pub fn model(store: &Store, today: NaiveDate, stay: bool) -> Vec<Entry> {
    let mut m = vec![item("add", &format!("Add task\t{ADD_HOTKEY}")), item("show", &format!("Show Fastoist\t{SHOW_HOTKEY}")), Entry::Sep];
    let before = m.len();
    section(&mut m, "Today", day_rows(store, today, true), "go:today");
    section(&mut m, "Tomorrow", day_rows(store, today + Duration::days(1), false), "go:upcoming");
    if m.len() > before {
        m.push(Entry::Sep);
    }
    m.extend([
        item("go:inbox", "Inbox"),
        item("go:today", "Today"),
        item("go:upcoming", "Upcoming"),
        item("go:filters", "Filters and Labels"),
        item("go:completed", "Completed"),
        Entry::Sep,
    ]);
    let favs = favorites(store);
    if !favs.is_empty() {
        m.push(Entry::Sub { text: "Favorites".into(), items: favs });
        m.push(Entry::Sep);
    }
    m.extend([
        item("settings", "Settings"),
        Entry::Check { id: "stay".into(), text: "Stay running in tray".into(), checked: stay },
        Entry::Sep,
        item("quit", "Quit Fastoist"),
    ]);
    m
}

/// Hover text for the tray icon.
pub fn tooltip(store: &Store, today: NaiveDate) -> String {
    match store.today_count(today) {
        0 => "Fastoist".into(),
        1 => "Fastoist: 1 task today".into(),
        n => format!("Fastoist: {n} tasks today"),
    }
}

#[cfg(windows)]
mod platform {
    use super::{Cmd, Entry};
    use eframe::egui;
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
    use std::ffi::c_void;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use tray_icon::menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject, INFINITE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, SetForegroundWindow, ShowWindow, SW_HIDE, SW_RESTORE, SW_SHOW};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Returns false when another Fastoist already runs. In that case it asks that one to show itself.
    pub fn claim_instance() -> bool {
        let mutex = wide("Local\\Fastoist.Instance");
        let event = wide("Local\\Fastoist.Show");
        unsafe {
            let _m = CreateMutexW(std::ptr::null(), 0, mutex.as_ptr());
            let exists = GetLastError() == ERROR_ALREADY_EXISTS;
            let ev = CreateEventW(std::ptr::null(), 0, 0, event.as_ptr());
            if exists {
                if !ev.is_null() {
                    SetEvent(ev);
                }
                return false;
            }
        }
        true
    }

    /// Wait for a second launch and treat it as a request to show the window.
    fn watch_second_launch(send: impl Fn(Cmd) + Send + 'static) {
        let event = wide("Local\\Fastoist.Show");
        let ev = unsafe { CreateEventW(std::ptr::null(), 0, 0, event.as_ptr()) } as usize;
        if ev == 0 {
            return;
        }
        std::thread::spawn(move || loop {
            if unsafe { WaitForSingleObject(ev as *mut c_void, INFINITE) } != 0 {
                break;
            }
            send(Cmd::Show);
        });
    }

    pub fn show_window(hwnd: usize) {
        let h = hwnd as *mut c_void;
        unsafe {
            ShowWindow(h, if IsIconic(h) != 0 { SW_RESTORE } else { SW_SHOW });
            SetForegroundWindow(h);
        }
    }

    pub fn hide_window(hwnd: usize) {
        unsafe { ShowWindow(hwnd as *mut c_void, SW_HIDE) };
    }

    fn build(entries: &[Entry]) -> Vec<Box<dyn IsMenuItem>> {
        entries
            .iter()
            .map(|e| -> Box<dyn IsMenuItem> {
                match e {
                    Entry::Item { id, text } => Box::new(MenuItem::with_id(id.as_str(), text, true, None)),
                    Entry::Header(t) => Box::new(MenuItem::new(t, false, None)),
                    Entry::Check { id, text, checked } => Box::new(CheckMenuItem::with_id(id.as_str(), text, true, *checked, None)),
                    Entry::Sep => Box::new(PredefinedMenuItem::separator()),
                    Entry::Sub { text, items } => {
                        let kids = build(items);
                        let refs: Vec<&dyn IsMenuItem> = kids.iter().map(|k| k.as_ref()).collect();
                        Box::new(Submenu::with_items(text, true, &refs).expect("submenu"))
                    }
                }
            })
            .collect()
    }

    /// Runs a command's window side at once, on the event thread, so the window shows
    /// even when the UI is not drawing frames.
    fn dispatch(cmd: Cmd, tx: &Sender<Cmd>, ctx: &egui::Context, hwnd: usize) {
        if cmd.needs_window() {
            show_window(hwnd);
        }
        if cmd == Cmd::Quit {
            // If the UI cannot close the window in time, leave anyway.
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(3));
                std::process::exit(0);
            });
        }
        let _ = tx.send(cmd);
        ctx.request_repaint();
    }

    pub struct Tray {
        icon: TrayIcon,
        rx: Receiver<Cmd>,
        shown: Vec<Entry>,
        tip: String,
        hwnd: usize,
        _hotkeys: Option<GlobalHotKeyManager>,
    }

    impl Tray {
        pub fn new(ctx: &egui::Context, hwnd: usize) -> Option<Self> {
            let (tx, rx) = channel();
            let (w, h, rgba) = {
                let i = crate::icon();
                (i.width, i.height, i.rgba)
            };
            let icon = TrayIconBuilder::new()
                .with_icon(tray_icon::Icon::from_rgba(rgba, w, h).ok()?)
                .with_tooltip("Fastoist")
                .with_menu_on_left_click(false)
                .build()
                .ok()?;

            let (t, c) = (tx.clone(), ctx.clone());
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                if let Some(cmd) = Cmd::from_id(e.id.as_ref()) {
                    dispatch(cmd, &t, &c, hwnd);
                }
            }));
            let (t, c) = (tx.clone(), ctx.clone());
            TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| match e {
                TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                    dispatch(Cmd::Show, &t, &c, hwnd)
                }
                _ => {}
            }));

            // A taken hotkey is not an error: the menu still works.
            let hotkeys = GlobalHotKeyManager::new().ok();
            let mut ids = Vec::new();
            if let Some(m) = &hotkeys {
                let add = HotKey::new(Some(Modifiers::CONTROL), Code::Space);
                let show = HotKey::new(Some(Modifiers::META | Modifiers::ALT), Code::KeyS);
                for (hk, cmd) in [(add, Cmd::AddTask), (show, Cmd::Show)] {
                    if m.register(hk).is_ok() {
                        ids.push((hk.id(), cmd));
                    }
                }
            }
            let (t, c) = (tx.clone(), ctx.clone());
            GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                if e.state == HotKeyState::Pressed {
                    if let Some((_, cmd)) = ids.iter().find(|(id, _)| *id == e.id) {
                        dispatch(cmd.clone(), &t, &c, hwnd);
                    }
                }
            }));

            let (t, c) = (tx, ctx.clone());
            watch_second_launch(move |cmd| dispatch(cmd, &t, &c, hwnd));
            Some(Self { icon, rx, shown: Vec::new(), tip: String::new(), hwnd, _hotkeys: hotkeys })
        }

        pub fn poll(&self) -> Vec<Cmd> {
            self.rx.try_iter().collect()
        }

        /// Replace the menu when it changed.
        pub fn update(&mut self, entries: Vec<Entry>, tip: String) {
            if entries != self.shown {
                let items = build(&entries);
                let refs: Vec<&dyn IsMenuItem> = items.iter().map(|i| i.as_ref()).collect();
                if let Ok(menu) = Menu::with_items(&refs) {
                    self.icon.set_menu(Some(Box::new(menu)));
                    self.shown = entries;
                }
            }
            if tip != self.tip {
                let _ = self.icon.set_tooltip(Some(&tip));
                self.tip = tip;
            }
        }

        pub fn hide(&self) {
            hide_window(self.hwnd);
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{Cmd, Entry};
    use eframe::egui;

    pub fn claim_instance() -> bool {
        true
    }

    pub struct Tray;

    impl Tray {
        pub fn new(_ctx: &egui::Context, _hwnd: usize) -> Option<Self> {
            None
        }
        pub fn poll(&self) -> Vec<Cmd> {
            Vec::new()
        }
        pub fn update(&mut self, _entries: Vec<Entry>, _tip: String) {}
        pub fn hide(&self) {}
    }
}

pub use platform::{claim_instance, Tray};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Due, Event, Filter, Project};
    use chrono::NaiveDateTime;

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn task(id: &str, content: &str, due: &str) -> Item {
        Item { id: id.into(), content: content.into(), due: Some(Due { date: due.into(), ..Default::default() }), ..Default::default() }
    }

    fn texts(m: &[Entry]) -> Vec<String> {
        m.iter()
            .map(|e| match e {
                Entry::Item { text, .. } | Entry::Check { text, .. } | Entry::Sub { text, .. } => text.clone(),
                Entry::Header(t) => format!("[{t}]"),
                Entry::Sep => "-".into(),
            })
            .collect()
    }

    #[test]
    fn today_and_tomorrow_sections() {
        let mut s = Store::default();
        s.items.insert("a".into(), task("a", "Midterm", "2026-10-31"));
        s.items.insert("b".into(), task("b", "Essay & notes", "2026-11-01"));
        s.items.insert("c".into(), task("c", "Late one", "2026-10-30"));
        s.events.push(Event::new(
            "halloween".into(),
            NaiveDateTime::parse_from_str("2026-10-31 16:00", "%Y-%m-%d %H:%M").unwrap(),
            NaiveDateTime::parse_from_str("2026-10-31 17:00", "%Y-%m-%d %H:%M").unwrap(),
            false,
        ));
        let m = texts(&model(&s, day("2026-10-31"), true));
        let start = m.iter().position(|t| t == "[Today]").unwrap();
        // The timed event comes first, then tasks without a time.
        assert_eq!(&m[start..start + 4], ["[Today]", "4 PM halloween", "Late one", "Midterm"]);
        let tm = m.iter().position(|t| t == "[Tomorrow]").unwrap();
        assert_eq!(m[tm + 1], "Essay && notes");
    }

    #[test]
    fn empty_days_have_no_sections_and_menu_ends_with_quit() {
        let m = texts(&model(&Store::default(), day("2026-10-31"), false));
        assert!(!m.iter().any(|t| t == "[Today]" || t == "[Tomorrow]"));
        assert_eq!(m.last().unwrap(), "Quit Fastoist");
        assert!(m[0].starts_with("Add task"));
        assert!(!m.iter().any(|t| t == "Favorites"));
    }

    #[test]
    fn long_lists_are_cut_with_a_count() {
        let mut s = Store::default();
        for i in 0..9 {
            s.items.insert(i.to_string(), task(&i.to_string(), &format!("t{i}"), "2026-10-31"));
        }
        let m = texts(&model(&s, day("2026-10-31"), true));
        assert!(m.contains(&"3 more…".to_string()));
    }

    #[test]
    fn favorites_and_command_ids() {
        let mut s = Store::default();
        s.projects.insert("p1".into(), Project { id: "p1".into(), name: "POL620".into(), is_favorite: true, ..Default::default() });
        s.filters.insert("f1".into(), Filter { id: "f1".into(), name: "Frog".into(), is_favorite: true, ..Default::default() });
        let m = model(&s, day("2026-10-31"), true);
        let Some(Entry::Sub { items, .. }) = m.iter().find(|e| matches!(e, Entry::Sub { .. })) else { panic!("no favorites") };
        let ids: Vec<_> = items.iter().filter_map(|e| if let Entry::Item { id, .. } = e { Some(id.as_str()) } else { None }).collect();
        assert_eq!(ids, ["go:p:p1", "go:f:f1"]);
        assert_eq!(Cmd::from_id("go:p:p1"), Some(Cmd::Go(View::Project("p1".into()))));
        assert_eq!(Cmd::from_id("task:9"), Some(Cmd::OpenTask("9".into())));
        assert_eq!(Cmd::from_id("nope"), None);
    }
}
