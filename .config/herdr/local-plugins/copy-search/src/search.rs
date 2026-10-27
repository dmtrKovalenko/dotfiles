use crate::buffer::{Buffer, Pos};

/// One search hit as an inclusive span of buffer positions. A hit may
/// cross soft-wrapped rows, so `start` and `end` can be on different
/// rows; `end` is the last matched position (inclusive), which lets a
/// wrapped match be copied directly via `Buffer::slice_inclusive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub start: Pos,
    pub end: Pos,
}

impl Match {
    /// Whether `p` lies within the span. Pos ordering is row-major, so a
    /// multi-row span covers every position on its interior rows.
    pub fn covers(&self, p: Pos) -> bool {
        self.start <= p && p <= self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

impl Direction {
    pub fn flipped(self) -> Self {
        match self {
            Direction::Forward => Direction::Backward,
            Direction::Backward => Direction::Forward,
        }
    }
}

pub struct Search {
    pub pattern: String,
    pub direction: Option<Direction>,
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    /// Smartcase (vi-style, default on): a pattern with no uppercase is
    /// matched case-insensitively; any uppercase makes it case-sensitive.
    /// Toggled off, search is always case-sensitive.
    pub smartcase: bool,
}

impl Default for Search {
    fn default() -> Self {
        Search {
            pattern: String::new(),
            direction: None,
            matches: Vec::new(),
            current: None,
            smartcase: true,
        }
    }
}

impl Search {
    /// Recompute all matches for a pattern. The pattern is tried as a
    /// regex first (tmux search semantics) and falls back to a literal
    /// match when it is not a valid regex.
    pub fn run(&mut self, buf: &Buffer, pattern: &str, direction: Direction) {
        self.pattern = pattern.to_string();
        self.direction = Some(direction);
        self.matches.clear();
        self.current = None;
        if pattern.is_empty() {
            return;
        }
        // Smartcase: fold case only when the pattern has no uppercase.
        let ci = self.smartcase && !pattern.chars().any(char::is_uppercase);
        let build = |pat: &str| {
            regex::RegexBuilder::new(pat)
                .case_insensitive(ci)
                .build()
                .ok()
        };
        let re = match build(pattern).or_else(|| build(&regex::escape(pattern))) {
            Some(re) => re,
            None => return,
        };
        // Match over logical lines (soft-wrapped rows joined) so a hit
        // can cross a wrap; each hit maps back to inclusive buffer
        // positions. Groups are visited in row order and each yields
        // matches left to right, keeping `matches` sorted for seek.
        let mut row = 0;
        while row < buf.lines.len() {
            let (r0, r1) = buf.logical_span(row);
            let mut s = String::new();
            let mut spans: Vec<(usize, usize)> = Vec::new(); // (row, char offset)
            let mut acc = 0usize;
            for r in r0..=r1 {
                spans.push((r, acc));
                let line = buf.line(r);
                s.push_str(&line.text());
                acc += line.len();
            }
            // Map a char offset in `s` back to its buffer Pos.
            let to_pos = |o: usize| {
                let mut chosen = spans[0];
                for &pair in &spans {
                    if pair.1 <= o {
                        chosen = pair;
                    } else {
                        break;
                    }
                }
                Pos {
                    row: chosen.0,
                    col: o - chosen.1,
                }
            };
            for m in re.find_iter(&s) {
                if m.start() == m.end() {
                    continue;
                }
                let cstart = s[..m.start()].chars().count();
                let cend = cstart + s[m.start()..m.end()].chars().count();
                self.matches.push(Match {
                    start: to_pos(cstart),
                    end: to_pos(cend - 1),
                });
            }
            row = r1 + 1;
        }
    }

    pub fn clear(&mut self) {
        self.pattern.clear();
        self.matches.clear();
        self.current = None;
    }

    /// Find the nearest match from `pos` in `dir`. With `inclusive`, a
    /// match starting exactly at `pos` counts (used while typing the
    /// pattern); without, the cursor position is skipped (n/N).
    pub fn seek(&mut self, pos: Pos, dir: Direction, inclusive: bool) -> Option<Match> {
        if self.matches.is_empty() {
            self.current = None;
            return None;
        }
        let idx = match dir {
            Direction::Forward => {
                let key = if inclusive {
                    pos
                } else {
                    Pos {
                        row: pos.row,
                        col: pos.col + 1,
                    }
                };
                self.matches
                    .iter()
                    .position(|m| m.start >= key)
                    .unwrap_or(0)
            }
            Direction::Backward => {
                let key = pos;
                let hit = if inclusive {
                    self.matches.iter().rposition(|m| m.start <= key)
                } else {
                    self.matches.iter().rposition(|m| m.start < key)
                };
                hit.unwrap_or(self.matches.len() - 1)
            }
        };
        self.current = Some(idx);
        Some(self.matches[idx])
    }

    /// n: continue in the search direction.
    pub fn next(&mut self, pos: Pos) -> Option<Match> {
        let dir = self.direction?;
        self.seek(pos, dir, false)
    }

    /// N: continue against the search direction.
    pub fn prev(&mut self, pos: Pos) -> Option<Match> {
        let dir = self.direction?.flipped();
        self.seek(pos, dir, false)
    }

    /// The match covering `pos`, if any.
    pub fn match_at(&self, pos: Pos) -> Option<Match> {
        self.matches.iter().copied().find(|m| m.covers(pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf() -> Buffer {
        Buffer::from_text("foo bar foo\nbaz\nfoo end")
    }

    fn p(row: usize, col: usize) -> Pos {
        Pos { row, col }
    }

    /// Single-row match spanning cols `start..=end` (both inclusive).
    fn hit(row: usize, start: usize, end: usize) -> Match {
        Match {
            start: Pos { row, col: start },
            end: Pos { row, col: end },
        }
    }

    #[test]
    fn finds_all_matches_with_char_indices() {
        let b = buf();
        let mut s = Search::default();
        s.run(&b, "foo", Direction::Forward);
        assert_eq!(s.matches, vec![hit(0, 0, 2), hit(0, 8, 10), hit(2, 0, 2)]);
    }

    #[test]
    fn smartcase_lowercase_matches_any_case() {
        let b = Buffer::from_text("Foo foo FOO");
        let mut s = Search::default();
        s.run(&b, "foo", Direction::Forward);
        assert_eq!(s.matches.len(), 3);
    }

    #[test]
    fn smartcase_uppercase_is_case_sensitive() {
        let b = Buffer::from_text("Foo foo FOO");
        let mut s = Search::default();
        s.run(&b, "Foo", Direction::Forward);
        assert_eq!(s.matches, vec![hit(0, 0, 2)]);
    }

    #[test]
    fn smartcase_off_is_always_case_sensitive() {
        let b = Buffer::from_text("Foo foo FOO");
        let mut s = Search {
            smartcase: false,
            ..Default::default()
        };
        s.run(&b, "foo", Direction::Forward);
        assert_eq!(s.matches, vec![hit(0, 4, 6)]);
    }

    #[test]
    fn invalid_regex_falls_back_to_literal() {
        let b = Buffer::from_text("a(b\nc");
        let mut s = Search::default();
        s.run(&b, "a(b", Direction::Forward);
        assert_eq!(s.matches.len(), 1);
    }

    #[test]
    fn wide_chars_use_char_columns() {
        let b = Buffer::from_text("\u{4e2d}\u{6587}foo");
        let mut s = Search::default();
        s.run(&b, "foo", Direction::Forward);
        assert_eq!(s.matches, vec![hit(0, 2, 4)]);
    }

    #[test]
    fn matches_across_soft_wrap() {
        // "foobar" wrapped as "foo"(soft) + "bar"; "oobar" spans the wrap.
        let b = Buffer::from_reads("foobar\nend", "foo\nbar\nend");
        let mut s = Search::default();
        s.run(&b, "oobar", Direction::Forward);
        assert_eq!(
            s.matches,
            vec![Match {
                start: p(0, 1),
                end: p(1, 2),
            }]
        );
        assert!(s.match_at(p(0, 1)).is_some());
        assert!(s.match_at(p(1, 0)).is_some());
        assert!(s.match_at(p(1, 2)).is_some());
        assert!(s.match_at(p(0, 0)).is_none());
    }

    #[test]
    fn hard_newline_is_not_crossed() {
        // A real newline breaks the logical line, so no cross-row match.
        let b = Buffer::from_text("foo\nbar");
        let mut s = Search::default();
        s.run(&b, "oobar", Direction::Forward);
        assert!(s.matches.is_empty());
    }

    #[test]
    fn seek_forward_wraps() {
        let b = buf();
        let mut s = Search::default();
        s.run(&b, "foo", Direction::Forward);
        let m = s.seek(p(2, 1), Direction::Forward, false).unwrap();
        assert_eq!(m.start, p(0, 0));
    }

    #[test]
    fn seek_inclusive_stays_on_cursor_match() {
        let b = buf();
        let mut s = Search::default();
        s.run(&b, "foo", Direction::Forward);
        let m = s.seek(p(0, 0), Direction::Forward, true).unwrap();
        assert_eq!(m.start, p(0, 0));
        let m = s.seek(p(0, 0), Direction::Forward, false).unwrap();
        assert_eq!(m.start, p(0, 8));
    }

    #[test]
    fn seek_backward_wraps() {
        let b = buf();
        let mut s = Search::default();
        s.run(&b, "foo", Direction::Backward);
        let m = s.seek(p(0, 0), Direction::Backward, false).unwrap();
        assert_eq!(m.start.row, 2);
    }

    #[test]
    fn next_and_prev_respect_direction() {
        let b = buf();
        let mut s = Search::default();
        s.run(&b, "foo", Direction::Backward);
        // n with a backward search keeps going backward.
        let m = s.next(p(2, 0)).unwrap();
        assert_eq!(m.start, p(0, 8));
        let m = s.prev(p(0, 8)).unwrap();
        assert_eq!(m.start, p(2, 0));
    }

    #[test]
    fn match_at_covers_position() {
        let b = buf();
        let mut s = Search::default();
        s.run(&b, "foo", Direction::Forward);
        assert!(s.match_at(Pos { row: 0, col: 9 }).is_some());
        assert!(s.match_at(Pos { row: 0, col: 3 }).is_none());
    }
}
