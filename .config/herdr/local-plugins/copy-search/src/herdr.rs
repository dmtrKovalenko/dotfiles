use crate::buffer::Buffer;
use std::io;
use std::path::PathBuf;
use std::process::Command;

/// Thin wrapper over the herdr CLI, which is the plugin API surface.
pub struct Herdr {
    bin: PathBuf,
}

impl Herdr {
    pub fn from_env() -> Self {
        let bin = std::env::var_os("HERDR_BIN_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("herdr"));
        Herdr { bin }
    }

    /// ANSI reads are passive terminal snapshots. A plain-text recent read
    /// can make Herdr scroll an idle full-screen agent to harvest history,
    /// taking up to 20 seconds and sending input to the source application.
    fn read_ansi(&self, pane: &str, lines: usize, source: &str) -> io::Result<String> {
        let out = Command::new(&self.bin)
            .args([
                "pane",
                "read",
                pane,
                "--source",
                source,
                "--lines",
                &lines.to_string(),
                "--format",
                "ansi",
            ])
            .output()?;
        if !out.status.success() {
            return Err(io::Error::other(format!(
                "herdr pane read {pane} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        String::from_utf8(out.stdout).map_err(io::Error::other)
    }

    /// Convert a passive unwrapped ANSI snapshot to logical plain text.
    /// Never request recent history in text format: it can scroll agents.
    pub fn pane_read(&self, pane: &str, lines: usize) -> io::Result<String> {
        let ansi = self.read_ansi(pane, lines, "recent-unwrapped")?;
        Ok(Buffer::from_ansi(&ansi).text())
    }

    /// Recent scrollback as SGR-styled rows wrapped at the pane width.
    /// (recent-unwrapped + ansi is unreliable server-side: it returns
    /// empty once the scrollback grows, so styled reads use `recent`.)
    pub fn pane_read_ansi(&self, pane: &str, lines: usize) -> io::Result<String> {
        self.read_ansi(pane, lines, "recent")
    }

    /// The pane's current screen as plain text rows (wrapped at the
    /// pane width; the server trims trailing blank rows).
    pub fn pane_read_visible(&self, pane: &str, lines: usize) -> io::Result<String> {
        let ansi = self.read_ansi(pane, lines, "visible")?;
        Ok(Buffer::from_ansi(&ansi).text())
    }

    /// Type `text` into a pane, as if entered at its terminal.
    pub fn send_text(&self, pane: &str, text: &str) -> io::Result<()> {
        let out = Command::new(&self.bin)
            .args(["pane", "send-text", pane, text])
            .output()?;
        if !out.status.success() {
            return Err(io::Error::other(format!(
                "herdr pane send-text {pane} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(())
    }
}

/// The pane whose scrollback we operate on. Spike-verified order:
/// explicit --pane, then HERDR_ACTIVE_PANE_ID (set for keys.command
/// bindings), then the context JSON's focused_pane_id (set for plugin
/// panes and actions). HERDR_PANE_ID is the plugin pane itself, never
/// the source, so it is deliberately not consulted.
pub fn resolve_source_pane(
    cli: Option<&str>,
    env: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    if let Some(pane) = cli {
        return Some(pane.to_string());
    }
    if let Some(pane) = env("HERDR_ACTIVE_PANE_ID") {
        return Some(pane);
    }
    env("HERDR_PLUGIN_CONTEXT_JSON").and_then(|json| json_str_field(&json, "focused_pane_id"))
}

/// Minimal `"key":"value"` extractor for herdr's flat context JSON.
/// Values with escaped quotes are not handled; pane and workspace ids
/// are plain tokens like "w8:p1".
pub fn json_str_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let rest = &json[json.find(&needle)? + needle.len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTX: &str = r#"{"workspace_id":"w8","tab_id":"w8:t1","focused_pane_id":"w8:p1","focused_pane_cwd":"/home/x","invocation_source":"api"}"#;

    #[test]
    fn json_str_field_extracts_flat_fields() {
        assert_eq!(
            json_str_field(CTX, "focused_pane_id").as_deref(),
            Some("w8:p1")
        );
        assert_eq!(json_str_field(CTX, "workspace_id").as_deref(), Some("w8"));
        assert_eq!(json_str_field(CTX, "missing"), None);
    }

    #[test]
    fn json_str_field_allows_spacing() {
        let json = r#"{ "focused_pane_id" : "w1:p2" }"#;
        assert_eq!(
            json_str_field(json, "focused_pane_id").as_deref(),
            Some("w1:p2")
        );
    }

    #[test]
    fn cli_pane_wins_over_env() {
        let env = |_: &str| Some("env-pane".to_string());
        assert_eq!(
            resolve_source_pane(Some("cli-pane"), env).as_deref(),
            Some("cli-pane")
        );
    }

    #[test]
    fn active_pane_id_wins_over_context_json() {
        let env = |k: &str| match k {
            "HERDR_ACTIVE_PANE_ID" => Some("w8:pA".to_string()),
            "HERDR_PLUGIN_CONTEXT_JSON" => Some(CTX.to_string()),
            _ => None,
        };
        assert_eq!(resolve_source_pane(None, env).as_deref(), Some("w8:pA"));
    }

    #[test]
    fn context_json_is_the_fallback() {
        let env = |k: &str| match k {
            "HERDR_PLUGIN_CONTEXT_JSON" => Some(CTX.to_string()),
            _ => None,
        };
        assert_eq!(resolve_source_pane(None, env).as_deref(), Some("w8:p1"));
    }

    #[test]
    fn no_sources_resolves_to_none() {
        assert_eq!(resolve_source_pane(None, |_| None), None);
    }
}
