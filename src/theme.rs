//! Colors, fonts, and small painted icons that match the Todoist dark theme.

use eframe::egui::{
    self, epaint::PathShape, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2, Rect,
    Stroke, StrokeKind, Vec2,
};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x1e);
pub const SIDEBAR: Color32 = Color32::from_rgb(0x24, 0x24, 0x24);
pub const SURFACE: Color32 = Color32::from_rgb(0x28, 0x28, 0x28);
pub const POPUP: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);
pub const HOVER: Color32 = Color32::from_rgb(0x30, 0x30, 0x30);
pub const LINE: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe6);
pub const MUTED: Color32 = Color32::from_rgb(0x9a, 0x9a, 0x9a);
pub const FAINT: Color32 = Color32::from_rgb(0x6b, 0x6b, 0x6b);
pub const ACCENT: Color32 = Color32::from_rgb(0xdc, 0x4c, 0x3e);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0xff, 0x70, 0x66);
pub const SELECTED: Color32 = Color32::from_rgb(0x47, 0x24, 0x22);
pub const GREEN: Color32 = Color32::from_rgb(0x25, 0xb8, 0x4c);
pub const ORANGE: Color32 = Color32::from_rgb(0xff, 0x9a, 0x14);
pub const PURPLE: Color32 = Color32::from_rgb(0xa9, 0x70, 0xff);
pub const RED: Color32 = Color32::from_rgb(0xff, 0x70, 0x66);
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
    match p {
        4 => Color32::from_rgb(0x5a, 0x2a, 0x27),
        3 => Color32::from_rgb(0x55, 0x3b, 0x17),
        2 => Color32::from_rgb(0x1f, 0x3a, 0x60),
        _ => Color32::from_rgb(0x33, 0x33, 0x33),
    }
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
pub fn install(ctx: &egui::Context) {
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

    ctx.set_visuals(visuals());
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
        s.spacing.interact_size.y = 28.0;
        s.text_styles.insert(egui::TextStyle::Body, font(14.0));
        s.text_styles.insert(egui::TextStyle::Button, font(14.0));
        s.text_styles.insert(egui::TextStyle::Small, font(12.0));
        s.text_styles.insert(egui::TextStyle::Heading, bold_font(26.0));
    });
}

fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = POPUP;
    v.extreme_bg_color = Color32::from_rgb(0x1a, 0x1a, 0x1a);
    v.faint_bg_color = SURFACE;
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT.gamma_multiply(0.6);
    v.selection.stroke = Stroke::new(1.0, ACCENT_TEXT);
    v.hyperlink_color = ACCENT_TEXT;
    for w in [&mut v.widgets.inactive, &mut v.widgets.noninteractive] {
        w.bg_stroke = Stroke::NONE;
        w.corner_radius = CornerRadius::same(6);
    }
    v.widgets.inactive.weak_bg_fill = SURFACE;
    v.widgets.inactive.bg_fill = SURFACE;
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.hovered.corner_radius = CornerRadius::same(6);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x38, 0x38, 0x38);
    v.widgets.active.corner_radius = CornerRadius::same(6);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
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
    Sidebar,
    Left,
    Right,
    Down,
    Gear,
    Sync,
    Check,
    Panel,
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
    }
}

/// The round completion button. `fill` adds the light tint Todoist uses for priorities.
pub fn check_circle(p: &Painter, center: Pos2, radius: f32, priority: u8, hovered: bool, done: bool) {
    let col = priority_color(priority);
    if done {
        p.circle_filled(center, radius, col.gamma_multiply(0.8));
        icon(p, Rect::from_center_size(center, Vec2::splat(radius * 2.0)), Icon::Check, BG);
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
