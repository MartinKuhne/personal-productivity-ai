//! Build script — application icon embedding and compile-time build metadata generation.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if output.status.success() {
        let s = String::from_utf8(output.stdout).ok()?;
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn format_current_date() -> String {
    let secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });
    let days = secs / 86400;
    // Howard Hinnant's algorithm for converting epoch days to civil date
    let z = days as i64 + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        // Also set language-neutral manifest defaults so the icon is embedded even without a .rc file.
        if let Err(e) = res.compile() {
            // Fail the build loudly if the icon is missing or malformed.
            eprintln!("winresource compile failed: {e}");
            std::process::exit(1);
        }
    }

    let branch = std::env::var("GIT_BRANCH")
        .ok()
        .or_else(|| get_git_output(&["rev-parse", "--abbrev-ref", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());

    let commit_hash = std::env::var("GIT_COMMIT")
        .ok()
        .or_else(|| get_git_output(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());

    let short_hash = if commit_hash != "unknown" && commit_hash.len() >= 8 {
        commit_hash[..8].to_string()
    } else {
        commit_hash.clone()
    };

    let build_date = format_current_date();

    println!("cargo:rustc-env=BUILD_BRANCH={branch}");
    println!("cargo:rustc-env=BUILD_COMMIT_HASH={commit_hash}");
    println!("cargo:rustc-env=BUILD_COMMIT_SHORT_HASH={short_hash}");
    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
