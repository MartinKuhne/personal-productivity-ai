use fastmd::eframe::egui::{Align, CentralPanel, Context, Grid, Layout, RawInput, epaint::Shape};

#[test]
fn test_grid_align() {
    let mut ctx = Context::default();
    let raw = RawInput::default();

    let _ = ctx.run_ui(raw.clone(), |ui| {
        CentralPanel::default().show(ui, |ui| {
            Grid::new("g").show(ui, |ui| {
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.label("Short");
                });
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.label("Tall\nTall\nTall");
                });
                ui.end_row();
            });
        });
    });

    let out2 = ctx.run_ui(raw, |ui| {
        CentralPanel::default().show(ui, |ui| {
            Grid::new("g").show(ui, |ui| {
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.label("Short");
                });
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.label("Tall\nTall\nTall");
                });
                ui.end_row();
            });
        });
    });

    for shape in out2.shapes {
        if let Shape::Text(t) = &shape.shape {
            println!("Text: {:?}, Y: {}", t.galley.text(), t.pos.y);
        }
    }
}
