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
        fn pick<'a>(&mut self, items: &'a [&str]) -> &'a str {
            items[(self.next_u64() as usize) % items.len()]
        }
        fn gen_insurance_text(&mut self, count: usize) -> String {
            let companies = [
                "State Farm",
                "Geico",
                "Progressive",
                "Allstate",
                "Liberty Mutual",
                "Nationwide",
                "USAA",
                "Travelers",
                "Aetna",
                "Cigna",
                "Blue Cross Blue Shield",
                "UnitedHealthcare",
                "MetLife",
                "Prudential",
                "Farmers",
                "American Family",
                "Erie Insurance",
                "Chubb",
            ];
            let terms = [
                "Comprehensive coverage",
                "Collision deductible",
                "Liability limit",
                "Uninsured motorist",
                "Personal injury protection",
                "Medical payments",
                "Property damage",
                "Bodily injury",
                "Rental reimbursement",
                "Roadside assistance",
                "Gap coverage",
                "Annual premium",
                "Monthly payment",
                "Deductible amount",
                "Out-of-pocket maximum",
                "Co-payment",
                "Co-insurance",
                "Policy renewal",
                "Coverage limit",
                "Benefit period",
                "Waiting period",
                "Network provider",
                "Preferred provider",
                "Out-of-network",
                "Prior authorization",
            ];
            let mut parts = Vec::with_capacity(count);
            for _ in 0..count {
                match self.next_u64() % 3 {
                    0 => parts.push(self.pick(&companies).to_string()),
                    _ => parts.push(self.pick(&terms).to_string()),
                }
            }
            let mut s = parts.join(" ");
            if let Some(c) = s.chars().next() {
                let uc = c.to_uppercase().to_string();
                s = uc + &s[c.len_utf8()..];
            }
            s
        }
        fn gen_short_insurance(&mut self) -> String {
            let short_terms = [
                "Deductible",
                "Premium",
                "Co-pay",
                "Liability",
                "Coverage",
                "Policy",
                "Claim",
                "Limit",
                "Waiver",
                "Rider",
                "Exclusion",
                "Benefit",
            ];
            self.pick(&short_terms).to_string()
        }
    }

    const SEED: u64 = 0xdead_beef_cafe_babe;

    /// Verify that `allocate_at_least` + `child_ui` + `with_main_wrap(true)`
    /// wraps content at the FTWA-assigned width `w` inside a Grid *regardless* of
    /// the specific column width or text (randomised over 20 iterations with
    /// realistic insurance data — company names and dollar amounts).
    #[test]
    fn fix_allocate_ui_insurance_data() {
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
                            // Long insurance text — phrase count proportional to column width
                            let phrase_count = (col_w / 45.0).ceil() as usize + 8;
                            let long_text = rng.gen_insurance_text(phrase_count);
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

                            // Short insurance term in same column
                            let short_text = rng.gen_short_insurance();
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
                            let row2_phrases = (col_w / 45.0).ceil() as usize + 5;
                            let r3 = child_ui3.horizontal_wrapped(|ui| {
                                ui.add(
                                    egui::Label::new(rng.gen_insurance_text(row2_phrases))
                                        .wrap(true),
                                );
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
