use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use std::fs::OpenOptions;
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

/// Many terminals cap OSC 52 payloads around 100 KB of base64; larger
/// writes may be truncated or dropped by the outer terminal.
pub const SOFT_LIMIT: usize = 100_000;

/// Time to let herdr forward the OSC 52 write to the outer terminal
/// before the emitting pane exits.
pub const FORWARD_GRACE: Duration = Duration::from_millis(200);

fn encode(text: &str) -> String {
    STANDARD.encode(text.as_bytes())
}

pub fn is_oversize(text: &str) -> bool {
    encode(text).len() > SOFT_LIMIT
}

/// OSC 52 set-clipboard sequence for the `c` (clipboard) selection.
pub fn sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", encode(text))
}

/// Write the sequence to the controlling terminal and flush. Returns
/// true when the payload exceeds SOFT_LIMIT so the caller can warn.
pub fn emit(text: &str) -> io::Result<bool> {
    let seq = sequence(text);
    match OpenOptions::new().write(true).open("/dev/tty") {
        Ok(mut tty) => {
            tty.write_all(seq.as_bytes())?;
            tty.flush()?;
        }
        Err(_) => {
            let mut out = io::stdout();
            if !out.is_terminal() {
                return Err(io::Error::other("no terminal to receive OSC 52"));
            }
            out.write_all(seq.as_bytes())?;
            out.flush()?;
        }
    }
    Ok(is_oversize(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_wraps_base64_payload() {
        assert_eq!(sequence("hello"), "\x1b]52;c;aGVsbG8=\x07");
    }

    #[test]
    fn sequence_of_empty_text_is_valid() {
        assert_eq!(sequence(""), "\x1b]52;c;\x07");
    }

    #[test]
    fn oversize_flags_large_payloads_only() {
        assert!(!is_oversize("small"));
        // 80_000 raw bytes encode to ~106_667 base64 bytes.
        let big = "x".repeat(80_000);
        assert!(is_oversize(&big));
    }
}
