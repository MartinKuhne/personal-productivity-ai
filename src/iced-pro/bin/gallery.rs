//! Runnable visual gallery for the `UIF-VER-004` gate.
//!
//! Builds the whole `(Variant × State)` matrix through
//! [`iced_pro::gallery`], renders every cell's frame through the recording
//! renderer, and writes an inspectable artifact: one `manifest.txt` with
//! every cell's resolved-style snapshot plus the gate ledgers, and one
//! `cells/<id>.txt` per cell for diffing a single cell in isolation.
//!
//! The snapshot is resolved style values and the recorded quad list, not a
//! rasterised image — see the [gallery module docs](iced_pro::gallery) for
//! why. Nothing here opens a window, reads a clock, or touches a real user
//! configuration path: the output directory is either given on the command
//! line or defaulted to a subdirectory of the system temp directory, and
//! [`guard`] refuses the application's own configuration roots outright.
//!
//! ```text
//! cargo run -p iced-pro --bin gallery -- C:\temp\gallery
//! ```

use iced_pro::gallery::{self, Cell, CellKind};

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Environment variables whose exact directories the gallery must never
/// write into.
///
/// These are the roots a FastMD configuration write would land in
/// (`RUST-024`), so a mistyped output argument that resolved to one of
/// them is refused rather than honoured. A *subdirectory* of the temp
/// directory is fine — the rule is about the app's own config roots, not
/// about the temp tree the default lives in.
const FORBIDDEN_ROOT_VARS: [&str; 3] = ["APPDATA", "LOCALAPPDATA", "USERPROFILE"];

/// The filename fragment that marks a FastMD application directory.
const FASTMD_MARKER: &str = ".fastmd";

/// Why a path was refused, if it was.
///
/// Case-insensitive and separator-normalised, because the caller may pass
/// either Windows or POSIX separators and Windows paths are
/// case-preserving but not case-sensitive.
fn refusal(path: &Path) -> Option<String> {
    let normalised = path.to_string_lossy().replace('\\', "/").to_lowercase();

    if normalised
        .split('/')
        .any(|component| component.starts_with(FASTMD_MARKER))
    {
        return Some(format!(
            "path contains a {FASTMD_MARKER} application directory component"
        ));
    }

    for variable in FORBIDDEN_ROOT_VARS {
        let Ok(value) = std::env::var(variable) else {
            continue;
        };
        let root = value.replace('\\', "/").to_lowercase();
        if root.is_empty() {
            continue;
        }
        if normalised == root {
            return Some(format!("path is the ${variable} root ({value})"));
        }
    }

    None
}

/// Refuse `path` or return it as an absolute path.
fn guard(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(reason) = refusal(path) {
        return Err(format!("refusing to write output: {reason}").into());
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if let Some(reason) = refusal(&absolute) {
        return Err(format!("refusing to write output: {reason}").into());
    }
    Ok(absolute)
}

/// The directory a run with no argument writes into.
fn default_output_dir() -> PathBuf {
    std::env::temp_dir().join("iced-pro-gallery")
}

/// A cell id as a filename: `/` is the id's own hierarchy separator, so
/// it becomes `__` and the id stays readable and unique on disk.
fn cell_filename(id: &str) -> String {
    format!("{}.txt", id.replace('/', "__"))
}

/// Per-subject cell counts, in first-appearance order.
fn subject_counts(cells: &[Cell]) -> Vec<(&'static str, usize)> {
    let mut order: Vec<&'static str> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();

    for cell in cells {
        if let Some(index) = order.iter().position(|subject| *subject == cell.subject) {
            counts[index] += 1;
        } else {
            order.push(cell.subject);
            counts.push(1);
        }
    }

    order.into_iter().zip(counts).collect()
}

/// Write the artifact and print the run summary.
fn run(output: &Path) -> Result<(), Box<dyn Error>> {
    let cells = gallery::matrix();

    fs::create_dir_all(output)?;
    let cells_dir = output.join("cells");
    fs::create_dir_all(&cells_dir)?;

    fs::write(output.join("manifest.txt"), gallery::manifest())?;

    for cell in &cells {
        fs::write(cells_dir.join(cell_filename(&cell.id)), cell.snapshot())?;
    }

    let unmapped = gallery::unmapped_colours(&cells);
    let contrast = gallery::contrast_failures(&cells);
    let not_owned = cells
        .iter()
        .filter(|cell| cell.kind == CellKind::NotOwned)
        .count();

    println!("iced-pro visual gallery (UIF-VER-004)");
    println!("  output        : {}", output.display());
    println!(
        "  manifest      : {}",
        output.join("manifest.txt").display()
    );
    println!("  cell files    : {}", cells_dir.display());
    println!("  cells         : {}", cells.len());
    println!("  NotOwned      : {not_owned}");
    println!("  unmapped      : {}", unmapped.len());
    println!("  contrast fails: {}", contrast.len());
    println!("  by subject    :");
    for (subject, count) in subject_counts(&cells) {
        println!("    {subject:<16} {count}");
    }

    if !unmapped.is_empty() || !contrast.is_empty() {
        return Err("gallery gates failed; see manifest.txt".into());
    }

    Ok(())
}

fn main() {
    let requested = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_output_dir);

    let output = match guard(&requested) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = run(&output) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
