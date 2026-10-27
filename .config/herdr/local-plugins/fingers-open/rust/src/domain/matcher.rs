//! Runs the patterns over the screen and places every match back on the grid.

use super::patterns::PatternSet;
use super::screen::{Screen, Segment};

/// Something on screen worth a hint: its copyable text and where it is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub pattern: String,
    /// One segment per row the copyable text touches, in reading order.
    pub segments: Vec<Segment>,
}

impl Candidate {
    pub fn first_row(&self) -> usize {
        self.segments.first().map_or(0, |s| s.row)
    }

    pub fn first_col(&self) -> u16 {
        self.segments.first().map_or(0, |s| s.col)
    }
}

/// Every match on the screen, in reading order (top to bottom, left to right).
pub fn find_candidates(screen: &Screen, patterns: &PatternSet) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for line in screen.logical_lines() {
        for found in patterns.find(line.text()) {
            let segments = line.segments(found.capture_start, found.capture_end);
            if segments.is_empty() {
                continue;
            }
            candidates.push(Candidate {
                text: line.text()[found.capture_start..found.capture_end].to_string(),
                pattern: found.pattern,
                segments,
            });
        }
    }
    candidates.sort_by_key(|c| (c.first_row(), c.first_col()));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_come_out_in_reading_order_with_their_positions() {
        let screen = Screen::from_ansi("a /etc/hosts\n1234 https://x.io", 40);
        let found = find_candidates(&screen, &PatternSet::builtin());
        let summary: Vec<(&str, usize, u16)> = found
            .iter()
            .map(|c| (c.text.as_str(), c.first_row(), c.first_col()))
            .collect();
        assert_eq!(
            summary,
            vec![("/etc/hosts", 0, 2), ("1234", 1, 0), ("https://x.io", 1, 5)]
        );
    }

    #[test]
    fn a_url_wrapped_over_two_rows_is_one_candidate() {
        let screen = Screen::from_ansi("go https://example.com/abc\ndef/ghi ok", 26);
        let found = find_candidates(&screen, &PatternSet::builtin());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "https://example.com/abcdef/ghi");
        assert_eq!(found[0].segments.len(), 2);
        assert_eq!(
            found[0].segments[1],
            Segment {
                row: 1,
                col: 0,
                width: 7
            }
        );
    }

    #[test]
    fn styled_text_matches_the_same_as_plain_text() {
        let plain = Screen::from_ansi("see /var/log/syslog", 40);
        let styled = Screen::from_ansi("\x1b[32msee \x1b[1;34m/var/log/syslog\x1b[0m", 40);
        let set = PatternSet::builtin();
        assert_eq!(
            find_candidates(&plain, &set),
            find_candidates(&styled, &set)
        );
    }
}
