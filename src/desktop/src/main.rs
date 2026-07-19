#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fastmd::ui::FastMdApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // Install rustls crypto provider
    rustls::crypto::ring::default_provider().install_default().ok();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("⚡ FastMD Viewer"),
        ..Default::default()
    };

    let mut config = fastmd::config::load_config();
    
    if !config.models.is_empty() {
        let mut lowest_cost = i32::MAX;
        let mut best_model_key = None;
        for (key, cfg) in &config.models {
            let cost = cfg.get_cost();
            if cost < lowest_cost {
                lowest_cost = cost;
                best_model_key = Some(key.clone());
            }
        }
        if let Some(key) = best_model_key {
            if let Some(best_cfg) = config.models.get(&key) {
                config.model = best_cfg.model.clone();
                config.api_url = best_cfg.api_url.clone();
                config.api_key = best_cfg.api_key.clone();
                
                let config_path = fastmd::config::get_config_path();
                if let Ok(yaml_str) = serde_yaml::to_string(&config) {
                    let _ = std::fs::write(&config_path, yaml_str);
                }
            }
        }
    }

    let prompt = fastmd::agent::get_base_system_prompt(&config);
    println!("--- System Prompt (Startup) ---\n{}\n-------------------------------", prompt);

    eframe::run_native(
        "fastmd",
        options,
        Box::new(|cc| Box::new(FastMdApp::new(cc))),
    )
}