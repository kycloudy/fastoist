//! The settings window: a sidebar of pages, with changes applied as you make them.

use crate::app::{data_dir, App, Config};
use crate::theme::{self, Accent, Icon, Mode, Palette, *};
use crate::widgets;
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2};

pub const REPO: &str = "https://github.com/kycloudy/fastoist";
const TOKEN_URL: &str = "https://app.todoist.com/app/settings/integrations/developer";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    General,
    Appearance,
    Calendars,
    Tray,
    Shortcuts,
    About,
}

impl Page {
    pub const ALL: [Page; 6] = [Page::General, Page::Appearance, Page::Calendars, Page::Tray, Page::Shortcuts, Page::About];

    pub fn name(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Appearance => "Appearance",
            Page::Calendars => "Calendars",
            Page::Tray => "Tray",
            Page::Shortcuts => "Shortcuts",
            Page::About => "About",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Page::General => Icon::Gear,
            Page::Appearance => Icon::Palette,
            Page::Calendars => Icon::Calendar,
            Page::Tray => Icon::Tray,
            Page::Shortcuts => Icon::Keyboard,
            Page::About => Icon::Info,
        }
    }
}

/// Keys that work in the main window, as (what it does, keys). "or" separates two ways to do it.
pub const SHORTCUTS: &[(&str, &[&str])] = &[
    ("Add task", &["Q", "or", "A"]),
    ("Search", &["Ctrl", "K", "or", "/"]),
    ("Show or hide sidebar", &["M"]),
    ("Sync now", &["Ctrl", "R"]),
    ("Undo", &["Ctrl", "Z"]),
    ("Back", &["Alt", "Left"]),
    ("Forward", &["Alt", "Right"]),
    ("Scroll days in the week view", &["Shift", "Scroll", "or", "Middle-click"]),
    ("Close dialog", &["Esc"]),
];

/// Keys that work anywhere in Windows while Fastoist runs in the tray.
pub const GLOBAL_SHORTCUTS: &[(&str, &[&str])] = &[("Add task", &["Ctrl", "Space"]), ("Show Fastoist", &["Win", "Alt", "S"])];

pub struct Settings {
    pub page: Page,
    token: String,
    calendars: String,
    google_id: String,
    google_secret: String,
}

impl Settings {
    pub fn new(cfg: &Config) -> Self {
        Self {
            page: Page::General,
            token: cfg.token.clone(),
            calendars: cfg.calendars.join("\n"),
            google_id: cfg.google_client_id.clone(),
            google_secret: cfg.google_client_secret.clone(),
        }
    }
}

const NAV_W: f32 = 200.0;
const RADIUS: u8 = 12;

pub fn show(app: &mut App, ctx: &egui::Context) {
    let Some(mut s) = app.settings.take() else { return };
    let screen = ctx.content_rect();
    let size = Vec2::new((screen.width() - 48.0).clamp(560.0, 860.0), (screen.height() - 48.0).clamp(380.0, 580.0));
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::unique("settings")).frame(egui::Frame::new().fill(popup()).corner_radius(RADIUS as f32).stroke(Stroke::new(1.0, line()))).show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let nav = Rect::from_min_size(rect.min, Vec2::new(NAV_W, rect.height()));
        let body = Rect::from_min_max(Pos2::new(nav.right(), rect.top()), rect.max);
        ui.painter().rect_filled(nav, CornerRadius { nw: RADIUS, sw: RADIUS, ne: 0, se: 0 }, sidebar());
        ui.painter().line_segment([nav.right_top(), nav.right_bottom()], Stroke::new(1.0, line()));

        ui.scope_builder(UiBuilder::new().max_rect(nav.shrink2(Vec2::new(10.0, 14.0))), |ui| {
            ui.label(RichText::new("Settings").font(bold_font(17.0)));
            ui.add_space(10.0);
            for p in Page::ALL {
                if widgets::nav_row(ui, p.icon(), muted(), p.name(), 0, s.page == p, 0.0).clicked() {
                    s.page = p;
                }
            }
        });

        let close_r = Rect::from_min_size(Pos2::new(body.right() - 40.0, body.top() + 12.0), Vec2::splat(28.0));
        ui.scope_builder(UiBuilder::new().max_rect(close_r), |ui| {
            if widgets::icon_button(ui, Icon::Close, "Close (Esc)").clicked() {
                close = true;
            }
        });

        let inner = body.shrink2(Vec2::new(28.0, 0.0));
        ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_max(Pos2::new(inner.left(), inner.top() + 16.0), inner.max)), |ui| {
            ui.label(RichText::new(s.page.name()).font(bold_font(20.0)));
            ui.add_space(14.0);
            egui::ScrollArea::vertical().id_salt(("settings_page", s.page as u8)).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 14.0);
                match s.page {
                    Page::General => general(app, &mut s, ui),
                    Page::Appearance => appearance(app, ui),
                    Page::Calendars => calendars(app, &mut s, ui),
                    Page::Tray => tray(app, ui),
                    Page::Shortcuts => shortcuts(ui),
                    Page::About => about(ui),
                }
                ui.add_space(20.0);
            });
        });
    });
    if !(close || modal.should_close()) {
        app.settings = Some(s);
    }
}

fn heading(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).font(bold_font(13.0)).color(muted()));
    ui.add_space(4.0);
}

/// One line of a page: a label on the left and its control on the right.
fn row(ui: &mut Ui, label: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(34.0);
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
    let r = ui.min_rect();
    let y = ui.cursor().top() - ui.spacing().item_spacing.y / 2.0;
    ui.painter().line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], Stroke::new(1.0, line()));
}

fn secondary(ui: &mut Ui, label: &str) -> bool {
    ui.add(egui::Button::new(label).fill(raised())).clicked()
}

fn primary(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(accent())).clicked()
}

/// A switch that looks like the Windows toggle.
fn toggle(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let (r, mut resp) = ui.allocate_exact_size(Vec2::new(40.0, 20.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let p = ui.painter();
    let fill = if *on { accent() } else { raised() };
    p.rect_filled(r, 10.0, fill);
    if !*on {
        p.rect_stroke(r, 10.0, Stroke::new(1.0, faint()), StrokeKind::Inside);
    }
    let x = egui::lerp(r.left() + 10.0..=r.right() - 10.0, t);
    p.circle_filled(Pos2::new(x, r.center().y), 6.0, if *on { Color32::WHITE } else { muted() });
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Switch, true, *on, ""));
    resp
}

fn toggle_row(app: &mut App, ui: &mut Ui, label: &str, get: impl Fn(&mut Config) -> &mut bool) {
    row(ui, label, |ui| {
        if toggle(ui, get(&mut app.cfg)).changed() {
            app.save_config();
        }
    });
}

fn general(app: &mut App, s: &mut Settings, ui: &mut Ui) {
    heading(ui, "Account");
    let user = &app.store.user;
    if !user.full_name.is_empty() || !user.email.is_empty() {
        ui.label(RichText::new(&user.full_name).font(bold_font(14.0)));
        if !user.email.is_empty() {
            ui.label(RichText::new(&user.email).color(muted()));
        }
        ui.add_space(8.0);
    }
    ui.horizontal(|ui| {
        ui.label("API token");
        ui.hyperlink_to(RichText::new("Get token").size(12.0).color(accent_text()), TOKEN_URL);
    });
    let changed = s.token.trim() != app.cfg.token && !s.token.trim().is_empty();
    ui.horizontal(|ui| {
        let w = ui.available_width() - 70.0;
        ui.add(egui::TextEdit::singleline(&mut s.token).password(true).desired_width(w));
        if primary(ui, "Save", changed) {
            app.set_token(s.token.clone());
        }
    });
    ui.add_space(14.0);
    heading(ui, "Behaviour");
    toggle_row(app, ui, "Play a sound when I complete a task", |c| &mut c.sound);
    ui.add_space(14.0);
    heading(ui, "Data");
    row(ui, "Download everything from Todoist again", |ui| {
        if secondary(ui, "Full resync") {
            app.send(crate::api::Job::FullSync);
        }
    });
}

fn appearance(app: &mut App, ui: &mut Ui) {
    let mut pick: Option<(Mode, Accent)> = None;
    heading(ui, "Theme");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0);
        for m in Mode::ALL {
            if theme_card(ui, Palette::new(m, app.cfg.accent), m.name(), app.cfg.mode == m).clicked() {
                pick = Some((m, app.cfg.accent));
            }
        }
    });
    ui.add_space(18.0);
    heading(ui, "Accent colour");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
        for a in Accent::ALL {
            if swatch(ui, a, app.cfg.accent == a).clicked() {
                pick = Some((app.cfg.mode, a));
            }
        }
    });
    if let Some((m, a)) = pick {
        app.cfg.mode = m;
        app.cfg.accent = a;
        theme::set(ui.ctx(), m, a);
        app.save_config();
    }
}

/// A small drawing of the main window in the given palette.
fn theme_card(ui: &mut Ui, p: Palette, name: &str, on: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(150.0, 118.0), Sense::click());
    let painter = ui.painter();
    let shot = Rect::from_min_size(r.min, Vec2::new(r.width(), 90.0));
    painter.rect_filled(shot, 8.0, p.bg);
    let side = Rect::from_min_size(shot.min, Vec2::new(46.0, shot.height()));
    painter.rect_filled(side, CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 }, p.sidebar);
    let bar = |y: f32, x: f32, w: f32, c: Color32| painter.rect_filled(Rect::from_min_size(Pos2::new(x, shot.top() + y), Vec2::new(w, 5.0)), 2.5, c);
    painter.rect_filled(Rect::from_min_size(Pos2::new(side.left() + 5.0, shot.top() + 22.0), Vec2::new(36.0, 11.0)), 3.0, p.selected);
    bar(12.0, side.left() + 8.0, 24.0, p.faint);
    bar(25.0, side.left() + 8.0, 22.0, p.accent_text);
    bar(40.0, side.left() + 8.0, 26.0, p.faint);
    bar(52.0, side.left() + 8.0, 20.0, p.faint);
    let x = side.right() + 12.0;
    painter.text(Pos2::new(x, shot.top() + 14.0), Align2::LEFT_CENTER, "Today", bold_font(11.0), p.text);
    for (i, w) in [62.0, 48.0, 56.0].into_iter().enumerate() {
        let y = shot.top() + 32.0 + i as f32 * 16.0;
        painter.circle_stroke(Pos2::new(x + 4.0, y + 2.5), 4.0, Stroke::new(1.0, if i == 0 { p.accent_text } else { p.faint }));
        bar(y - shot.top(), x + 14.0, w, p.muted);
        painter.line_segment([Pos2::new(x, y + 10.0), Pos2::new(shot.right() - 8.0, y + 10.0)], Stroke::new(1.0, p.line));
    }
    let ring = if on { Stroke::new(2.0, accent()) } else if resp.hovered() { Stroke::new(1.0, muted()) } else { Stroke::new(1.0, line()) };
    painter.rect_stroke(shot, 8.0, ring, StrokeKind::Outside);
    let label_c = Pos2::new(r.left() + 2.0, shot.bottom() + 16.0);
    if on {
        theme::icon(painter, Rect::from_center_size(Pos2::new(r.left() + 8.0, label_c.y), Vec2::splat(14.0)), Icon::Check, accent_text());
    }
    let tx = if on { label_c.x + 18.0 } else { label_c.x };
    painter.text(Pos2::new(tx, label_c.y), Align2::LEFT_CENTER, name, if on { bold_font(13.0) } else { font(13.0) }, text());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, name));
    resp
}

fn swatch(ui: &mut Ui, a: Accent, on: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    let (fill, _) = a.colors();
    let p = ui.painter();
    if on || resp.hovered() {
        p.circle_stroke(r.center(), 15.0, Stroke::new(2.0, if on { fill } else { faint() }));
    }
    p.circle_filled(r.center(), 11.5, fill);
    if on {
        theme::icon(p, Rect::from_center_size(r.center(), Vec2::splat(18.0)), Icon::Check, Color32::WHITE);
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, a.name()));
    resp.on_hover_text(a.name())
}

fn calendars(app: &mut App, s: &mut Settings, ui: &mut Ui) {
    google(app, s, ui);
    ui.add_space(18.0);
    heading(ui, "iCal feeds");
    ui.add(
        egui::TextEdit::multiline(&mut s.calendars)
            .hint_text("https://calendar.google.com/calendar/ical/…/basic.ics")
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);
    let urls: Vec<String> = s.calendars.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        if primary(ui, "Save", urls != app.cfg.calendars) {
            app.set_calendars(urls);
        }
    });
}

fn google(app: &mut App, s: &mut Settings, ui: &mut Ui) {
    ui.horizontal(|ui| {
        heading(ui, "Google Calendar");
        if !app.google_signed_in() {
            ui.hyperlink_to(RichText::new("Create client ID").size(12.0).color(accent_text()), "https://console.cloud.google.com/apis/credentials");
        }
    });
    if app.google_signed_in() {
        let account = app.store.gcals.iter().find(|c| c.primary).map(|c| c.id.clone()).unwrap_or_else(|| "Signed in".into());
        row(ui, &account, |ui| {
            if secondary(ui, "Sign out") {
                app.google_sign_out();
            }
        });
        let cals = app.store.gcals.clone();
        for c in &cals {
            ui.horizontal(|ui| {
                ui.set_min_height(34.0);
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 5.0, Color32::from_rgb(c.color[0], c.color[1], c.color[2]));
                ui.label(&c.name);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut shown = !app.cfg.google_hidden.contains(&c.id);
                    if toggle(ui, &mut shown).changed() {
                        app.google_toggle_calendar(&c.id);
                    }
                });
            });
            let r = ui.min_rect();
            let y = ui.cursor().top() - ui.spacing().item_spacing.y / 2.0;
            ui.painter().line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], Stroke::new(1.0, line()));
        }
    } else {
        ui.add(egui::TextEdit::singleline(&mut s.google_id).hint_text("Client ID").desired_width(f32::INFINITY));
        ui.add(egui::TextEdit::singleline(&mut s.google_secret).password(true).hint_text("Client secret").desired_width(f32::INFINITY));
        ui.add_space(4.0);
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            let label = if app.google_waiting { "Waiting for browser…" } else { "Sign in with Google" };
            let ready = !s.google_id.trim().is_empty() && !app.google_waiting;
            if primary(ui, label, ready) {
                app.google_sign_in(s.google_id.clone(), s.google_secret.clone());
            }
        });
    }
}

fn tray(app: &mut App, ui: &mut Ui) {
    heading(ui, "Background");
    toggle_row(app, ui, "Keep running in the tray when I close the window", |c| &mut c.stay_in_tray);
    ui.add_space(14.0);
    heading(ui, "Global shortcuts");
    keys_table(ui, GLOBAL_SHORTCUTS);
}

fn shortcuts(ui: &mut Ui) {
    heading(ui, "Main window");
    keys_table(ui, SHORTCUTS);
    ui.add_space(14.0);
    heading(ui, "Anywhere in Windows");
    keys_table(ui, GLOBAL_SHORTCUTS);
}

fn keys_table(ui: &mut Ui, keys: &[(&str, &[&str])]) {
    for (what, combo) in keys {
        row(ui, what, |ui| {
            // Right to left, so the last key comes first.
            for k in combo.iter().rev() {
                if *k == "or" {
                    ui.label(RichText::new("or").color(faint()));
                } else {
                    keycap(ui, k);
                }
            }
        });
    }
}

fn keycap(ui: &mut Ui, k: &str) {
    let g = ui.painter().layout_no_wrap(k.to_string(), font(12.0), text());
    let size = Vec2::new((g.size().x + 14.0).max(24.0), 22.0);
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 5.0, raised());
    p.line_segment([r.left_bottom() + Vec2::new(3.0, 0.0), r.right_bottom() - Vec2::new(3.0, 0.0)], Stroke::new(1.5, raised_hover()));
    p.galley(r.center() - g.size() / 2.0, g, text());
}

fn about(ui: &mut Ui) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
        app_icon(ui.painter(), r);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new("Fastoist").font(bold_font(22.0)));
            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(muted()));
        });
    });
    ui.add_space(14.0);
    ui.label("A fast, lightweight Todoist client for Windows. One small program, written in Rust, that opens instantly and keeps your tasks, calendar, and quick add close at hand in the tray.");
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.hyperlink_to("Source code", REPO);
        ui.label(RichText::new("·").color(faint()));
        ui.hyperlink_to("Report a problem", format!("{REPO}/issues"));
        ui.label(RichText::new("·").color(faint()));
        ui.hyperlink_to("Releases", format!("{REPO}/releases"));
    });
    ui.add_space(18.0);
    heading(ui, "Credits");
    row(ui, "Made by", |ui| {
        ui.label(RichText::new("kycloudy, with Claude").color(muted()));
    });
    row(ui, "Tasks from", |ui| {
        ui.hyperlink_to("Todoist API", "https://developer.todoist.com/api/v1/");
    });
    row(ui, "Built with", |ui| {
        ui.label(RichText::new("Rust, egui, chrono, serde, ureq, tray-icon").color(muted()));
    });
    ui.add_space(18.0);
    heading(ui, "Your data");
    let dir = data_dir();
    row(ui, &dir.display().to_string(), |ui| {
        if secondary(ui, "Open folder") {
            open_folder(&dir);
        }
    });
    ui.add_space(18.0);
    ui.label(RichText::new("Fastoist is not made by or affiliated with Doist. Todoist is a trademark of Doist.").size(12.0).color(faint()));
}

/// The app icon: an accent rounded square with a white check.
fn app_icon(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, 14.0, accent());
    let at = |x: f32, y: f32| Pos2::new(r.left() + r.width() * x, r.top() + r.height() * y);
    p.add(egui::epaint::PathShape::line(vec![at(0.29, 0.52), at(0.44, 0.67), at(0.73, 0.35)], Stroke::new(r.width() * 0.09, Color32::WHITE)));
}

fn open_folder(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(not(windows))]
    let cmd = "xdg-open";
    let _ = std::process::Command::new(cmd).arg(dir).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_has_a_unique_name() {
        let mut names: Vec<_> = Page::ALL.iter().map(|p| p.name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), Page::ALL.len());
    }

    #[test]
    fn themes_keep_text_readable() {
        // Rough contrast check: text and background must differ a lot in brightness.
        let lum = |c: Color32| {
            let [r, g, b, _] = c.to_array();
            0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32
        };
        for m in Mode::ALL {
            for a in Accent::ALL {
                let p = Palette::new(m, a);
                assert!((lum(p.text) - lum(p.bg)).abs() > 150.0, "{m:?} {a:?}");
                assert!((lum(p.muted) - lum(p.bg)).abs() > 70.0, "{m:?} {a:?}");
                assert_ne!(p.selected, p.bg);
            }
        }
    }

    #[test]
    fn old_config_without_theme_loads_as_dark_red() {
        let cfg: Config = serde_json::from_str(r#"{"token":"x","sound":false}"#).unwrap();
        assert_eq!((cfg.mode, cfg.accent), (Mode::Dark, Accent::Red));
        assert_eq!(cfg.token, "x");
    }
}
