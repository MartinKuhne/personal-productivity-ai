//! Windows resource embedding — application icon for taskbar, Alt-Tab, and Explorer.

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
}
