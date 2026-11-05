//! Small icons for the tray menu, drawn in code so the menu needs no image files.
//!
//! Each glyph is a few shapes on a 16 × 16 grid. `render` rasterizes one to RGBA with
//! 4 × 4 supersampling, so edges stay smooth at the 16 px size Windows menus use.

/// A tray menu icon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Glyph {
    Add,
    Window,
    Inbox,
    Today,
    Upcoming,
    Filters,
    Completed,
    Settings,
    Quit,
    Star,
    /// A task, as Todoist's check circle in the priority color.
    Task([u8; 3]),
    /// A calendar event, as a dot in the calendar color.
    Event([u8; 3]),
    Project([u8; 3]),
    Filter([u8; 3]),
    Label([u8; 3]),
}

pub const SIZE: u32 = 16;
const SS: u32 = 4;
const FG: [u8; 3] = [0xd8, 0xd8, 0xd8];
const RED: [u8; 3] = [0xdc, 0x4c, 0x3e];

enum Shape {
    /// A line from a to b with width w and round ends.
    Line([f32; 2], [f32; 2], f32),
    /// A circle outline: center, radius, width.
    Ring([f32; 2], f32, f32),
    Disc([f32; 2], f32),
    /// A filled polygon.
    Poly(Vec<[f32; 2]>),
}

fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) };
    let (x, y) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
    (x * x + y * y).sqrt()
}

fn inside(poly: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
    }
    c
}

impl Shape {
    fn hit(&self, p: [f32; 2]) -> bool {
        match self {
            Shape::Line(a, b, w) => seg_dist(p, *a, *b) <= w / 2.0,
            Shape::Ring(c, r, w) => (((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt() - r).abs() <= w / 2.0,
            Shape::Disc(c, r) => (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) <= r * r,
            Shape::Poly(pts) => inside(pts, p),
        }
    }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, w: f32) -> Vec<Shape> {
    vec![
        Shape::Line([x0, y0], [x1, y0], w),
        Shape::Line([x1, y0], [x1, y1], w),
        Shape::Line([x1, y1], [x0, y1], w),
        Shape::Line([x0, y1], [x0, y0], w),
    ]
}

/// The shapes of a glyph, each with its color.
fn shapes(g: Glyph) -> Vec<(Shape, [u8; 3])> {
    use Shape::*;
    let w = 1.4;
    let one = |v: Vec<Shape>, c: [u8; 3]| v.into_iter().map(|s| (s, c)).collect::<Vec<_>>();
    match g {
        Glyph::Add => {
            let mut v = vec![(Disc([8.0, 8.0], 7.0), RED)];
            v.extend(one(vec![Line([8.0, 4.5], [8.0, 11.5], 1.6), Line([4.5, 8.0], [11.5, 8.0], 1.6)], [0xff, 0xff, 0xff]));
            v
        }
        Glyph::Window => {
            let mut v = rect(2.0, 3.0, 14.0, 13.0, w);
            v.push(Line([2.0, 6.0], [14.0, 6.0], w));
            one(v, FG)
        }
        Glyph::Inbox => one(
            vec![
                Line([2.0, 8.5], [4.5, 3.0], w),
                Line([4.5, 3.0], [11.5, 3.0], w),
                Line([11.5, 3.0], [14.0, 8.5], w),
                Line([14.0, 8.5], [14.0, 13.0], w),
                Line([14.0, 13.0], [2.0, 13.0], w),
                Line([2.0, 13.0], [2.0, 8.5], w),
                Line([2.0, 8.5], [5.5, 8.5], w),
                Line([5.5, 8.5], [6.5, 10.5], w),
                Line([6.5, 10.5], [9.5, 10.5], w),
                Line([9.5, 10.5], [10.5, 8.5], w),
                Line([10.5, 8.5], [14.0, 8.5], w),
            ],
            FG,
        ),
        Glyph::Today => {
            let mut v = rect(2.0, 3.0, 14.0, 13.5, w);
            v.push(Line([2.0, 6.0], [14.0, 6.0], w));
            v.push(Disc([8.0, 9.75], 1.8));
            one(v, [0x4f, 0xb3, 0x5b])
        }
        Glyph::Upcoming => {
            let mut v = rect(2.0, 3.0, 14.0, 13.5, w);
            v.push(Line([2.0, 6.0], [14.0, 6.0], w));
            for (x, y) in [(5.0, 8.75), (8.0, 8.75), (11.0, 8.75), (5.0, 11.25), (8.0, 11.25)] {
                v.push(Disc([x, y], 0.9));
            }
            one(v, [0xa9, 0x7f, 0xe0])
        }
        Glyph::Filters => {
            let mut v = rect(2.5, 2.5, 7.0, 7.0, w);
            v.extend(rect(9.0, 2.5, 13.5, 7.0, w));
            v.extend(rect(2.5, 9.0, 7.0, 13.5, w));
            v.extend(rect(9.0, 9.0, 13.5, 13.5, w));
            one(v, [0xeb, 0x8d, 0x3a])
        }
        Glyph::Completed => one(vec![Ring([8.0, 8.0], 6.0, w), Line([5.2, 8.2], [7.2, 10.2], 1.5), Line([7.2, 10.2], [10.9, 6.2], 1.5)], FG),
        Glyph::Settings => {
            let mut v = vec![Ring([8.0, 8.0], 2.2, w), Ring([8.0, 8.0], 5.0, w)];
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let (s, c) = a.sin_cos();
                v.push(Line([8.0 + 5.0 * c, 8.0 + 5.0 * s], [8.0 + 6.8 * c, 8.0 + 6.8 * s], 2.2));
            }
            one(v, FG)
        }
        Glyph::Quit => {
            // A power symbol: an open ring with a bar through the gap.
            let p = |a: f32| [8.0 + 5.5 * a.sin(), 8.5 - 5.5 * a.cos()];
            let (from, to, steps) = (0.7f32, std::f32::consts::TAU - 0.7, 20);
            let step = (to - from) / steps as f32;
            let mut v: Vec<Shape> = (0..steps).map(|i| Line(p(from + i as f32 * step), p(from + (i + 1) as f32 * step), w)).collect();
            v.push(Line([8.0, 1.8], [8.0, 7.5], w));
            one(v, FG)
        }
        Glyph::Star => {
            let pts = (0..10)
                .map(|i| {
                    let r = if i % 2 == 0 { 6.8 } else { 2.9 };
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    [8.0 + r * a.cos(), 8.6 + r * a.sin()]
                })
                .collect();
            one(vec![Poly(pts)], [0xe8, 0xb3, 0x39])
        }
        Glyph::Task(c) => one(vec![Ring([8.0, 8.0], 5.8, 1.5)], c),
        Glyph::Event(c) => one(vec![Disc([8.0, 8.0], 4.0)], c),
        Glyph::Project(c) => one(
            vec![Line([6.2, 2.5], [4.8, 13.5], w), Line([11.2, 2.5], [9.8, 13.5], w), Line([2.5, 6.0], [13.5, 6.0], w), Line([2.5, 10.0], [13.5, 10.0], w)],
            c,
        ),
        Glyph::Filter(c) => one(vec![Disc([8.0, 9.8], 4.2), Poly(vec![[8.0, 2.0], [4.1, 8.6], [11.9, 8.6]])], c),
        Glyph::Label(c) => one(
            vec![
                Line([2.5, 2.5], [8.0, 2.5], w),
                Line([8.0, 2.5], [13.5, 8.0], w),
                Line([13.5, 8.0], [8.0, 13.5], w),
                Line([8.0, 13.5], [2.5, 8.0], w),
                Line([2.5, 8.0], [2.5, 2.5], w),
                Disc([5.5, 5.5], 1.0),
            ],
            c,
        ),
    }
}

/// Draw a glyph as straight (not premultiplied) RGBA, SIZE × SIZE pixels, top row first.
pub fn render(g: Glyph) -> Vec<u8> {
    let list = shapes(g);
    let mut out = vec![0u8; (SIZE * SIZE * 4) as usize];
    let n = (SS * SS) as f32;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (mut cover, mut rgb) = (0.0f32, [0.0f32; 3]);
            for sy in 0..SS {
                for sx in 0..SS {
                    let p = [x as f32 + (sx as f32 + 0.5) / SS as f32, y as f32 + (sy as f32 + 0.5) / SS as f32];
                    // The last shape that covers a sample wins, so later shapes draw on top.
                    if let Some((_, c)) = list.iter().rev().find(|(s, _)| s.hit(p)) {
                        cover += 1.0;
                        for k in 0..3 {
                            rgb[k] += c[k] as f32;
                        }
                    }
                }
            }
            if cover > 0.0 {
                let i = ((y * SIZE + x) * 4) as usize;
                for k in 0..3 {
                    out[i + k] = (rgb[k] / cover).round() as u8;
                }
                out[i + 3] = (cover / n * 255.0).round() as u8;
            }
        }
    }
    out
}

/// Todoist's priority colors for task circles in the menu.
pub fn priority_rgb(p: u8) -> [u8; 3] {
    match p {
        4 => [0xd1, 0x45, 0x3b],
        3 => [0xeb, 0x8d, 0x13],
        2 => [0x24, 0x6f, 0xe0],
        _ => [0x9a, 0x9a, 0x9a],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_draws_something_inside_its_box() {
        let all = [
            Glyph::Add,
            Glyph::Window,
            Glyph::Inbox,
            Glyph::Today,
            Glyph::Upcoming,
            Glyph::Filters,
            Glyph::Completed,
            Glyph::Settings,
            Glyph::Quit,
            Glyph::Star,
            Glyph::Task([255, 0, 0]),
            Glyph::Event([0, 255, 0]),
            Glyph::Project([0, 0, 255]),
            Glyph::Filter([9, 9, 9]),
            Glyph::Label([9, 9, 9]),
        ];
        for g in all {
            let px = render(g);
            assert_eq!(px.len(), (SIZE * SIZE * 4) as usize);
            let lit = px.chunks(4).filter(|p| p[3] > 0).count();
            assert!(lit > 10 && lit < 256, "{g:?} lit {lit}");
        }
        // Edges are smooth: some pixels are partly covered.
        assert!(render(Glyph::Completed).chunks(4).any(|p| p[3] > 0 && p[3] < 255));
    }
}
