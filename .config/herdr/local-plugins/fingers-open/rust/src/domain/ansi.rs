//! Turns the ANSI-styled text Herdr returns for a pane into rows of cells,
//! keeping the colors so the overlay looks like the pane it covers.

use unicode_width::UnicodeWidthChar;

use super::style::{Color, TextStyle};

/// One terminal cell: a grapheme, the columns it occupies, and its style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub width: u8,
    pub style: TextStyle,
}

impl Cell {
    pub fn is_blank(&self) -> bool {
        self.text.chars().all(char::is_whitespace)
    }
}

/// One screen row, left to right.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<Cell>,
}

impl Row {
    pub fn display_width(&self) -> usize {
        self.cells.iter().map(|cell| usize::from(cell.width)).sum()
    }

    pub fn text(&self) -> String {
        self.cells.iter().map(|cell| cell.text.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn ends_with_ink(&self) -> bool {
        self.cells.last().is_some_and(|cell| !cell.is_blank())
    }
}

const TAB_STOP: usize = 8;

/// Parses a screen dump: SGR sequences become styles, other escape sequences
/// are dropped, `\n` starts a new row and `\r` is ignored.
pub fn parse_rows(text: &str) -> Vec<Row> {
    let mut parser = Parser::default();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => parser.escape(&mut chars),
            '\n' => parser.rows.push(Row::default()),
            '\r' => {}
            '\t' => parser.tab(),
            ch if ch.is_control() => {}
            ch => parser.print(ch),
        }
    }
    if text.ends_with('\n') && parser.rows.last().is_some_and(Row::is_empty) {
        parser.rows.pop();
    }
    parser.rows
}

struct Parser {
    rows: Vec<Row>,
    style: TextStyle,
}

impl Default for Parser {
    fn default() -> Self {
        Parser {
            rows: vec![Row::default()],
            style: TextStyle::PLAIN,
        }
    }
}

impl Parser {
    fn current_row(&mut self) -> &mut Row {
        if self.rows.is_empty() {
            self.rows.push(Row::default());
        }
        self.rows.last_mut().expect("at least one row")
    }

    fn print(&mut self, ch: char) {
        let width = ch.width().unwrap_or(0);
        let style = self.style;
        let row = self.current_row();
        if width == 0 {
            if let Some(last) = row.cells.last_mut() {
                last.text.push(ch);
            }
            return;
        }
        row.cells.push(Cell {
            text: ch.to_string(),
            width: width.min(2) as u8,
            style,
        });
    }

    fn tab(&mut self) {
        let style = self.style;
        let row = self.current_row();
        let column = row.display_width();
        let spaces = TAB_STOP - (column % TAB_STOP);
        for _ in 0..spaces {
            row.cells.push(Cell {
                text: " ".to_string(),
                width: 1,
                style,
            });
        }
    }

    fn escape(&mut self, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
        match chars.next() {
            Some('[') => {
                let mut params = String::new();
                for ch in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&ch) {
                        if ch == 'm' {
                            self.apply_sgr(&params);
                        }
                        break;
                    }
                    params.push(ch);
                }
            }
            Some(']') | Some('P') | Some('_') | Some('^') | Some('X') => {
                skip_string_sequence(chars);
            }
            Some('(') | Some(')') | Some('*') | Some('+') | Some('#') | Some('%') => {
                chars.next();
            }
            _ => {}
        }
    }

    fn apply_sgr(&mut self, params: &str) {
        let codes: Vec<Option<u16>> = params
            .split([';', ':'])
            .map(|part| {
                if part.is_empty() {
                    Some(0)
                } else {
                    part.parse().ok()
                }
            })
            .collect();
        let mut index = 0;
        while index < codes.len() {
            let Some(code) = codes[index] else {
                index += 1;
                continue;
            };
            match code {
                0 => self.style = TextStyle::PLAIN,
                1 => self.style.bold = true,
                2 => self.style.dim = true,
                3 => self.style.italic = true,
                4 | 21 => self.style.underline = true,
                7 => self.style.reverse = true,
                9 => self.style.strikethrough = true,
                22 => {
                    self.style.bold = false;
                    self.style.dim = false;
                }
                23 => self.style.italic = false,
                24 => self.style.underline = false,
                27 => self.style.reverse = false,
                29 => self.style.strikethrough = false,
                30..=37 => self.style.fg = Some(Color::Indexed((code - 30) as u8)),
                90..=97 => self.style.fg = Some(Color::Indexed((code - 90 + 8) as u8)),
                39 => self.style.fg = None,
                40..=47 => self.style.bg = Some(Color::Indexed((code - 40) as u8)),
                100..=107 => self.style.bg = Some(Color::Indexed((code - 100 + 8) as u8)),
                49 => self.style.bg = None,
                38 | 48 => {
                    let (color, consumed) = extended_color(&codes[index + 1..]);
                    if code == 38 {
                        self.style.fg = color.or(self.style.fg);
                    } else {
                        self.style.bg = color.or(self.style.bg);
                    }
                    index += consumed;
                }
                _ => {}
            }
            index += 1;
        }
    }
}

/// Decodes the tail of a `38`/`48` sequence: `5;n` or `2;r;g;b`. Returns the
/// color and how many parameters were consumed.
fn extended_color(tail: &[Option<u16>]) -> (Option<Color>, usize) {
    match tail.first().copied().flatten() {
        Some(5) => {
            let index = tail
                .get(1)
                .copied()
                .flatten()
                .and_then(|n| u8::try_from(n).ok());
            (index.map(Color::Indexed), 2)
        }
        Some(2) => {
            let channel = |offset: usize| {
                tail.get(offset)
                    .copied()
                    .flatten()
                    .and_then(|n| u8::try_from(n).ok())
            };
            match (channel(1), channel(2), channel(3)) {
                (Some(r), Some(g), Some(b)) => (Some(Color::Rgb(r, g, b)), 4),
                _ => (None, 4),
            }
        }
        _ => (None, 0),
    }
}

fn skip_string_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(ch) = chars.next() {
        match ch {
            '\x07' => return,
            '\x1b' => {
                if chars.peek() == Some(&'\\') {
                    chars.next();
                }
                return;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> Vec<String> {
        parse_rows(text).iter().map(Row::text).collect()
    }

    #[test]
    fn plain_text_splits_into_rows_and_ignores_carriage_returns() {
        assert_eq!(plain("one\r\ntwo\nthree"), vec!["one", "two", "three"]);
    }

    #[test]
    fn a_trailing_newline_does_not_create_an_empty_row() {
        assert_eq!(plain("one\ntwo\n"), vec!["one", "two"]);
        assert_eq!(plain("one\n\n"), vec!["one", ""]);
    }

    #[test]
    fn sgr_sequences_style_the_following_cells() {
        let rows = parse_rows("\x1b[1;31mred\x1b[0m plain");
        let cells = &rows[0].cells;
        assert_eq!(cells[0].text, "r");
        assert_eq!(cells[0].style.fg, Some(Color::Indexed(1)));
        assert!(cells[0].style.bold);
        assert_eq!(cells[4].text, "p");
        assert_eq!(cells[4].style, TextStyle::PLAIN);
    }

    #[test]
    fn truecolor_and_palette_colors_are_decoded() {
        let rows = parse_rows("\x1b[38;2;215;119;87ma\x1b[48;5;236mb\x1b[39mc");
        let cells = &rows[0].cells;
        assert_eq!(cells[0].style.fg, Some(Color::Rgb(215, 119, 87)));
        assert_eq!(cells[1].style.bg, Some(Color::Indexed(236)));
        assert_eq!(cells[2].style.fg, None);
        assert_eq!(cells[2].style.bg, Some(Color::Indexed(236)));
    }

    #[test]
    fn bright_colors_and_attribute_resets_are_understood() {
        let rows = parse_rows("\x1b[95;104;4ma\x1b[22;24mb");
        let cells = &rows[0].cells;
        assert_eq!(cells[0].style.fg, Some(Color::Indexed(13)));
        assert_eq!(cells[0].style.bg, Some(Color::Indexed(12)));
        assert!(cells[0].style.underline);
        assert!(!cells[1].style.underline);
    }

    #[test]
    fn non_sgr_escape_sequences_are_dropped_without_leaking_text() {
        assert_eq!(
            plain("\x1b[2Ka\x1b]0;title\x07b\x1b]8;;http://x\x1b\\c\x1b(Bd\x1b=e"),
            vec!["abcde"]
        );
    }

    #[test]
    fn wide_characters_occupy_two_columns_and_combining_marks_join_their_base() {
        let rows = parse_rows("日本e\u{301}");
        let cells = &rows[0].cells;
        assert_eq!(cells[0].width, 2);
        assert_eq!(cells[2].text, "e\u{301}");
        assert_eq!(rows[0].display_width(), 5);
    }

    #[test]
    fn tabs_advance_to_the_next_tab_stop() {
        assert_eq!(plain("ab\tc"), vec!["ab      c"]);
        assert_eq!(parse_rows("ab\tc")[0].display_width(), 9);
    }

    #[test]
    fn a_row_ends_with_ink_only_when_its_last_cell_is_not_blank() {
        assert!(parse_rows("abc")[0].ends_with_ink());
        assert!(!parse_rows("abc ")[0].ends_with_ink());
        assert!(!parse_rows("")[0].ends_with_ink());
    }
}
