#![windows_subsystem = "windows"]

mod api;
mod app;
mod dialogs;
mod ics;
mod model;
mod theme;
mod tray;
mod upcoming;
mod views;
mod widgets;

fn main() -> eframe::Result {
    if !tray::claim_instance() {
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Fastoist")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([720.0, 480.0])
            .with_icon(icon()),
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native("Fastoist", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}

/// A red rounded square with a white check, drawn at startup so the exe needs no image files.
fn icon() -> eframe::egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    let check = |x: f32, y: f32| {
        let d1 = dist_seg(x, y, (18.0, 33.0), (28.0, 43.0));
        let d2 = dist_seg(x, y, (28.0, 43.0), (47.0, 22.0));
        d1.min(d2) < 3.6
    };
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let dx = (fx - 32.0).abs() - 18.0;
            let dy = (fy - 32.0).abs() - 18.0;
            let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() - 12.0;
            let i = (y * n + x) * 4;
            if outside <= 0.0 {
                let c = if check(fx, fy) { [255, 255, 255] } else { [0xdc, 0x4c, 0x3e] };
                rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }
    eframe::egui::IconData { rgba, width: n as u32, height: n as u32 }
}

fn dist_seg(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let t = (((x - a.0) * vx + (y - a.1) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
    ((x - a.0 - t * vx).powi(2) + (y - a.1 - t * vy).powi(2)).sqrt()
}
