//! Deploy helper binary — runs `cargo build --release` and copies the resulting `fastmd.exe` to `C:\tools`.

use std::fs;
use std::process::Command;

fn main() {
    println!("Building optimized binary for deployment...");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("Failed to execute cargo build");

    if !status.success() {
        eprintln!("Build failed!");
        std::process::exit(1);
    }

    let target_dir = std::path::Path::new("C:\\tools");
    if !target_dir.exists() {
        println!("Creating target directory: {:?}", target_dir);
        fs::create_dir_all(target_dir).expect("Failed to create target directory");
    }

    let source_bin = std::path::Path::new("target\\release\\fastmd.exe");
    let target_bin = target_dir.join("fastmd.exe");

    println!("Deploying binary to {:?}", target_bin);
    match fs::copy(source_bin, &target_bin) {
        Ok(_) => println!("Deployment successful!"),
        Err(e) => {
            eprintln!("Deployment failed: {}", e);
            std::process::exit(1);
        }
    }
}
