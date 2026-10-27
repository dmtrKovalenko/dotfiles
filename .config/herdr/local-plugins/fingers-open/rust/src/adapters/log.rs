//! A tiny append-only log in the plugin's state directory, for the moments
//! when the overlay is already gone and stderr with it.

use std::io::Write;
use std::path::Path;

pub const LOG_FILE_NAME: &str = "herdr-fingers.log";

pub fn append(state_dir: Option<&Path>, message: &str) {
    let Some(dir) = state_dir else { return };
    let _ = std::fs::create_dir_all(dir);
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(LOG_FILE_NAME))
    else {
        return;
    };
    let _ = writeln!(file, "{message}");
}
