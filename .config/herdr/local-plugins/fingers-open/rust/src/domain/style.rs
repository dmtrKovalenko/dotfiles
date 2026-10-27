//! Colors and text attributes shared by the captured screen and the theme.

use thiserror::Error;

/// A terminal color: a palette index (`0`–`255`) or a truecolor triple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error(
    "unknown color `{0}`: use a name like `yellow` or `light-blue`, a palette index (0-255) or `#RRGGBB`"
)]
pub struct ColorParseError(pub String);

const NAMED_COLORS: &[(&str, u8)] = &[
    ("black", 0),
    ("red", 1),
    ("green", 2),
    ("yellow", 3),
    ("blue", 4),
    ("magenta", 5),
    ("cyan", 6),
    ("white", 7),
    ("light-gray", 7),
    ("light-grey", 7),
    ("gray", 8),
    ("grey", 8),
    ("dark-gray", 8),
    ("dark-grey", 8),
    ("bright-black", 8),
    ("light-red", 9),
    ("bright-red", 9),
    ("light-green", 10),
    ("bright-green", 10),
    ("light-yellow", 11),
    ("bright-yellow", 11),
    ("light-blue", 12),
    ("bright-blue", 12),
    ("light-magenta", 13),
    ("bright-magenta", 13),
    ("light-cyan", 14),
    ("bright-cyan", 14),
    ("bright-white", 15),
];

impl Color {
    /// Parses a color name (`yellow`, `light-blue`, `gray`), a palette index
    /// (`0`–`255`) or a hex triple (`#RRGGBB`). Case-insensitive.
    pub fn parse(input: &str) -> Result<Color, ColorParseError> {
        let trimmed = input.trim().to_ascii_lowercase();
        if let Some(&(_, index)) = NAMED_COLORS.iter().find(|(name, _)| *name == trimmed) {
            return Ok(Color::Indexed(index));
        }
        if let Ok(index) = trimmed.parse::<u8>() {
            return Ok(Color::Indexed(index));
        }
        if let Some(hex) = trimmed.strip_prefix('#')
            && hex.len() == 6
            && let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            )
        {
            return Ok(Color::Rgb(r, g, b));
        }
        Err(ColorParseError(input.trim().to_string()))
    }
}

/// Foreground, background and attributes of one screen cell or theme entry.
/// `None` means "leave whatever is underneath".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct TextStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
    pub strikethrough: bool,
}

impl TextStyle {
    pub const PLAIN: TextStyle = TextStyle {
        fg: None,
        bg: None,
        bold: false,
        dim: false,
        italic: false,
        underline: false,
        reverse: false,
        strikethrough: false,
    };

    pub const fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    pub const fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    /// `self` painted over `base`: colors set here win, attributes accumulate.
    pub fn over(self, base: TextStyle) -> TextStyle {
        TextStyle {
            fg: self.fg.or(base.fg),
            bg: self.bg.or(base.bg),
            bold: self.bold || base.bold,
            dim: self.dim || base.dim,
            italic: self.italic || base.italic,
            underline: self.underline || base.underline,
            reverse: self.reverse || base.reverse,
            strikethrough: self.strikethrough || base.strikethrough,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_colors_map_to_the_ansi_palette() {
        assert_eq!(Color::parse("yellow"), Ok(Color::Indexed(3)));
        assert_eq!(Color::parse("Light-Blue"), Ok(Color::Indexed(12)));
        assert_eq!(Color::parse("gray"), Ok(Color::Indexed(8)));
        assert_eq!(Color::parse("white"), Ok(Color::Indexed(7)));
    }

    #[test]
    fn palette_indexes_and_hex_triples_are_accepted() {
        assert_eq!(Color::parse("208"), Ok(Color::Indexed(208)));
        assert_eq!(Color::parse("#FF8800"), Ok(Color::Rgb(255, 136, 0)));
        assert_eq!(Color::parse(" #00ff00 "), Ok(Color::Rgb(0, 255, 0)));
    }

    #[test]
    fn garbage_is_rejected_with_the_offending_text() {
        assert_eq!(
            Color::parse("chartreuse"),
            Err(ColorParseError("chartreuse".into()))
        );
        assert!(Color::parse("#12345").is_err());
        assert!(Color::parse("256").is_err());
    }

    #[test]
    fn painting_a_style_over_another_keeps_the_base_where_unset() {
        let base = TextStyle::PLAIN.fg(Color::Indexed(1)).bg(Color::Indexed(2));
        let top = TextStyle::PLAIN.fg(Color::Indexed(3)).bold();
        let merged = top.over(base);
        assert_eq!(merged.fg, Some(Color::Indexed(3)));
        assert_eq!(merged.bg, Some(Color::Indexed(2)));
        assert!(merged.bold);
    }
}
