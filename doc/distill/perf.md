# Rust Performance & Build Optimization Guide

This document distills best practices, tooling, and architectural strategies for optimizing **dependency footprints**, **binary sizes**, and **compilation times** in Rust applications, with empirical baselines and findings from the `fastmd` workspace.

---

## 1. Overview & Optimization Pillars

Performance optimization in Rust build systems spans three interconnected dimensions:

```
                  ┌────────────────────────┐
                  │ Dependency Management  │
                  │ (Crate count, pruning) │
                  └───────────┬────────────┘
                              │
               ┌──────────────┴──────────────┐
               │                             │
┌──────────────▼─────────────┐ ┌─────────────▼──────────────┐
│        Binary Size         │ │       Compile Times        │
│ (LTO, strip, assets, codegen)│ │(Linkers, caching, timings)│
└────────────────────────────┘ └────────────────────────────┘
```

Each optimization involves conscious engineering trade-offs:

| Optimization | Primary Benefit | Potential Trade-Off |
| :--- | :--- | :--- |
| **Link-Time Optimization (`lto = "fat"`)** | Maximizes dead code removal and binary size reduction | Significantly slower release link times |
| **Size Optimization (`opt-level = "z"`)** | Minimizes final binary footprint | Minor decrease in peak throughput vs `opt-level = 3` |
| **High-Performance Linkers (`lld-link` / `mold`)** | Up to 5-10x faster linking phase | Requires external toolchain install |
| **Debug Info Reduction (`line-tables-only`)** | Fast inner dev loops, smaller debug binaries | Less rich variable inspection during debugging |
| **Embedded Asset Decoupling** | Massive reduction in executable size (tens of MBs) | Requires runtime asset loading or sidecar deployment |

---

## 2. Dependency Management & Pruning

### 2.1 Tooling Matrix

| Tool | Role | Mechanism | Command / Workflow |
| :--- | :--- | :--- | :--- |
| **`cargo-tree`** | Graph inspection | Built-in Cargo resolver | `cargo tree --edges no-dev`<br>`cargo tree -d` (duplicates)<br>`cargo tree -i <crate>` (reverse dependencies) |
| **`cargo-machete`** | Fast unused dep detection | Fast regex / AST scanning of code and `Cargo.toml` | `cargo-machete` (zero compilation, ideal for CI and pre-commit) |
| **`cargo-udeps`** | Precise unused dep detection | Compiles crate and checks compiler metadata | `cargo +nightly udeps --all-targets` (periodic / release audits) |
| **`cargo-deny`** | Policy & compliance auditing | Lints advisory databases, licenses, bans, duplicate versions | `cargo deny check advisories bans sources` |
| **`cargo-hakari`** | Workspace feature unification | Generates a unified `workspace-hack` crate | `cargo hakari init` / `cargo hakari generate` |

### 2.2 Best Practices

1. **Disable Default Features on Omnibus Crates**:
   Never depend on full default features for heavy network, async, image, or UI crates. Always specify `default-features = false` and select only needed capabilities:
   ```toml
   # Example: Minimal Reqwest with single TLS stack
   reqwest = { version = "0.13", default-features = false, features = ["blocking", "json", "rustls"] }
   
   # Example: Minimal Tokio runtime features
   tokio = { version = "1.53", features = ["rt-multi-thread", "macros", "sync"] }
   ```

2. **Feature-Gate Heavy and Non-Essential Subsystems**:
   Non-core capabilities (vector databases, browser drivers, chat integrations) should never be unconditional dependencies:
   ```toml
   [features]
   default = []
   vector-search = ["dep:qdrant-client", "dep:text-splitter"]
   browser = ["dep:playwright-rs"]
   ```

3. **Eliminate Duplicate Versions**:
   In complex workspaces, transitive dependencies frequently pull multiple versions of identical crates (e.g., `syn 2.x` vs `syn 3.x`, `windows-sys 0.52` vs `0.61`). Use `cargo tree -d` to detect version splits and align version specifications.

4. **Isolate Heavy Modules into Dedicated Crates**:
   Extracting heavy subsystems into separate workspace crates prevents changes in core logic from triggering cascading recompilations of the heavy dependency tree.

---

## 3. Binary Size Optimization

### 3.1 Recommended Profile Configurations

Configure release profiles in `Cargo.toml` or `.cargo/config.toml`:

```toml
[profile.release]
# Strip debug symbols and symbol tables from binary
strip = true

# Optimize for size ('z' for aggressive size, 's' for balanced size)
opt-level = "z"

# Thin LTO provides ~80-90% of Fat LTO reduction with substantially faster link times
lto = "thin"

# Single codegen unit allows LLVM to perform whole-crate optimization and dead code elimination
codegen-units = 1

# Abort on panic to remove stack unwinding landing pads and EH tables
panic = "abort"
```

### 3.2 Profiling Tools

- **`cargo-bloat`**:
  ```bash
  cargo install cargo-bloat
  # View top size-contributing crates
  cargo bloat --release --crates
  # View top size-contributing functions
  cargo bloat --release -n 30
  ```
- **`cargo-llvm-lines`**:
  Identifies generic functions that generate excessive monomorphized object code:
  ```bash
  cargo install cargo-llvm-lines
  cargo llvm-lines --release
  ```

### 3.3 Asset Decoupling & Static Embedding

Embedding large static assets (such as font collections or model weights) directly into binaries with `include_bytes!` or crate features can inflate executable size by tens of megabytes:
- **Embedded Fonts vs System Fonts**: Crates like `typst-as-lib` include optional features like `typst-kit-embed-fonts` (~8.4 MB of bundled font data). By switching to `typst-kit-fonts`, the application resolves fonts from the host operating system at runtime with zero binary size overhead.
- **Sidecar Architecture**: Heavy subsystems that are only invoked on-demand (e.g., PDF generation, document export) can be compiled into an auxiliary CLI sidecar or invoked via a background worker process, keeping the main interactive desktop binary lean.

---

## 4. Compile Time Acceleration

### 4.1 High-Performance Linkers

Linking is the single most common bottleneck during development builds, often taking 50-80% of inner-loop build time.

#### Windows Configuration (`.cargo/config.toml`)
Switch to LLVM's `lld-link`:
```toml
[target.x86_64-pc-windows-msvc]
linker = "lld-link.exe"
```
*(Requires LLVM installed via Visual Studio Installer or `scoop install llvm`)*

#### Linux / macOS Configuration (`.cargo/config.toml`)
Switch to `mold` (fastest modern linker for Linux) or `lld`:
```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

### 4.2 Compiler Caching with `sccache`

`sccache` wraps `rustc` to cache compiled crate artifacts locally or across CI runners (using S3, GCS, or Redis):
```toml
[build]
rustc-wrapper = "sccache"
```
Command line setup:
```bash
cargo install sccache
set RUSTC_WRAPPER=sccache
```

### 4.3 Inner-Loop Dev Profile Tuning

Speed up daily development builds without degrading runtime debuggability:

```toml
[profile.dev]
# Generate line tables only, avoiding full variable inspection tables
debug = "line-tables-only"

# Keep debuginfo separate from object files to minimize linker I/O
split-debuginfo = "packed" # on Windows, "unpacked" on Linux

# Maximize parallel codegen threads for rapid dev compilation
codegen-units = 16

# Keep incremental compilation active
incremental = true
```

### 4.4 Build Profiling (`cargo build --timings`)

Cargo has a built-in build profiler that generates an interactive HTML report:
```bash
cargo build --timings
# Report generated at: target/cargo-timings/cargo-timing.html
```
The report reveals:
- **Critical path**: The chain of dependencies that blocked parallel compilation.
- **Unit concurrency**: How many CPU cores were idle while waiting for heavy procedural macros or leaf crates.
- **Longest compiling crates**: Primary targets for optimization or crate splitting.

### 4.5 Windows Platform Optimizations

- **Windows Dev Drive (ReFS)**: Placing the repository and `CARGO_TARGET_DIR` on an ReFS Dev Drive reduces file system metadata overhead by up to 20-30% during Rust builds.
- **Antivirus Exclusions**: Add the workspace `target/` folder and `~/.cargo/` to the Windows Defender exclusion list to prevent real-time scan locks on `.rlib` and `.pdb` writes.

---

## 5. Case Study: FastMD Empirical Findings

An empirical audit of the `fastmd` workspace demonstrated the practical impact of these practices:

### Baseline Distribution

| Subsystem | Transitive Crates | Exclusive Crates | Binary Size Impact | Dev Check Time |
| :--- | :--- | :--- | :--- | :--- |
| **PDF Export (`fastmd-pdf`)** | 298 | **168** | **+43.82 MB (57.6%)** | **24.0s** |
| **AI Agent (`fastmd-agent`)** | 215 | **59** | ~6.5 MB | **4.1s** |
| **GUI Layer (`eframe` / `wgpu`)** | 158 | **51** | ~18.2 MB | ~12.0s |
| **Base Core (`fastmd`)** | 351 | — | **32.23 MB** | ~21.4s |
| **Full App (Default)** | 519 | — | **76.05 MB** | ~45.0s+ (clean) |

### Key Takeaways for FastMD
1. **PDF Export Dominance**: The Typst engine (`fastmd-pdf`) accounts for **57.6% of release binary size** and **32.4% of all dependencies**.
2. **Font Decoupling Opportunity**: Disabling `typst-kit-embed-fonts` in `src/md2pdf/Cargo.toml` and relying on host system fonts via `typst-kit-fonts` can yield an immediate multi-megabyte size reduction.
3. **Linker Acceleration**: Configuring `lld-link` directly addresses the link latency on Windows for the 76 MB executable.
