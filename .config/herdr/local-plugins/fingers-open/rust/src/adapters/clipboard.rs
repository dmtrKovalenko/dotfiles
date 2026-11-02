//! Getting text onto the clipboard: OSC 52 through Herdr, or a local command.

use std::io::Write;
use std::process::{Command, Stdio};

use base64::Engine;
use thiserror::Error;

use crate::domain::settings::ClipboardMode;
use crate::usecases::ports::{Clipboard, PortError};

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("cannot write to the terminal: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "no clipboard command found (looked for pbcopy, wl-copy, xclip, xsel); set clipboard_command"
    )]
    NoCommand,
    #[error("clipboard command `{0}` failed")]
    CommandFailed(String),
}

/// The clipboard the settings ask for: OSC 52, a local command, or both.
#[derive(Debug, Clone)]
pub struct SystemClipboard {
    mode: ClipboardMode,
    command: Vec<String>,
}

impl SystemClipboard {
    pub fn new(mode: ClipboardMode, command: Vec<String>) -> Self {
        SystemClipboard { mode, command }
    }
}

impl Clipboard for SystemClipboard {
    fn copy(&mut self, text: &str) -> Result<(), PortError> {
        copy(text, self.mode, &self.command).map_err(PortError::new)
    }
}

/// The OSC 52 sequence that asks the terminal to set its clipboard.
pub fn osc52_sequence(text: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    format!("\x1b]52;c;{encoded}\x07")
}

/// Copies `text` the way the settings ask for.
pub fn copy(text: &str, mode: ClipboardMode, command: &[String]) -> Result<(), ClipboardError> {
    match mode {
        ClipboardMode::Osc52 => write_osc52(text),
        ClipboardMode::System => run_command(text, command),
        ClipboardMode::Both => {
            write_osc52(text)?;
            run_command(text, command)
        }
    }
}

fn write_osc52(text: &str) -> Result<(), ClipboardError> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(osc52_sequence(text).as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn run_command(text: &str, command: &[String]) -> Result<(), ClipboardError> {
    let argv = if command.is_empty() {
        detect_command().ok_or(ClipboardError::NoCommand)?
    } else {
        command.to_vec()
    };
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ClipboardError::CommandFailed(argv.join(" ")))?;
    if let Some(mut stdin) = child.stdin.take() {
        // A command that exits early closes the pipe; its exit status is the
        // verdict, not the broken pipe.
        let _ = stdin.write_all(text.as_bytes());
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(ClipboardError::CommandFailed(argv.join(" ")))
    }
}

/// The first clipboard tool present on this machine.
pub fn detect_command() -> Option<Vec<String>> {
    let candidates: &[&[&str]] = &[
        &["pbcopy"],
        &["wl-copy"],
        &["xclip", "-selection", "clipboard"],
        &["xsel", "--clipboard", "--input"],
    ];
    candidates
        .iter()
        .find(|argv| command_exists(argv[0]))
        .map(|argv| argv.iter().map(|s| s.to_string()).collect())
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_osc52_sequence_is_base64_and_bel_terminated() {
        assert_eq!(osc52_sequence("hello"), "\x1b]52;c;aGVsbG8=\x07");
    }

    #[test]
    fn an_explicit_command_receives_the_text_on_stdin() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let target = std::env::temp_dir().join(format!("herdr-fingers-clip-{unique}"));
        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("cat > '{}'", target.display()),
        ];
        copy("copied text", ClipboardMode::System, &command).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "copied text");
        let _ = std::fs::remove_file(target);
    }

    #[test]
    fn a_failing_command_is_reported_by_name() {
        let command = vec!["sh".to_string(), "-c".to_string(), "exit 3".to_string()];
        let error = copy("x", ClipboardMode::System, &command).unwrap_err();
        assert!(matches!(error, ClipboardError::CommandFailed(name) if name.starts_with("sh -c")));
    }

    #[test]
    fn the_port_implementation_uses_the_configured_command() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let target = std::env::temp_dir().join(format!("herdr-fingers-port-{unique}"));
        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("cat > '{}'", target.display()),
        ];
        let mut clipboard = SystemClipboard::new(ClipboardMode::System, command);
        clipboard.copy("via port").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "via port");
        let _ = std::fs::remove_file(target);
    }

    #[test]
    fn a_missing_command_is_reported_by_name() {
        let command = vec!["definitely-not-a-clipboard-tool".to_string()];
        let error = copy("x", ClipboardMode::System, &command).unwrap_err();
        assert!(matches!(error, ClipboardError::CommandFailed(_)));
    }
}
