//! Reuse the installed custom Fingers router instead of duplicating it.
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn open(text: &str, source_pane: Option<&str>) -> io::Result<()> {
    let source = source_pane.ok_or_else(|| io::Error::other("gx needs a Herdr source pane"))?;
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or_else(|| io::Error::other("HOME and XDG_DATA_HOME are not set"))?;
    let binary = data.join("herdr/fingers-open/runtime/herdr-fingers");
    let mut child = Command::new(&binary)
        .arg("open-selection")
        // Fingers uses the first geometry field as the original pane and
        // HERDR_PANE_ID as the overlay to wait for before focusing Neovim.
        .env("HERDR_FINGERS_GEOMETRY", source)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| io::Error::other(format!("cannot start {}: {error}", binary.display())))?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let written = stdin.write_all(text.as_bytes());
    drop(stdin);
    let status = child.wait()?;
    written?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "Fingers exited with {status}; see its notification"
        )));
    }
    Ok(())
}
