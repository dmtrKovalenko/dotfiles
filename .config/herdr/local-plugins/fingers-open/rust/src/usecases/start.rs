//! `start`: the plugin action. Records where the focused pane sits, then
//! asks the host to open the overlay over it.

use std::collections::BTreeMap;

use thiserror::Error;

use super::ports::{PaneHost, PortError};
use crate::domain::geometry::{GeometryError, OverlayGeometry};
use crate::domain::patterns::{self, PatternError};

/// Environment variable carrying the overlay geometry from `start` to `ui`.
pub const GEOMETRY_ENV: &str = "HERDR_FINGERS_GEOMETRY";
/// Environment variable carrying a comma-separated pattern subset.
pub const PATTERNS_ENV: &str = "HERDR_FINGERS_PATTERNS";
/// The pane title declared in `herdr-plugin.toml` for the overlay.
pub const OVERLAY_TITLE: &str = "Fingers";
/// The pane entrypoint declared in `herdr-plugin.toml` for the overlay.
pub const OVERLAY_ENTRYPOINT: &str = "overlay";

#[derive(Debug, Error)]
pub enum StartError {
    #[error(transparent)]
    Host(#[from] PortError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error(transparent)]
    Pattern(#[from] PatternError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Started {
    /// The overlay pane that was opened.
    Opened(String),
    /// The focused pane already is an overlay; nothing to do.
    AlreadyOpen,
}

/// Opens the overlay over `pane_id`, restricted to `patterns` when given.
pub fn start(
    host: &dyn PaneHost,
    plugin_id: &str,
    pane_id: &str,
    patterns: Option<&[&str]>,
) -> Result<Started, StartError> {
    if host.pane_label(pane_id)?.as_deref() == Some(OVERLAY_TITLE) {
        return Ok(Started::AlreadyOpen);
    }
    let layout = host.layout(pane_id)?;
    let geometry = OverlayGeometry::locate(&layout, pane_id)?;
    let mut env = BTreeMap::new();
    env.insert(GEOMETRY_ENV.to_string(), geometry.encode());
    if let Some(names) = patterns {
        patterns::builtin_specs(names)?;
        env.insert(PATTERNS_ENV.to_string(), names.join(","));
    }
    let overlay = host.open_overlay(plugin_id, OVERLAY_ENTRYPOINT, env)?;
    Ok(Started::Opened(overlay))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Rect;
    use crate::usecases::testing::FakeHost;

    #[test]
    fn the_overlay_opens_with_the_focused_pane_geometry_in_its_environment() {
        let host = FakeHost::with_split_layout("w1:p2");
        let started = start(&host, "nathan-poncet.herdr-fingers", "w1:p2", None).unwrap();
        assert_eq!(started, Started::Opened("w1:p9".into()));
        let opened = host.opened.borrow();
        let (plugin, entrypoint, env) = &opened[0];
        assert_eq!(plugin, "nathan-poncet.herdr-fingers");
        assert_eq!(entrypoint, OVERLAY_ENTRYPOINT);
        let geometry = OverlayGeometry::decode(&env[GEOMETRY_ENV]).unwrap();
        assert_eq!(geometry.pane_id, "w1:p2");
        assert_eq!(geometry.pane, Rect::new(51, 0, 49, 40));
        assert!(!env.contains_key(PATTERNS_ENV));
    }

    #[test]
    fn nothing_happens_when_the_focused_pane_is_already_the_overlay() {
        let host = FakeHost::with_split_layout("w1:p2").labelled(OVERLAY_TITLE);
        assert_eq!(
            start(&host, "id", "w1:p2", None).unwrap(),
            Started::AlreadyOpen
        );
        assert!(host.opened.borrow().is_empty());
    }

    #[test]
    fn a_pattern_subset_travels_along_and_unknown_names_are_refused() {
        let host = FakeHost::with_split_layout("w1:p2");
        start(&host, "id", "w1:p2", Some(&["url", "path"])).unwrap();
        assert_eq!(host.opened.borrow()[0].2[PATTERNS_ENV], "url,path");

        let error = start(&host, "id", "w1:p2", Some(&["nope"])).unwrap_err();
        assert!(matches!(error, StartError::Pattern(_)));
        assert_eq!(host.opened.borrow().len(), 1);
    }

    #[test]
    fn a_pane_missing_from_the_layout_is_an_error() {
        let host = FakeHost::with_split_layout("w1:p2");
        let error = start(&host, "id", "w9:p9", None).unwrap_err();
        assert!(matches!(error, StartError::Geometry(_)));
    }
}
