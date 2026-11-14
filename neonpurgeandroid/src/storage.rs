//! Android save-file location.
//!
//! The desktop build writes next to the executable; Android only guarantees
//! write access to the app's private data directory, so `android_main` hands it
//! to us once at startup.

use std::path::PathBuf;
use std::sync::OnceLock;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init(dir: PathBuf) {
    let _ = DATA_DIR.set(dir);
}

pub fn file(name: &str) -> Option<PathBuf> {
    DATA_DIR.get().map(|dir| dir.join(name))
}
