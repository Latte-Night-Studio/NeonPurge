//! Where the game keeps its small save files: settings and the high score.
//!
//! On desktop they live next to the executable so a portable copy carries its
//! preferences with it. The Android build provides its own implementation that
//! points at the app's private data directory instead.

use std::path::PathBuf;

/// Full path for one of the game's save files.
pub fn file(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Some(dir.join(name))
}
