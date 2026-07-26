#[cfg(test)]
mod tests {
    use eframe::egui;

    /// Probe the egui Grid bug: `set_width` + `horizontal_wrapped` produces
    /// content wrapped at ~40 px (the Grid's initial column allocation) instead
    /// of the assigned column width.
    #[test]
    fn old_bug_set_width_ignored() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::Grid::new("bug_grid")
                    .striped(true)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        // Row1 C0 — set_width(100) + horizontal_wrapped
                        ui.set_width(100.0);
                        let hw = ui.horizontal_wrapped(|ui| {
                            ui.add(egui::Label::new("Short").wrap(true));
                        });
                        // response_rect width should be ~40 (wrapped at Grid default column allocation)
                        dbg!(hw.response.rect.width());

                        // Row1 C1
                        ui.set_width(200.0);
                        let hw2 = ui.horizontal_wrapped(|ui| {
                            ui.add(
                                egui::Label::new("Much longer text here for wrapping testing")
                                    .wrap(true),
                            );
                        });
                        dbg!(hw2.response.rect.width());

                        ui.end_row();

                        // Row2 C0
                        ui.set_width(100.0);
                        let hw3 = ui.horizontal_wrapped(|ui| {
                            ui.add(egui::Label::new("Another").wrap(true));
                        });
                        dbg!(hw3.response.rect.width());
                    });
            });
        });
    }

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
        /// Generate a "word" of random length (1–12 characters).
        fn gen_word(&mut self) -> String {
            let len = (self.next_u64() as usize) % 12 + 1;
            let mut s = String::with_capacity(len);
            for _ in 0..len {
                let ch = b'a' + (self.next_u64() as u8) % 26;
                s.push(ch as char);
            }
            s
        }
        /// Generate a sentence of `count` random words.
        fn gen_sentence(&mut self, count: usize) -> String {
            let mut words: Vec<String> = (0..count).map(|_| self.gen_word()).collect();
            words[0] = {
                let mut c = words[0].chars();
                c.next().unwrap().to_uppercase().to_string() + c.as_str()
            };
            words.join(" ") + "."
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

            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let col_w = rng.gen_f32(80.0, 400.0);

                    egui::Grid::new(("fix_random_grid", iteration))
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            // Long sentence — word count proportional to column width
                            // so text is guaranteed to wrap at col_w.
                            let word_count = (col_w / 35.0).ceil() as usize + 10;
                            let long_text = rng.gen_sentence(word_count);
                            let (rect, _) = ui
                                .allocate_at_least(egui::vec2(col_w, 0.0), egui::Sense::hover());
                            let layout = egui::Layout::left_to_right(egui::Align::Min)
                                .with_main_wrap(true);
                            let mut child_ui = ui.child_ui(rect, layout);
                            let r = child_ui.horizontal_wrapped(|ui| {
                                ui.add(egui::Label::new(long_text).wrap(true));
                            });
                            // Content wraps at col_w (not ~40 like the bug)
                            let content_w = r.response.rect.width();
                            let slack = (col_w * 0.15).max(20.0);
                            assert!(
                                (content_w - col_w).abs() < slack,
                                "Iter {iteration}: col_w={col_w:.0}, content_w={content_w:.0} (diff={:.0}, slack={slack:.0})",
                                (content_w - col_w).abs()
                            );

                            // Short text in same column
                            let short_text = rng.gen_word();
                            let (rect, _) = ui
                                .allocate_at_least(egui::vec2(col_w, 0.0), egui::Sense::hover());
                            let layout = egui::Layout::left_to_right(egui::Align::Min)
                                .with_main_wrap(true);
                            let mut child_ui2 = ui.child_ui(rect, layout);
                            let r2 = child_ui2.horizontal_wrapped(|ui| {
                                ui.add(egui::Label::new(short_text).wrap(true));
                            });
                            let w2 = r2.response.rect.width();
                            assert!(
                                w2 > 8.0 && w2 < col_w + 5.0,
                                "Iter {iteration}: short text width {w2:.0} out of range"
                            );

                            ui.end_row();

                            // Second row — reuses column; wraps correctly again
                            let (rect, _) = ui
                                .allocate_at_least(egui::vec2(col_w, 0.0), egui::Sense::hover());
                            let layout = egui::Layout::left_to_right(egui::Align::Min)
                                .with_main_wrap(true);
                            let mut child_ui3 = ui.child_ui(rect, layout);
                            let row2_words = (col_w / 35.0).ceil() as usize + 5;
                            let r3 = child_ui3.horizontal_wrapped(|ui| {
                                ui.add(egui::Label::new(rng.gen_sentence(row2_words)).wrap(true));
                            });
                            let w3 = r3.response.rect.width();
                            let slack3 = (col_w * 0.15).max(20.0);
                            assert!(
                                (w3 - col_w).abs() < slack3,
                                "Iter {iteration}: row2 col_w={col_w:.0}, w3={w3:.0} (diff={:.0}, slack={slack3:.0})",
                                (w3 - col_w).abs()
                            );
                        });
                });
            });
        }
    }
}
