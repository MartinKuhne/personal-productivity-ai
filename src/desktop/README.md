# FastMD - Hardware-Accelerated Markdown Viewer

## Summary
FastMD is a high-performance, GPU-accelerated Markdown viewer for Windows built using Rust and the `egui` framework. It parses documents in the background, extracts YAML metadata tags, and provides dynamic tag-based filesystem filtering and table rendering.

## Background / Context
Markdown document viewing is historically handled by CPU-bound rendering engines or resource-heavy Electron shells. FastMD solves this by providing a lightweight native binary that leverages hardware acceleration (via DirectX on Windows) to render text and layout elements with minimal CPU and memory footprint, making it ideal for large note vaults or documentation trees.

### Key Features
1. **GPU Acceleration**: Fluid rendering and layout, bypassing large web browser dependencies.
2. **Dynamic File Tree**: The left panel builds a tree of all markdown files and directories in real-time, showing folder expansion states. Double-clicking a file opens it in your Windows default external application.
3. **YAML Front-Matter Parsing**: Extracts front-matter metadata and displays it in a dedicated table format with a distinct container style.
4. **Concurrent Parser Pipeline**: On startup, a dedicated directory enumerator thread scans the filesystem and enqueues markdown file paths into a shared internal work queue. A pool of 4 parallel worker threads pulls files from this queue, parses their YAML front-matter, and updates the GUI in real-time, significantly boosting loading speed on multi-core processors.
5. **Live Directory Watcher**: Schedules a background thread using the Windows directory change API (`notify` crate) to listen to file creations, modifications, and removals. The file tree, tag lists, and currently open document reload instantly on save.
6. **Hierarchical Table of Contents (ToC)**: When a document contains H1 to H3 headers, a right panel is automatically displayed showing a ToC. Clicking a ToC item scrolls the markdown view directly to that header.
7. **Command Prompt Panel**: Displays a prompt panel at the bottom of the window for entering commands. Includes built-in AI agent slash commands: `/models` to list available LLM models and `/model <alias>` to switch the active model on the fly.

## Sources
- Rust Programming Language: [rust-lang.org](https://www.rust-lang.org)
- Egui GUI Library: [github.com/ocornut/egui](https://github.com/ocornut/egui)
- Pulldown-Cmark Parser: [github.com/pulldown-cmark/pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark)
- Walkdir Library: [github.com/BurntSushi/walkdir](https://github.com/BurntSushi/walkdir)
