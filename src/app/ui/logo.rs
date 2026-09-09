//! Application logo — Document + Bolt (Option 4).
//!
//! Provides the taskbar/window icon (`IconData`) and the in-app header paint routine.
//! Colors are shared with `assets/icon.svg` and the dark theme (`9,9,11` bg, `99,102,241` indigo).

use eframe::egui;

/// Try to load the 32px PNG icon for `ViewportBuilder::with_icon`.
///
/// Falls back to `None` (eframe default) if decoding fails — never panics in production.
pub fn load_app_icon() -> Option<egui::IconData> {
    // 32px is a good default for winit; Windows .ico embedding is handled separately via `build.rs`.
    // Path is relative to this file: `src/app/ui/logo.rs` -> `../../..` -> crate root.
    let bytes = include_bytes!("../../../assets/icon-32.png");
    // Decode PNG via `image` crate (egui 0.36 has no `try_from_png_bytes`).
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width: w,
        height: h,
    })
}

/// Paint the Document + Bolt mark into `rect` using the current painter.
///
/// Intended for the top toolbar (≈20×20). Falls back gracefully if `rect` is degenerate.
pub fn paint_logo(ui: &mut egui::Ui, rect: egui::Rect) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let painter = ui.painter_at(rect);

    // Theme-aligned palette
    let indigo = egui::Color32::from_rgb(99, 102, 241);
    let cyan = egui::Color32::from_rgb(100, 200, 255);
    let white = egui::Color32::WHITE;
    let fold = egui::Color32::from_rgb(199, 210, 254);
    let fold_stroke = egui::Color32::from_rgb(165, 180, 252);
    let dark_stroke = egui::Color32::from_rgb(67, 56, 202);

    let rounding = rect.width() * 7.0 / 32.0;

    // Badge background
    painter.rect_filled(rect, rounding, indigo);
    // subtle inner stroke
    painter.rect_stroke(
        rect,
        rounding,
        egui::Stroke::new(0.5, egui::Color32::from_white_alpha(20)),
        egui::StrokeKind::Inside,
    );

    // Document — inset ~14% on each side for padding
    let doc_inset = rect.width() * 0.14;
    let fold_size = rect.width() * 0.14;
    let doc_left = rect.left() + doc_inset;
    let doc_right = rect.right() - doc_inset;
    let doc_top = rect.top() + doc_inset * 0.55;
    let doc_bottom = rect.bottom() - doc_inset * 0.45;
    let fold_x = doc_right - fold_size;
    let fold_y = doc_top + fold_size;

    // Document polygon: rect with top-right corner cut for fold
    let doc_points = vec![
        egui::pos2(doc_left, doc_top),
        egui::pos2(fold_x, doc_top),
        egui::pos2(doc_right, fold_y),
        egui::pos2(doc_right, doc_bottom),
        egui::pos2(doc_left, doc_bottom),
    ];
    painter.add(egui::Shape::convex_polygon(
        doc_points,
        white,
        egui::Stroke::NONE,
    ));

    // Fold triangle
    let fold_points = vec![
        egui::pos2(fold_x, doc_top),
        egui::pos2(fold_x, fold_y),
        egui::pos2(doc_right, fold_y),
    ];
    painter.add(egui::Shape::convex_polygon(
        fold_points,
        fold,
        egui::Stroke::NONE,
    ));
    painter.line_segment(
        [egui::pos2(fold_x, doc_top), egui::pos2(fold_x, fold_y)],
        egui::Stroke::new(0.5, fold_stroke),
    );
    painter.line_segment(
        [egui::pos2(fold_x, fold_y), egui::pos2(doc_right, fold_y)],
        egui::Stroke::new(0.5, fold_stroke),
    );

    // Faint markdown lines — only if rect >= 18px (avoid clutter at tiny sizes)
    if rect.width() >= 18.0 {
        let line_h = rect.width() * 0.034;
        let line_w1 = rect.width() * 0.28;
        let line_w2 = rect.width() * 0.20;
        let line_y0 = doc_top + rect.height() * 0.42;
        let line_x0 = doc_left + rect.width() * 0.08;
        let line_round = line_h * 0.5;
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(line_x0, line_y0), egui::vec2(line_w1, line_h)),
            line_round,
            fold,
        );
        if rect.width() >= 22.0 {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(line_x0, line_y0 + line_h * 2.0),
                    egui::vec2(line_w2, line_h),
                ),
                line_round,
                egui::Color32::from_rgba_premultiplied(199, 210, 254, 165),
            );
        }
    }

    // Bolt — normalized to badge rect
    // Original bolt outer: M17.9 13.0 L12.1 19.4 H14.9 L13.8 24.8 L20.0 17.2 H17.0 Z in 32-viewbox
    let vb = |x: f32, y: f32| -> egui::Pos2 {
        egui::pos2(
            rect.left() + x / 32.0 * rect.width(),
            rect.top() + y / 32.0 * rect.height(),
        )
    };
    let bolt = vec![
        vb(17.9, 13.0),
        vb(12.1, 19.4),
        vb(14.9, 19.4),
        vb(13.8, 24.8),
        vb(20.0, 17.2),
        vb(17.0, 17.2),
    ];
    // dark outline
    painter.add(egui::Shape::convex_polygon(
        bolt.clone(),
        cyan,
        egui::Stroke::new(0.6, dark_stroke),
    ));
    // white thin stroke via line loop
    let n = bolt.len();
    for i in 0..n {
        painter.line_segment([bolt[i], bolt[(i + 1) % n]], egui::Stroke::new(0.35, white));
    }
    // inner highlight
    let hl = vec![
        vb(17.4, 14.3),
        vb(13.6, 18.6),
        vb(15.2, 18.6),
        vb(14.6, 22.0),
        vb(18.3, 17.9),
        vb(16.4, 17.9),
    ];
    painter.add(egui::Shape::convex_polygon(
        hl,
        egui::Color32::from_white_alpha(58),
        egui::Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_app_icon_decodes() {
        let icon = load_app_icon();
        assert!(icon.is_some(), "icon-32.png should decode");
        let icon = icon.unwrap();
        assert_eq!(icon.width, 32);
        assert_eq!(icon.height, 32);
        assert!(!icon.rgba.is_empty());
        assert_eq!(icon.rgba.len(), 32 * 32 * 4);
    }
}
