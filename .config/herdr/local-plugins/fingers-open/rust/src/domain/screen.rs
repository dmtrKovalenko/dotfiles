//! The captured pane as a grid, plus the "logical lines" that stitch wrapped
//! rows back together so a long URL split by the terminal still matches whole.

use super::ansi::{Row, parse_rows};

/// A contiguous run of cells on one row, in columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Segment {
    pub row: usize,
    pub col: u16,
    pub width: u16,
}

impl Segment {
    pub fn end(&self) -> u16 {
        self.col.saturating_add(self.width)
    }
}

/// The visible rows of a pane and the width they were rendered at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    rows: Vec<Row>,
    width: u16,
}

impl Screen {
    pub fn new(rows: Vec<Row>, width: u16) -> Self {
        Screen { rows, width }
    }

    /// Parses Herdr's `pane.read` output (ANSI format) rendered at `width`
    /// columns. A width of 0 disables wrapped-row joining.
    pub fn from_ansi(text: &str, width: u16) -> Self {
        Screen::new(parse_rows(text), width)
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    /// Rows glued back into the lines the program printed: a row that runs
    /// to the right edge and ends in ink continues on the next row.
    pub fn logical_lines(&self) -> Vec<LogicalLine> {
        let mut lines = Vec::new();
        let mut current = LogicalLine::default();
        for (index, row) in self.rows.iter().enumerate() {
            current.append_row(index, row);
            let next_continues = self
                .rows
                .get(index + 1)
                .is_some_and(|next| !next.is_empty());
            if !(self.row_wraps(row) && next_continues) {
                lines.push(std::mem::take(&mut current));
            }
        }
        if !current.cells.is_empty() {
            lines.push(current);
        }
        lines
    }

    fn row_wraps(&self, row: &Row) -> bool {
        // Herdr's pane rectangle may include a separator column, so a wrapped
        // row is one that fills the width or stops one column short of it.
        let wrap_width = usize::from(self.width.saturating_sub(1)).max(1);
        self.width > 0 && row.display_width() >= wrap_width && row.ends_with_ink()
    }
}

/// Where a slice of a logical line's text sits on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CellRef {
    row: usize,
    col: u16,
    width: u16,
    byte_start: usize,
    byte_end: usize,
}

/// One printed line, possibly spanning several rows, with a map from bytes
/// of its text back to screen cells.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogicalLine {
    text: String,
    cells: Vec<CellRef>,
}

impl LogicalLine {
    fn append_row(&mut self, row_index: usize, row: &Row) {
        let mut col: u16 = 0;
        for cell in &row.cells {
            let byte_start = self.text.len();
            self.text.push_str(&cell.text);
            self.cells.push(CellRef {
                row: row_index,
                col,
                width: u16::from(cell.width),
                byte_start,
                byte_end: self.text.len(),
            });
            col = col.saturating_add(u16::from(cell.width));
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The screen segments covering bytes `start..end` of the text, one per
    /// row touched, in reading order. Partial cells are included whole.
    pub fn segments(&self, start: usize, end: usize) -> Vec<Segment> {
        let mut segments: Vec<Segment> = Vec::new();
        for cell in &self.cells {
            if cell.byte_end <= start || cell.byte_start >= end {
                continue;
            }
            match segments.last_mut() {
                Some(last) if last.row == cell.row && last.end() == cell.col => {
                    last.width = last.width.saturating_add(cell.width);
                }
                _ => segments.push(Segment {
                    row: cell.row,
                    col: cell.col,
                    width: cell.width,
                }),
            }
        }
        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(screen: &Screen) -> Vec<String> {
        screen
            .logical_lines()
            .iter()
            .map(|line| line.text().to_string())
            .collect()
    }

    #[test]
    fn short_rows_stay_separate_lines() {
        let screen = Screen::from_ansi("first\nsecond", 40);
        assert_eq!(texts(&screen), vec!["first", "second"]);
    }

    #[test]
    fn a_row_filling_the_width_joins_the_next_row() {
        let screen = Screen::from_ansi("https://example.com/a/very/long/pa\nth/index.html", 34);
        assert_eq!(
            texts(&screen),
            vec!["https://example.com/a/very/long/path/index.html"]
        );
    }

    #[test]
    fn a_row_one_column_short_of_the_width_also_joins() {
        let screen = Screen::from_ansi("0123456789\nabc", 11);
        assert_eq!(texts(&screen), vec!["0123456789abc"]);
    }

    #[test]
    fn a_full_row_ending_in_spaces_does_not_join() {
        let screen = Screen::from_ansi("prompt    \nnext", 10);
        assert_eq!(texts(&screen), vec!["prompt    ", "next"]);
    }

    #[test]
    fn a_full_row_followed_by_an_empty_row_does_not_join() {
        let screen = Screen::from_ansi("0123456789\n\nabc", 10);
        assert_eq!(texts(&screen), vec!["0123456789", "", "abc"]);
    }

    #[test]
    fn zero_width_disables_joining() {
        let screen = Screen::from_ansi("0123456789\nabc", 0);
        assert_eq!(texts(&screen), vec!["0123456789", "abc"]);
    }

    #[test]
    fn segments_of_a_wrapped_match_cover_both_rows() {
        let screen = Screen::from_ansi("ab https://x.y\n/z rest", 14);
        let line = &screen.logical_lines()[0];
        assert_eq!(line.text(), "ab https://x.y/z rest");
        let start = line.text().find("https").unwrap();
        let end = line.text().find(" rest").unwrap();
        assert_eq!(
            line.segments(start, end),
            vec![
                Segment {
                    row: 0,
                    col: 3,
                    width: 11
                },
                Segment {
                    row: 1,
                    col: 0,
                    width: 2
                },
            ]
        );
    }

    #[test]
    fn segments_count_wide_characters_as_two_columns() {
        let screen = Screen::from_ansi("日本 /tmp/x", 40);
        let line = &screen.logical_lines()[0];
        let start = line.text().find('/').unwrap();
        assert_eq!(
            line.segments(start, line.text().len()),
            vec![Segment {
                row: 0,
                col: 5,
                width: 6
            }]
        );
    }
}
