//! Guard against launching copy-search from inside its own overlay.
//!
//! The overlay is a full-window TUI. Pressing the herdr trigger key
//! while an overlay is already up asks herdr to open a second overlay
//! whose source pane is the first overlay - a nested launch that stacks
//! copy-search on top of itself.
//!
//! Each live instance drops a marker file, named after its own overlay
//! pane id (HERDR_PANE_ID), into a runtime registry dir. A new instance
//! whose source pane matches a live marker is nested and exits silently.
//! Markers are removed on normal return or panic unwind (RegistryGuard);
//! markers left by a killed process (dead PID) are reaped on inspection.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Registry directory for active-overlay markers:
/// `${XDG_RUNTIME_DIR:-<temp_dir>}/herdr-copy-search/panes`.
pub fn registry_dir(env: impl Fn(&str) -> Option<String>) -> PathBuf {
    let base = env("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("herdr-copy-search").join("panes")
}

/// Marker file path for a pane id. herdr pane ids are plain tokens like
/// `w8:p1`; non-alphanumeric chars are mapped to `_` so the name is
/// filesystem-safe.
fn marker_path(dir: &Path, pane_id: &str) -> PathBuf {
    let name: String = pane_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    dir.join(name)
}

/// Is `pid` a live process? Uses `kill -0`, portable across Unix; any
/// failure (dead, out of range, or no `kill`) is treated as not alive.
fn pid_alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// True if `pane_id` has a live marker in the registry. A marker whose
/// PID is dead is stale: it is removed and reported as absent.
pub fn is_active_overlay(dir: &Path, pane_id: &str) -> bool {
    let path = marker_path(dir, pane_id);
    let Ok(pid) = std::fs::read_to_string(&path) else {
        return false;
    };
    if pid_alive(pid.trim()) {
        return true;
    }
    let _ = std::fs::remove_file(&path);
    false
}

/// A launch is nested when its source pane is itself a live overlay.
pub fn is_nested(source_pane: Option<&str>, is_active: impl Fn(&str) -> bool) -> bool {
    source_pane.is_some_and(is_active)
}

/// Register this instance's overlay pane as active, returning a guard
/// that removes the marker on drop. None when the marker cannot be
/// written (registry unusable) - nesting is then simply unguarded
/// rather than blocking a legitimate launch.
pub fn register(dir: &Path, self_pane_id: &str, pid: u32) -> Option<RegistryGuard> {
    std::fs::create_dir_all(dir).ok()?;
    let path = marker_path(dir, self_pane_id);
    std::fs::write(&path, pid.to_string()).ok()?;
    Some(RegistryGuard { path })
}

/// Removes its registry marker on drop (normal return or panic unwind).
/// A killed process skips Drop; those stale markers are reaped by
/// is_active_overlay's dead-PID check.
pub struct RegistryGuard {
    path: PathBuf,
}

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_nested_only_when_source_is_a_live_overlay() {
        assert!(!is_nested(None, |_| true), "no source pane");
        assert!(!is_nested(Some("w1:p1"), |_| false), "source not active");
        assert!(is_nested(Some("w1:p1"), |p| p == "w1:p1"), "source active");
    }

    #[test]
    fn registry_dir_prefers_xdg_then_temp() {
        let d = registry_dir(|k| (k == "XDG_RUNTIME_DIR").then(|| "/run/user/x".to_string()));
        assert_eq!(d, PathBuf::from("/run/user/x/herdr-copy-search/panes"));
        let fallback = registry_dir(|_| None);
        assert!(fallback.starts_with(std::env::temp_dir()));
        assert!(fallback.ends_with("herdr-copy-search/panes"));
    }

    #[test]
    fn register_marks_active_then_guard_drop_clears() {
        let dir = std::env::temp_dir().join("hcs-reg-active");
        let _ = std::fs::remove_dir_all(&dir);
        let guard = register(&dir, "w9:p9", std::process::id()).expect("register writes marker");
        assert!(is_active_overlay(&dir, "w9:p9"), "own pid is alive");
        drop(guard);
        assert!(!is_active_overlay(&dir, "w9:p9"), "marker removed on drop");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_marker_with_dead_pid_is_reaped() {
        let dir = std::env::temp_dir().join("hcs-reg-stale");
        std::fs::create_dir_all(&dir).unwrap();
        let path = marker_path(&dir, "w0:p0");
        // An implausible pid (above the usual pid_max) that no live
        // process can hold.
        std::fs::write(&path, "2147483646").unwrap();
        assert!(
            !is_active_overlay(&dir, "w0:p0"),
            "dead pid reads as absent"
        );
        assert!(!path.exists(), "stale marker reaped");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
