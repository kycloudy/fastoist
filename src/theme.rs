//! Colors, fonts, and small painted icons that match the Todoist dark theme.

use eframe::egui::{
    self, epaint::PathShape, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2, Rect,
    Stroke, StrokeKind, Vec2,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// The light or dark base of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Dark,
    Midnight,
    Light,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Dark, Mode::Midnight, Mode::Light];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Dark => "Dark",
            Mode::Midnight => "Midnight",
            Mode::Light => "Light",
        }
    }
}

/// The color of buttons, links, and the selected row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Accent {
    #[default]
    Red,
    Orange,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
}

impl Accent {
    pub const ALL: [Accent; 7] = [Accent::Red, Accent::Orange, Accent::Green, Accent::Teal, Accent::Blue, Accent::Purple, Accent::Pink];

    pub fn name(self) -> &'static str {
        match self {
            Accent::Red => "Red",
            Accent::Orange => "Orange",
            Accent::Green => "Green",
            Accent::Teal => "Teal",
            Accent::Blue => "Blue",
            Accent::Purple => "Purple",
            Accent::Pink => "Pink",
        }
    }

    /// The solid color, and a lighter one for text on dark backgrounds.
    pub const fn colors(self) -> (Color32, Color32) {
        let (a, t) = match self {
            Accent::Red => (0xdc4c3e, 0xff7066),
            Accent::Orange => (0xe8780c, 0xffa04d),
            Accent::Green => (0x2e9a48, 0x5fd17c),
            Accent::Teal => (0x158fad, 0x4cc3e0),
            Accent::Blue => (0x3c6fe8, 0x7aa2ff),
            Accent::Purple => (0x8a4fe0, 0xb48cff),
            Accent::Pink => (0xd1478c, 0xff85bf),
        };
        (hex(a), hex(t))
    }
}

const fn hex(h: u32) -> Color32 {
    Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

/// Every color the app paints with. `set` swaps it when the theme changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub mode: Mode,
    pub bg: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub popup: Color32,
    pub hover: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub selected: Color32,
    /// Text fields and other sunken areas.
    pub sunken: Color32,
    /// Toasts, badges, and controls that sit on a popup.
    pub raised: Color32,
    pub raised_hover: Color32,
}

impl Palette {
    pub const fn new(mode: Mode, accent: Accent) -> Self {
        let (a, t) = accent.colors();
        let [ar, ag, ab, _] = a.to_array();
        match mode {
            Mode::Dark => Self {
                mode,
                bg: hex(0x1e1e1e),
                sidebar: hex(0x242424),
                surface: hex(0x282828),
                popup: hex(0x2a2a2a),
                hover: hex(0x303030),
                line: hex(0x333333),
                text: hex(0xe6e6e6),
                muted: hex(0x9a9a9a),
                faint: hex(0x6b6b6b),
                accent: a,
                accent_text: t,
                selected: mix(hex(0x1e1e1e), ar, ag, ab, 0.28),
                sunken: hex(0x1a1a1a),
                raised: hex(0x333333),
                raised_hover: hex(0x444444),
            },
            Mode::Midnight => Self {
                mode,
                bg: hex(0x0d0d0d),
                sidebar: hex(0x141414),
                surface: hex(0x181818),
                popup: hex(0x1a1a1a),
                hover: hex(0x222222),
                line: hex(0x262626),
                text: hex(0xe6e6e6),
                muted: hex(0x8f8f8f),
                faint: hex(0x5e5e5e),
                accent: a,
                accent_text: t,
                selected: mix(hex(0x0d0d0d), ar, ag, ab, 0.28),
                sunken: hex(0x080808),
                raised: hex(0x262626),
                raised_hover: hex(0x343434),
            },
            Mode::Light => Self {
                mode,
                bg: hex(0xffffff),
                sidebar: hex(0xfcfaf8),
                surface: hex(0xf5f5f5),
                popup: hex(0xffffff),
                hover: hex(0xeeeeee),
                line: hex(0xe6e6e6),
                text: hex(0x202020),
                muted: hex(0x666666),
                faint: hex(0x999999),
                accent: a,
                // Text needs the darker shade to read on white.
                accent_text: a,
                selected: mix(hex(0xffffff), ar, ag, ab, 0.14),
                sunken: hex(0xf7f7f7),
                raised: hex(0xebebeb),
                raised_hover: hex(0xdddddd),
            },
        }
    }

    pub fn dark(&self) -> bool {
        self.mode != Mode::Light
    }
}

/// Blend `base` toward (r, g, b) by `t`.
const fn mix(base: Color32, r: u8, g: u8, b: u8, t: f32) -> Color32 {
    let [br, bg, bb, _] = base.to_array();
    Color32::from_rgb(lerp(br, r, t), lerp(bg, g, t), lerp(bb, b, t))
}

const fn lerp(x: u8, y: u8, t: f32) -> u8 {
    (x as f32 + (y as f32 - x as f32) * t) as u8
}

static PALETTE: RwLock<Palette> = RwLock::new(Palette::new(Mode::Dark, Accent::Red));

pub fn palette() -> Palette {
    *PALETTE.read().unwrap_or_else(|e| e.into_inner())
}

/// Switch the theme for the whole app, including egui's own widgets.
pub fn set(ctx: &egui::Context, mode: Mode, accent: Accent) {
    let p = Palette::new(mode, accent);
    *PALETTE.write().unwrap_or_else(|e| e.into_inner()) = p;
    // Fill both slots so a change in the Windows light/dark setting cannot swap in egui's defaults.
    let v = visuals(&p);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    let sys = if p.dark() { egui::SystemTheme::Dark } else { egui::SystemTheme::Light };
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(sys));
    ctx.request_repaint();
}

pub fn bg() -> Color32 { palette().bg }
pub fn sidebar() -> Color32 { palette().sidebar }
pub fn surface() -> Color32 { palette().surface }
pub fn popup() -> Color32 { palette().popup }
pub fn hover() -> Color32 { palette().hover }
pub fn line() -> Color32 { palette().line }
pub fn text() -> Color32 { palette().text }
pub fn muted() -> Color32 { palette().muted }
pub fn faint() -> Color32 { palette().faint }
pub fn accent() -> Color32 { palette().accent }
pub fn accent_text() -> Color32 { palette().accent_text }
pub fn selected() -> Color32 { palette().selected }
pub fn sunken() -> Color32 { palette().sunken }
pub fn raised() -> Color32 { palette().raised }
pub fn raised_hover() -> Color32 { palette().raised_hover }

pub const GREEN: Color32 = Color32::from_rgb(0x25, 0xb8, 0x4c);
pub const ORANGE: Color32 = Color32::from_rgb(0xff, 0x9a, 0x14);
pub const PURPLE: Color32 = Color32::from_rgb(0xa9, 0x70, 0xff);
pub const RED: Color32 = Color32::from_rgb(0xff, 0x70, 0x66);
pub const BLUE_SOFT: Color32 = Color32::from_rgb(0x52, 0x97, 0xff);
pub const EVENT: Color32 = Color32::from_rgb(0xf5, 0xcf, 0x3d);

/// Todoist priority 4 is "p1" (highest).
pub fn priority_color(p: u8) -> Color32 {
    match p {
        4 => Color32::from_rgb(0xff, 0x70, 0x66),
        3 => Color32::from_rgb(0xff, 0x9a, 0x14),
        2 => Color32::from_rgb(0x52, 0x97, 0xff),
        _ => Color32::from_rgb(0x80, 0x80, 0x80),
    }
}

/// Background for a calendar chip of the given priority.
pub fn chip_fill(p: u8) -> Color32 {
    let pal = palette();
    if p <= 1 {
        return pal.raised;
    }
    let [r, g, b, _] = priority_color(p).to_array();
    mix(pal.bg, r, g, b, if pal.dark() { 0.3 } else { 0.2 })
}

pub fn named_color(name: &str) -> Color32 {
    let hex = match name {
        "berry_red" => 0xb8255f,
        "red" => 0xdb4035,
        "orange" => 0xff9933,
        "yellow" => 0xfad000,
        "olive_green" => 0xafb83b,
        "lime_green" => 0x7ecc49,
        "green" => 0x299438,
        "mint_green" => 0x6accbc,
        "teal" | "turquoise" => 0x158fad,
        "sky_blue" => 0x14aaf5,
        "light_blue" => 0x96c3eb,
        "blue" => 0x4073ff,
        "grape" => 0x884dff,
        "violet" => 0xaf38eb,
        "lavender" => 0xeb96eb,
        "magenta" => 0xe05194,
        "salmon" => 0xff8d85,
        "grey" | "gray" => 0xb8b8b8,
        "taupe" => 0xccac93,
        _ => 0x808080,
    };
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, bold())
}

/// Use Segoe UI on Windows so text looks native; fall back to egui's fonts elsewhere.
pub fn install(ctx: &egui::Context, mode: Mode, accent: Accent) {
    let mut fonts = FontDefinitions::default();
    let dir = std::env::var("WINDIR").map(|w| format!("{w}\\Fonts")).unwrap_or_default();
    let load = |file: &str| std::fs::read(format!("{dir}\\{file}")).ok();
    let mut regular = Vec::new();
    let mut semibold = Vec::new();
    if let Some(b) = load("segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), Arc::new(FontData::from_owned(b)));
        regular.push("segoe".to_string());
    }
    if let Some(b) = load("seguisb.ttf") {
        fonts.font_data.insert("segoe-sb".into(), Arc::new(FontData::from_owned(b)));
        semibold.push("segoe-sb".to_string());
    }
    if let Some(b) = load("seguisym.ttf") {
        fonts.font_data.insert("segoe-sym".into(), Arc::new(FontData::from_owned(b)));
    }
    let defaults = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop = regular.clone();
    prop.extend(defaults.iter().cloned());
    if fonts.font_data.contains_key("segoe-sym") {
        prop.push("segoe-sym".into());
    }
    let mut b = semibold;
    b.extend(prop.iter().cloned());
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(bold(), b);
    ctx.set_fonts(fonts);

    set(ctx, mode, accent);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
        s.spacing.interact_size.y = 28.0;
        s.text_styles.insert(egui::TextStyle::Body, font(14.0));
        s.text_styles.insert(egui::TextStyle::Button, font(14.0));
        s.text_styles.insert(egui::TextStyle::Small, font(12.0));
        s.text_styles.insert(egui::TextStyle::Heading, bold_font(26.0));
    });
    // egui's default wheel step (40 px per notch) feels slow next to native Windows apps.
    ctx.options_mut(|o| o.input_options.line_scroll_speed = 110.0);
}

fn visuals(p: &Palette) -> egui::Visuals {
    let mut v = if p.dark() { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.popup;
    v.extreme_bg_color = p.sunken;
    v.faint_bg_color = p.surface;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent.gamma_multiply(0.6);
    v.selection.stroke = Stroke::new(1.0, p.accent_text);
    v.hyperlink_color = p.accent_text;
    for w in [&mut v.widgets.inactive, &mut v.widgets.noninteractive] {
        w.bg_stroke = Stroke::NONE;
        w.corner_radius = CornerRadius::same(6);
    }
    v.widgets.inactive.weak_bg_fill = p.surface;
    v.widgets.inactive.bg_fill = p.surface;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.hovered.corner_radius = CornerRadius::same(6);
    v.widgets.active.weak_bg_fill = p.raised;
    v.widgets.active.corner_radius = CornerRadius::same(6);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    v
}

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Search,
    Inbox,
    Today(u32),
    Upcoming,
    Filters,
    Hash,
    Tag,
    Drop,
    Calendar,
    Deadline,
    Repeat,
    Plus,
    Close,
    Dot,
    Sun,
    Sofa,
    Slash,
    Section,
    Flag,
    Clock,
    Sidebar,
    Left,
    Right,
    Down,
    Gear,
    Sync,
    Check,
    Panel,
    Info,
    Keyboard,
    Palette,
    Tray,
    Alarm,
    Send,
}

/// Paint a 16px line icon centered in `r`.
pub fn icon(p: &Painter, r: Rect, icon: Icon, color: Color32) {
    let c = r.center();
    let s = Stroke::new(1.3, color);
    let sz = r.width().min(r.height()).min(18.0);
    let k = sz / 16.0;
    let at = |x: f32, y: f32| Pos2::new(c.x + x * k, c.y + y * k);
    let rr = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
    match icon {
        Icon::Search => {
            p.circle_stroke(at(-1.5, -1.5), 5.0 * k, s);
            p.line_segment([at(2.3, 2.3), at(6.0, 6.0)], s);
        }
        Icon::Inbox => {
            p.add(PathShape::line(
                vec![at(-6.5, 0.5), at(-4.0, -5.5), at(4.0, -5.5), at(6.5, 0.5), at(6.5, 5.5), at(-6.5, 5.5), at(-6.5, 0.5)],
                s,
            ));
            p.add(PathShape::line(vec![at(-6.5, 0.5), at(-2.5, 0.5), at(-1.5, 2.5), at(1.5, 2.5), at(2.5, 0.5), at(6.5, 0.5)], s));
        }
        Icon::Today(day) => {
            p.rect_stroke(rr(-6.5, -6.0, 6.5, 6.5), 2.0, s, StrokeKind::Middle);
            p.line_segment([at(-6.5, -3.0), at(6.5, -3.0)], s);
            p.text(at(0.0, 1.9), egui::Align2::CENTER_CENTER, day.to_string(), bold_font(8.0 * k), color);
        }
        Icon::Upcoming | Icon::Calendar => {
            p.rect_stroke(rr(-6.5, -6.0, 6.5, 6.5), 2.0, s, StrokeKind::Middle);
            p.line_segment([at(-6.5, -3.0), at(6.5, -3.0)], s);
            for (x, y) in [(-3.0, 0.0), (0.0, 0.0), (3.0, 0.0), (-3.0, 3.0), (0.0, 3.0)] {
                p.circle_filled(at(x, y), 0.8 * k, color);
            }
        }
        Icon::Filters => {
            for (x, y) in [(-6.0, -6.0), (1.0, -6.0), (-6.0, 1.0), (1.0, 1.0)] {
                p.rect_stroke(rr(x, y, x + 5.0, y + 5.0), 1.5, s, StrokeKind::Middle);
            }
        }
        Icon::Hash => {
            p.text(c, egui::Align2::CENTER_CENTER, "#", font(16.0 * k), color);
        }
        Icon::Tag => {
            p.add(PathShape::closed_line(vec![at(-6.0, -6.0), at(0.0, -6.0), at(6.0, 0.0), at(0.0, 6.0), at(-6.0, 0.0)], s));
            p.circle_filled(at(-3.0, -3.0), 1.0 * k, color);
        }
        Icon::Drop => {
            p.add(PathShape::closed_line(
                vec![at(0.0, -7.0), at(4.0, -1.0), at(4.5, 2.5), at(2.5, 5.5), at(0.0, 6.5), at(-2.5, 5.5), at(-4.5, 2.5), at(-4.0, -1.0)],
                s,
            ));
        }
        Icon::Deadline => {
            p.circle_stroke(c, 6.0 * k, s);
            p.circle_stroke(c, 3.0 * k, s);
            p.circle_filled(c, 1.0 * k, color);
        }
        Icon::Repeat => {
            p.add(PathShape::line(
                (0..=10).map(|i| {
                    let a = 0.6 + i as f32 * 0.5;
                    at(5.5 * a.cos(), 5.5 * a.sin())
                }).collect(),
                s,
            ));
            p.add(PathShape::line(vec![at(2.5, -6.5), at(5.6, -2.6), at(6.8, -6.0)], s));
        }
        Icon::Dot => {
            p.circle_stroke(c, 3.0 * k, s);
        }
        Icon::Sun => {
            p.circle_stroke(c, 3.2 * k, s);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                p.line_segment([at(5.0 * a.cos(), 5.0 * a.sin()), at(7.0 * a.cos(), 7.0 * a.sin())], s);
            }
        }
        Icon::Sofa => {
            p.add(PathShape::line(vec![at(-7.0, 4.5), at(-7.0, -1.0), at(-4.5, -1.0), at(-4.5, 1.5), at(4.5, 1.5), at(4.5, -1.0), at(7.0, -1.0), at(7.0, 4.5), at(-7.0, 4.5)], s));
            p.add(PathShape::line(vec![at(-4.5, -1.0), at(-4.5, -4.5), at(4.5, -4.5), at(4.5, -1.0)], s));
        }
        Icon::Slash => {
            p.circle_stroke(c, 6.0 * k, s);
            p.line_segment([at(-4.2, 4.2), at(4.2, -4.2)], s);
        }
        Icon::Section => {
            p.rect_stroke(rr(-6.0, -5.0, 6.0, 5.0), 1.5, s, StrokeKind::Middle);
            p.line_segment([at(-6.0, -1.5), at(6.0, -1.5)], s);
        }
        Icon::Flag => {
            p.add(PathShape::convex_polygon(vec![at(-4.5, -6.5), at(5.5, -6.5), at(3.0, -3.0), at(5.5, 0.5), at(-4.5, 0.5)], color, Stroke::NONE));
            p.line_segment([at(-4.5, -6.5), at(-4.5, 7.0)], Stroke::new(1.6, color));
        }
        Icon::Clock => {
            p.circle_stroke(c, 6.0 * k, s);
            p.add(PathShape::line(vec![at(0.0, -3.5), at(0.0, 0.0), at(2.5, 1.5)], s));
        }
        Icon::Alarm => {
            p.circle_stroke(at(0.0, 0.8), 5.5 * k, s);
            p.add(PathShape::line(vec![at(0.0, -2.0), at(0.0, 0.8), at(2.0, 2.0)], s));
            p.line_segment([at(-6.5, -4.0), at(-4.0, -6.5)], s);
            p.line_segment([at(6.5, -4.0), at(4.0, -6.5)], s);
            p.line_segment([at(-3.5, 5.5), at(-5.0, 7.0)], s);
            p.line_segment([at(3.5, 5.5), at(5.0, 7.0)], s);
        }
        Icon::Send => {
            let s = Stroke::new(1.8, color);
            p.line_segment([at(0.0, 6.0), at(0.0, -6.0)], s);
            p.add(PathShape::line(vec![at(-5.0, -1.0), at(0.0, -6.0), at(5.0, -1.0)], s));
        }
        Icon::Close => {
            p.line_segment([at(-4.5, -4.5), at(4.5, 4.5)], s);
            p.line_segment([at(-4.5, 4.5), at(4.5, -4.5)], s);
        }
        Icon::Plus => {
            p.line_segment([at(-6.0, 0.0), at(6.0, 0.0)], s);
            p.line_segment([at(0.0, -6.0), at(0.0, 6.0)], s);
        }
        Icon::Sidebar | Icon::Panel => {
            p.rect_stroke(rr(-7.0, -6.0, 7.0, 6.0), 2.0, s, StrokeKind::Middle);
            let x = if icon == Icon::Sidebar { -2.0 } else { 2.0 };
            p.line_segment([at(x, -6.0), at(x, 6.0)], s);
        }
        Icon::Left => {
            p.add(PathShape::line(vec![at(2.5, -5.0), at(-2.5, 0.0), at(2.5, 5.0)], s));
        }
        Icon::Right => {
            p.add(PathShape::line(vec![at(-2.5, -5.0), at(2.5, 0.0), at(-2.5, 5.0)], s));
        }
        Icon::Down => {
            p.add(PathShape::line(vec![at(-4.0, -2.0), at(0.0, 2.0), at(4.0, -2.0)], s));
        }
        Icon::Gear => {
            p.circle_stroke(c, 2.5 * k, s);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                p.line_segment([at(4.5 * a.cos(), 4.5 * a.sin()), at(6.8 * a.cos(), 6.8 * a.sin())], Stroke::new(2.0, color));
            }
            p.circle_stroke(c, 4.8 * k, s);
        }
        Icon::Sync => {
            p.add(PathShape::line((0..=8).map(|i| { let a = 3.6 + i as f32 * 0.32; at(5.5 * a.cos(), 5.5 * a.sin()) }).collect(), s));
            p.add(PathShape::line((0..=8).map(|i| { let a = 0.46 + i as f32 * 0.32; at(5.5 * a.cos(), 5.5 * a.sin()) }).collect(), s));
            p.add(PathShape::line(vec![at(2.0, -6.5), at(4.2, -4.3), at(1.6, -2.6)], s));
            p.add(PathShape::line(vec![at(-2.0, 6.5), at(-4.2, 4.3), at(-1.6, 2.6)], s));
        }
        Icon::Check => {
            p.add(PathShape::line(vec![at(-3.5, 0.0), at(-1.0, 2.5), at(3.5, -2.5)], Stroke::new(1.6, color)));
        }
        Icon::Info => {
            p.circle_stroke(c, 6.5 * k, s);
            p.circle_filled(at(0.0, -3.2), 0.9 * k, color);
            p.line_segment([at(0.0, -0.8), at(0.0, 3.8)], Stroke::new(1.5, color));
        }
        Icon::Keyboard => {
            p.rect_stroke(rr(-7.0, -4.5, 7.0, 4.5), 1.5, s, StrokeKind::Middle);
            for x in [-4.0, -1.3, 1.3, 4.0] {
                p.circle_filled(at(x, -1.5), 0.7 * k, color);
            }
            p.line_segment([at(-3.5, 1.8), at(3.5, 1.8)], s);
        }
        Icon::Palette => {
            p.circle_stroke(c, 6.5 * k, s);
            for (x, y) in [(-3.0, -2.0), (0.0, -3.8), (3.0, -2.0)] {
                p.circle_filled(at(x, y), 1.0 * k, color);
            }
            p.circle_stroke(at(1.8, 2.6), 1.6 * k, s);
        }
        Icon::Tray => {
            p.rect_stroke(rr(-7.0, -5.5, 7.0, 5.5), 1.5, s, StrokeKind::Middle);
            p.line_segment([at(-7.0, 2.5), at(7.0, 2.5)], s);
            p.circle_filled(at(4.2, 4.0), 0.9 * k, color);
            p.circle_filled(at(1.6, 4.0), 0.9 * k, color);
        }
    }
}

/// The round completion button. `fill` adds the light tint Todoist uses for priorities.
pub fn check_circle(p: &Painter, center: Pos2, radius: f32, priority: u8, hovered: bool, done: bool) {
    let col = priority_color(priority);
    if done {
        p.circle_filled(center, radius, col.gamma_multiply(0.8));
        icon(p, Rect::from_center_size(center, Vec2::splat(radius * 2.0)), Icon::Check, bg());
        return;
    }
    if priority > 1 {
        p.circle_filled(center, radius, col.gamma_multiply(0.12));
    }
    p.circle_stroke(center, radius, Stroke::new(if priority > 1 { 1.6 } else { 1.0 }, col));
    if hovered {
        icon(p, Rect::from_center_size(center, Vec2::splat(radius * 2.0)), Icon::Check, col);
    }
}
