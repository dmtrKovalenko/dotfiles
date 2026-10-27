//! What the use cases need from the world, as traits the adapters implement.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::geometry::{Layout, OverlayGeometry};
use crate::domain::screen::Screen;
use crate::domain::session::{Outcome, Session};
use crate::domain::settings::Theme;

/// An adapter failed; the message is already fit for a human.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct PortError(pub String);

impl PortError {
    pub fn new(message: impl std::fmt::Display) -> Self {
        PortError(message.to_string())
    }
}

/// The multiplexer the panes live in.
pub trait PaneHost {
    /// The tab layout around `pane_id`.
    fn layout(&self, pane_id: &str) -> Result<Layout, PortError>;
    /// The pane's visible screen, ANSI styling included.
    fn read_visible(&self, pane_id: &str) -> Result<String, PortError>;
    /// The label the host shows for the pane, if any.
    fn pane_label(&self, pane_id: &str) -> Result<Option<String>, PortError>;
    /// The directory the pane's foreground process runs in.
    fn pane_cwd(&self, pane_id: &str) -> Result<Option<PathBuf>, PortError>;
    /// Opens one of this plugin's declared panes; returns the new pane id.
    fn open_overlay(
        &self,
        plugin_id: &str,
        entrypoint: &str,
        env: BTreeMap<String, String>,
    ) -> Result<String, PortError>;
    /// Types `text` into the pane as if the user had.
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), PortError>;
    /// Shows a toast.
    fn notify(&self, title: &str, body: &str) -> Result<(), PortError>;
}

/// Where `:copy:` puts the text.
pub trait Clipboard {
    fn copy(&mut self, text: &str) -> Result<(), PortError>;
}

/// Opens things and runs the user's own commands.
pub trait Launcher {
    /// Opens a URL or file with whatever the system uses for it.
    fn open(&self, target: &str, cwd: Option<&Path>) -> Result<(), PortError>;
    /// Runs a command line with `stdin` on its standard input and `env` added
    /// to its environment; a non-zero exit is an error.
    fn run(
        &self,
        command_line: &str,
        stdin: &str,
        env: &[(&str, String)],
        cwd: Option<&Path>,
    ) -> Result<(), PortError>;
}

/// Everything the picker shows besides the session it drives.
pub struct PickView<'a> {
    pub screen: &'a Screen,
    pub theme: &'a Theme,
    pub geometry: Option<&'a OverlayGeometry>,
    /// A one-line notice (configuration problem…).
    pub notice: Option<&'a str>,
}

/// The interactive part: shows the hints, feeds keys to the session, and
/// returns when the user has picked or given up.
pub trait Picker {
    fn pick(&mut self, view: &PickView<'_>, session: &mut Session) -> Result<Outcome, PortError>;
}
