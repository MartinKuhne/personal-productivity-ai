//! Attribution catalog — curated direct third-party dependencies for the About dialog.
//!
//! The catalog enumerates every direct external crate across all workspace
//! members (`fastmd`, `fastmd-agent`, `fastmd-pdf`, `fastmd-tool-macros`),
//! excluding workspace-internal members. Each entry carries author and
//! GitHub URL for display.
//!
//! Unit tests live in the sibling `attributions_tests.rs` sidecar.

/// Represents a direct third-party crate dependency attribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribution {
    /// Crate package name.
    pub name: &'static str,
    /// Author(s) name(s) or maintainer organization.
    pub authors: &'static str,
    /// GitHub repository URL.
    pub github_url: &'static str,
}

/// All direct third-party dependencies across the workspace, sorted
/// alphabetically by `name` with no duplicates. See `SPEC.md` FR-009
/// and `data-model.md` for provenance.
pub const DIRECT_DEPENDENCIES: &[Attribution] = &[
    Attribution {
        name: "anyhow",
        authors: "David Tolnay",
        github_url: "https://github.com/dtolnay/anyhow",
    },
    Attribution {
        name: "arboard",
        authors: "1Password",
        github_url: "https://github.com/1Password/arboard",
    },
    Attribution {
        name: "arc-swap",
        authors: "Michal 'vorner' Vaner",
        github_url: "https://github.com/vorner/arc-swap",
    },
    Attribution {
        name: "async-openai",
        authors: "Himanshu Neema",
        github_url: "https://github.com/64bit/async-openai",
    },
    Attribution {
        name: "async-trait",
        authors: "David Tolnay",
        github_url: "https://github.com/dtolnay/async-trait",
    },
    Attribution {
        name: "base64",
        authors: "Marshall Pierce",
        github_url: "https://github.com/marshallpierce/rust-base64",
    },
    Attribution {
        name: "bytes",
        authors: "Carl Lerche, Sean McArthur, Tokio Contributors",
        github_url: "https://github.com/tokio-rs/bytes",
    },
    Attribution {
        name: "chrono",
        authors: "Chrono Contributors",
        github_url: "https://github.com/chronotope/chrono",
    },
    Attribution {
        name: "csv",
        authors: "Andrew Gallant",
        github_url: "https://github.com/BurntSushi/rust-csv",
    },
    Attribution {
        name: "eframe",
        authors: "Emil Ernerfeldt",
        github_url: "https://github.com/emilk/egui",
    },
    Attribution {
        name: "egui-phosphor",
        authors: "amPerl",
        github_url: "https://github.com/amPerl/egui-phosphor",
    },
    Attribution {
        name: "egui_extras",
        authors: "Dominik Rössler, Emil Ernerfeldt, René Rössler",
        github_url: "https://github.com/emilk/egui",
    },
    Attribution {
        name: "encoding_rs",
        authors: "Henri Sivonen",
        github_url: "https://github.com/hsivonen/encoding_rs",
    },
    Attribution {
        name: "evalexpr",
        authors: "isibboi",
        github_url: "https://github.com/ISibboI/evalexpr",
    },
    Attribution {
        name: "fast_h2m",
        authors: "Na'aman Hirschfeld, Yehor Smoliakov",
        github_url: "https://github.com/MartinKuhne/fast_h2m",
    },
    Attribution {
        name: "futures-util",
        authors: "Tokio Contributors, Rust Project Developers",
        github_url: "https://github.com/rust-lang/futures-rs",
    },
    Attribution {
        name: "glob",
        authors: "The Rust Project Developers",
        github_url: "https://github.com/rust-lang/glob",
    },
    Attribution {
        name: "html-escape",
        authors: "Magic Len",
        github_url: "https://github.com/magiclen/html-escape",
    },
    Attribution {
        name: "image",
        authors: "The image-rs Developers",
        github_url: "https://github.com/image-rs/image",
    },
    Attribution {
        name: "jmap-client",
        authors: "Stalwart Labs LLC",
        github_url: "https://github.com/fastmail/jmap-client-rs",
    },
    Attribution {
        name: "mimalloc",
        authors: "Octavian Oncescu, Vincent Rouillé, Thom Chiovoloni",
        github_url: "https://github.com/purpleprotocol/mimalloc_rust",
    },
    Attribution {
        name: "mini-moka",
        authors: "Tatsuya Kawano",
        github_url: "https://github.com/moka-rs/mini-moka",
    },
    Attribution {
        name: "notify",
        authors: "Félix Saparelli, Daniel Faust, Aron Heinecke",
        github_url: "https://github.com/notify-rs/notify",
    },
    Attribution {
        name: "opener",
        authors: "Brian Bowman",
        github_url: "https://github.com/Seeker14491/opener",
    },
    Attribution {
        name: "playwright-rs",
        authors: "Luc Perkins, Dzenan Jupic",
        github_url: "https://github.com/lucperkins/playwright-rs",
    },
    Attribution {
        name: "proc-macro2",
        authors: "David Tolnay, Alex Crichton",
        github_url: "https://github.com/dtolnay/proc-macro2",
    },
    Attribution {
        name: "pulldown-cmark",
        authors: "Raph Levien, Marcus Klaas de Vries",
        github_url: "https://github.com/pulldown-cmark/pulldown-cmark",
    },
    Attribution {
        name: "qdrant-client",
        authors: "Qdrant Team",
        github_url: "https://github.com/qdrant/rust-client",
    },
    Attribution {
        name: "quote",
        authors: "David Tolnay",
        github_url: "https://github.com/dtolnay/quote",
    },
    Attribution {
        name: "rayon",
        authors: "Niko Matsakis, Josh Stone",
        github_url: "https://github.com/rayon-rs/rayon",
    },
    Attribution {
        name: "reqwest",
        authors: "Sean McArthur",
        github_url: "https://github.com/seanmonstar/reqwest",
    },
    Attribution {
        name: "rfd",
        authors: "Poly (Bartłomiej Maryńczak)",
        github_url: "https://github.com/PolyMeilex/rfd",
    },
    Attribution {
        name: "ring",
        authors: "Brian Smith",
        github_url: "https://github.com/briansmith/ring",
    },
    Attribution {
        name: "roxmltree",
        authors: "Yevhenii Reizner",
        github_url: "https://github.com/RazrFalcon/roxmltree",
    },
    Attribution {
        name: "rustls",
        authors: "Rustls Contributors",
        github_url: "https://github.com/rustls/rustls",
    },
    Attribution {
        name: "schemars",
        authors: "Graham Esau",
        github_url: "https://github.com/GREsau/schemars",
    },
    Attribution {
        name: "serde",
        authors: "Erick Tryzelaar, David Tolnay",
        github_url: "https://github.com/serde-rs/serde",
    },
    Attribution {
        name: "serde_json",
        authors: "Erick Tryzelaar, David Tolnay",
        github_url: "https://github.com/serde-rs/json",
    },
    Attribution {
        name: "serde_norway",
        authors: "Christina Sørensen, David Tolnay",
        github_url: "https://github.com/cafkafk/serde-yaml",
    },
    Attribution {
        name: "syn",
        authors: "David Tolnay",
        github_url: "https://github.com/dtolnay/syn",
    },
    Attribution {
        name: "tempfile",
        authors: "Steven Allen, Ashley Mannix, Jason White",
        github_url: "https://github.com/Stebalien/tempfile",
    },
    Attribution {
        name: "text-splitter",
        authors: "Ben Brandt",
        github_url: "https://github.com/benbrandt/text-splitter",
    },
    Attribution {
        name: "thiserror",
        authors: "David Tolnay",
        github_url: "https://github.com/dtolnay/thiserror",
    },
    Attribution {
        name: "tokio",
        authors: "Tokio Contributors",
        github_url: "https://github.com/tokio-rs/tokio",
    },
    Attribution {
        name: "tokio-tungstenite",
        authors: "Alexey Galakhov, Roman Proskuryakov",
        github_url: "https://github.com/snapview/tokio-tungstenite",
    },
    Attribution {
        name: "tracing",
        authors: "Eliza Weisman, Tokio Contributors",
        github_url: "https://github.com/tokio-rs/tracing",
    },
    Attribution {
        name: "tracing-subscriber",
        authors: "Eliza Weisman, David Barsky, Tokio Contributors",
        github_url: "https://github.com/tokio-rs/tracing",
    },
    Attribution {
        name: "trash",
        authors: "Artur Kovacs",
        github_url: "https://github.com/ArturKovacs/trash",
    },
    Attribution {
        name: "typst",
        authors: "The Typst Project Developers",
        github_url: "https://github.com/typst/typst",
    },
    Attribution {
        name: "typst-as-lib",
        authors: "Laurenz Mädje",
        github_url: "https://github.com/typst/typst",
    },
    Attribution {
        name: "typst-kit",
        authors: "The Typst Project Developers",
        github_url: "https://github.com/typst/typst",
    },
    Attribution {
        name: "typst-layout",
        authors: "The Typst Project Developers",
        github_url: "https://github.com/typst/typst",
    },
    Attribution {
        name: "typst-pdf",
        authors: "The Typst Project Developers",
        github_url: "https://github.com/typst/typst",
    },
    Attribution {
        name: "url",
        authors: "The rust-url Developers",
        github_url: "https://github.com/servo/rust-url",
    },
    Attribution {
        name: "uuid",
        authors: "Ashley Mannix, Dylan DPC, Hunar Roop Kahlon",
        github_url: "https://github.com/uuid-rs/uuid",
    },
    Attribution {
        name: "walkdir",
        authors: "Andrew Gallant",
        github_url: "https://github.com/BurntSushi/walkdir",
    },
    Attribution {
        name: "webbrowser",
        authors: "Amod Malviya",
        github_url: "https://github.com/amodm/webbrowser-rs",
    },
    Attribution {
        name: "windows",
        authors: "Microsoft",
        github_url: "https://github.com/microsoft/windows-rs",
    },
];

#[cfg(test)]
#[path = "attributions_tests.rs"]
mod attributions_tests;
