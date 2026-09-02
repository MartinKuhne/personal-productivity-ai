//! Tests for `tree/render.rs`.

#[cfg(test)]
mod tests {
    // UI rendering tests require an egui context and are difficult to maintain.
    // The core interactions have been extracted to `handlers.rs` and tested in `handlers_tests.rs`.
    // The context menu actions directly publish UserCommand, which is tested at the Executor level.
}
