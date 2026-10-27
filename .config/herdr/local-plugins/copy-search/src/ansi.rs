use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    #[default]
    Default,
    /// 0-15 from classic SGR (30-37/90-97), 0-255 from 38;5;N.
    Indexed(u8),
    /// 24-bit from 38;2;r;g;b.
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

pub struct StyledLine {
    pub chars: Vec<char>,
    /// Parallel to `chars`; empty means every char is default-styled.
    pub styles: Vec<Style>,
}

/// Parse SGR-styled text into plain lines with per-char styles.
///
/// SGR state persists across newlines (herdr re-emits per line, but
/// carry-over input is also handled). Tabs expand to 4 spaced cells
/// carrying the current style, '\r' is dropped, and non-SGR escapes
/// (other CSI, OSC, charset selects) are skipped, never rendered.
/// A trailing '\n' does not produce a final empty line (str::lines
/// parity with Buffer::from_text).
pub fn parse(text: &str) -> Vec<StyledLine> {
    let mut out = Vec::new();
    let mut chars = Vec::new();
    let mut styles = Vec::new();
    let mut style = Style::default();
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\x1b' => consume_escape(&mut it, &mut style),
            '\n' => finish_line(&mut out, &mut chars, &mut styles),
            '\r' => {}
            '\t' => {
                for _ in 0..4 {
                    chars.push(' ');
                    styles.push(style);
                }
            }
            c => {
                chars.push(c);
                styles.push(style);
            }
        }
    }
    if !chars.is_empty() {
        finish_line(&mut out, &mut chars, &mut styles);
    }
    out
}

fn finish_line(out: &mut Vec<StyledLine>, chars: &mut Vec<char>, styles: &mut Vec<Style>) {
    let styles = if styles.iter().all(|s| *s == Style::default()) {
        styles.clear();
        Vec::new()
    } else {
        std::mem::take(styles)
    };
    out.push(StyledLine {
        chars: std::mem::take(chars),
        styles,
    });
}

/// Consume one escape sequence (the ESC is already eaten). Only SGR
/// (CSI ... m) mutates the style; everything else is skipped safely.
fn consume_escape(it: &mut Peekable<Chars>, style: &mut Style) {
    match it.next() {
        Some('[') => {
            let mut params = String::new();
            for c in it.by_ref() {
                if ('\x40'..='\x7e').contains(&c) {
                    if c == 'm' {
                        apply_sgr(style, &params);
                    }
                    return;
                }
                params.push(c);
            }
        }
        Some(']') => {
            // OSC: terminated by BEL or ST (ESC \).
            while let Some(c) = it.next() {
                if c == '\x07' {
                    return;
                }
                if c == '\x1b' {
                    if it.peek() == Some(&'\\') {
                        it.next();
                    }
                    return;
                }
            }
        }
        Some(c) if ('\x20'..='\x2f').contains(&c) => {
            // Intermediate escape like ESC ( B: skip to the final byte.
            for c in it.by_ref() {
                if ('\x30'..='\x7e').contains(&c) {
                    return;
                }
            }
        }
        // Two-byte escapes (ESC =, ESC >, ...) or EOF: already consumed.
        _ => {}
    }
}

/// Apply a semicolon-separated SGR parameter list. Unknown or
/// unparsable chunks (including colon sub-parameters like 38:5:N) are
/// ignored, degrading to "unstyled" rather than corrupting output.
fn apply_sgr(style: &mut Style, params: &str) {
    let parts: Vec<&str> = params.split(';').collect();
    let mut i = 0;
    while i < parts.len() {
        let n = if parts[i].is_empty() {
            0 // an empty parameter means reset, including lone "ESC[m"
        } else {
            match parts[i].parse::<u16>() {
                Ok(n) => n,
                Err(_) => {
                    i += 1;
                    continue;
                }
            }
        };
        match n {
            0 => *style = Style::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            7 => style.reverse = true,
            22 => {
                style.bold = false;
                style.dim = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            27 => style.reverse = false,
            30..=37 => style.fg = Color::Indexed((n - 30) as u8),
            90..=97 => style.fg = Color::Indexed((n - 90 + 8) as u8),
            40..=47 => style.bg = Color::Indexed((n - 40) as u8),
            100..=107 => style.bg = Color::Indexed((n - 100 + 8) as u8),
            39 => style.fg = Color::Default,
            49 => style.bg = Color::Default,
            38 | 48 => {
                let set = |style: &mut Style, color: Color| {
                    if n == 38 {
                        style.fg = color;
                    } else {
                        style.bg = color;
                    }
                };
                match parts.get(i + 1).copied() {
                    Some("5") => {
                        if let Some(Ok(idx)) = parts.get(i + 2).map(|s| s.parse::<u8>()) {
                            set(style, Color::Indexed(idx));
                        }
                        i += 2;
                    }
                    Some("2") => {
                        let rgb = (
                            parts.get(i + 2).and_then(|s| s.parse::<u8>().ok()),
                            parts.get(i + 3).and_then(|s| s.parse::<u8>().ok()),
                            parts.get(i + 4).and_then(|s| s.parse::<u8>().ok()),
                        );
                        if let (Some(r), Some(g), Some(b)) = rgb {
                            set(style, Color::Rgb(r, g, b));
                        }
                        i += 4;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fg(n: u8) -> Style {
        Style {
            fg: Color::Indexed(n),
            ..Style::default()
        }
    }

    fn text_of(line: &StyledLine) -> String {
        line.chars.iter().collect()
    }

    #[test]
    fn plain_text_has_empty_styles() {
        let lines = parse("hello\nworld");
        assert_eq!(lines.len(), 2);
        assert_eq!(text_of(&lines[0]), "hello");
        assert!(lines[0].styles.is_empty());
        assert!(lines[1].styles.is_empty());
    }

    #[test]
    fn sixteen_color_and_reset() {
        let lines = parse("\x1b[31mred\x1b[0m plain");
        let l = &lines[0];
        assert_eq!(text_of(l), "red plain");
        assert_eq!(l.styles[0], fg(1));
        assert_eq!(l.styles[2], fg(1));
        assert_eq!(l.styles[3], Style::default());
    }

    #[test]
    fn bright_and_background_colors() {
        let lines = parse("\x1b[91;44mx");
        assert_eq!(
            lines[0].styles[0],
            Style {
                fg: Color::Indexed(9),
                bg: Color::Indexed(4),
                ..Style::default()
            }
        );
    }

    #[test]
    fn indexed_256_and_rgb() {
        let lines = parse("\x1b[38;5;215ma\x1b[48;2;10;20;30mb");
        assert_eq!(lines[0].styles[0].fg, Color::Indexed(215));
        assert_eq!(lines[0].styles[1].fg, Color::Indexed(215));
        assert_eq!(lines[0].styles[1].bg, Color::Rgb(10, 20, 30));
    }

    #[test]
    fn multi_param_and_bare_reset() {
        let lines = parse("\x1b[1;31ma\x1b[mb");
        assert_eq!(
            lines[0].styles[0],
            Style {
                fg: Color::Indexed(1),
                bold: true,
                ..Style::default()
            }
        );
        assert_eq!(lines[0].styles[1], Style::default());
    }

    #[test]
    fn attribute_set_and_clear() {
        let lines = parse("\x1b[1;2;3;4;7ma\x1b[22;23;24;27mb\x1b[39;49mc");
        let a = lines[0].styles[0];
        assert!(a.bold && a.dim && a.italic && a.underline && a.reverse);
        assert_eq!(lines[0].styles[1], Style::default());
    }

    #[test]
    fn style_persists_across_lines() {
        let lines = parse("\x1b[31mone\ntwo\x1b[0m");
        assert_eq!(lines[1].styles[0], fg(1));
    }

    #[test]
    fn tab_expands_to_styled_spaces() {
        let lines = parse("\x1b[31m\ta");
        assert_eq!(text_of(&lines[0]), "    a");
        assert_eq!(lines[0].styles[0], fg(1));
        assert_eq!(lines[0].styles[3], fg(1));
    }

    #[test]
    fn carriage_returns_are_dropped() {
        let lines = parse("one\r\ntwo\r");
        assert_eq!(text_of(&lines[0]), "one");
        assert_eq!(text_of(&lines[1]), "two");
    }

    #[test]
    fn trailing_newline_adds_no_line() {
        assert_eq!(parse("a\nb\n").len(), 2);
        assert_eq!(parse("").len(), 0);
    }

    #[test]
    fn non_sgr_sequences_are_skipped() {
        let lines = parse("a\x1b[2Jb\x1b]0;title\x07c\x1b]52;c;xyz\x1b\\d\x1b(Be\x1b=f");
        assert_eq!(text_of(&lines[0]), "abcdef");
        assert!(lines[0].styles.is_empty());
    }

    #[test]
    fn colon_subparams_degrade_to_unstyled() {
        let lines = parse("\x1b[38:5:196mx");
        assert_eq!(text_of(&lines[0]), "x");
        assert!(lines[0].styles.is_empty());
    }
}
