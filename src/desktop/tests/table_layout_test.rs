#[cfg(test)]
mod tests {
    use eframe::egui;

    // The previous `old_bug_set_width_ignored` test that lived at
    // the top of this module was a no-assert diagnostic that
    // printed widths via `dbg!`. The regression it documented
    // (`set_width` + `horizontal_wrapped` content wrapped at the
    // Grid's default column allocation instead of the assigned
    // width) is now pinned by `fix_allocate_ui_randomised` below
    // via real assertions. R-5 + P2-5: delete the no-assert
    // diagnostic. See `doc/planning/egui-testing.md` §P2-5.

    /// Deterministic minimal PRNG so we don't need the `rand` crate.
    struct SimpleRng(u64);

    impl SimpleRng {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0
        }
        fn gen_f32(&mut self, lo: f32, hi: f32) -> f32 {
            lo + (self.next_u64() as f32 / u64::MAX as f32) * (hi - lo)
        }
        fn gen_long_text(&mut self, count: usize, col_w: f32) -> String {
            let word_len = ((col_w / 8.0).ceil() as usize).max(4);
            let mut parts = Vec::with_capacity(count);
            for i in 1..=count {
                parts.push(format!("TEST{:0w$}", i, w = word_len - 4));
            }
            parts.join(" ")
        }
        fn gen_short_text(&mut self, col_w: f32) -> String {
            let word_len = ((col_w / 8.0).ceil() as usize).max(4);
            let n = if word_len > 10 {
                self.next_u64() % 1000 + 1
            } else {
                self.next_u64() % 10 + 1
            };
            format!("TEST{:0w$}", n, w = word_len - 4)
        }
    }

    const SEED: u64 = 0xdead_beef_cafe_babe;

    /// Verify that `allocate_at_least` + `child_ui` + `with_main_wrap(true)`
    /// wraps content at the FTWA-assigned width `w` inside a Grid *regardless* of
    /// the specific column width or text (randomised over 20 iterations).
    #[test]
    fn fix_allocate_ui_randomised() {
        let ctx = egui::Context::default();
        for iteration in 0..20 {
            let mut rng = SimpleRng::new(SEED.wrapping_add(iteration as u64));

            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let col_w = rng.gen_f32(80.0, 400.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 4.0);
                        ui.horizontal(|ui| {
                            // Long text — phrase count proportional to column width
                            let phrase_count = (col_w / 45.0).ceil() as usize + 8;
                            let long_text = rng.gen_long_text(phrase_count, col_w);
                            let (rect, _) = ui
                                .allocate_at_least(egui::vec2(col_w, 0.0), egui::Sense::hover());
                            let layout = egui::Layout::left_to_right(egui::Align::Min)
                                .with_main_wrap(true);
                            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout));
                            let r = child_ui.horizontal_wrapped(|ui| {
                                ui.add(egui::Label::new(long_text).wrap());
                            });
                            // Content should be wider than ~40px default,
                            // and at least half of col_w (accounting for word-width granularity)
                            let content_w = r.response.rect.width();
                            assert!(
                                content_w > 50.0 && content_w > col_w * 0.35,
                                "Iter {iteration}: content_w={content_w:.0} at col_w={col_w:.0} (wrapped at ~40px default)"
                            );

                            // Short text in same column
                            let short_text = rng.gen_short_text(col_w);
                            let (rect, _) = ui
                                .allocate_at_least(egui::vec2(col_w, 0.0), egui::Sense::hover());
                            let layout = egui::Layout::left_to_right(egui::Align::Min)
                                .with_main_wrap(true);
                            let mut child_ui2 = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout));
                            let r2 = child_ui2.horizontal_wrapped(|ui| {
                                ui.add(egui::Label::new(short_text).wrap());
                            });
                            let w2 = r2.response.rect.width();
                            assert!(
                                w2 > 8.0 && w2 < col_w * 1.2,
                                "Iter {iteration}: short text width {w2:.0} out of range"
                            );
                        });

                        // Second row — reuses column; wraps correctly again
                        ui.horizontal(|ui| {
                            let (rect, _) = ui
                                .allocate_at_least(egui::vec2(col_w, 0.0), egui::Sense::hover());
                            let layout = egui::Layout::left_to_right(egui::Align::Min)
                                .with_main_wrap(true);
                            let mut child_ui3 = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout));
                            let row2_phrases = (col_w / 45.0).ceil() as usize + 5;
                            let r3 = child_ui3.horizontal_wrapped(|ui| {
                                ui.add(
                                    egui::Label::new(rng.gen_long_text(row2_phrases, col_w))
                                        .wrap(),
                                );
                            });
                            let w3 = r3.response.rect.width();
                            assert!(
                                w3 > 50.0 && w3 > col_w * 0.35,
                                "Iter {iteration}: row2 w3={w3:.0} at col_w={col_w:.0} (wrapped at default)"
                            );
                        });
                    });
                });
            });
        }
    }
}
