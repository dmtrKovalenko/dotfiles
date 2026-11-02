use crate::config::Theme;
use crate::lineedit::LineEdit;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

/// Punctuation stripped from token edges; interior chars are kept so
/// paths, URLs, and versions survive intact.
const EDGE_PUNCT: &[char] = &[
    '(', ')', '[', ']', '{', '}', '<', '>', '\'', '"', '`', ',', ';', ':', '!', '?', '.',
];

const MIN_TOKEN_LEN: usize = 3;

/// Whitespace-separated tokens, edge punctuation trimmed, newest
/// (bottom of the scrollback) first, deduplicated keeping the newest.
pub fn tokenize_words(text: &str, min_len: usize) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line in text.lines().rev() {
        for tok in line.split_whitespace() {
            let tok = tok.trim_matches(|c| EDGE_PUNCT.contains(&c));
            if tok.chars().count() < min_len {
                continue;
            }
            if seen.insert(tok.to_string()) {
                out.push(tok.to_string());
            }
        }
    }
    out
}

/// Trimmed non-empty lines, newest first, deduplicated.
pub fn tokenize_lines(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line in text.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if seen.insert(line.to_string()) {
            out.push(line.to_string());
        }
    }
    out
}

fn is_boundary(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => c.is_whitespace() || "/._-:@".contains(c),
    }
}

/// Case-insensitive greedy subsequence score. None when `query` is not
/// a subsequence of `cand`. Consecutive hits and hits starting a word
/// or path segment score higher; skipped chars cost a little.
pub fn fuzzy_score(query: &str, cand: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let mut score = 0i64;
    let mut qchars = query.chars().map(|c| c.to_ascii_lowercase()).peekable();
    let mut prev: Option<char> = None;
    let mut prev_hit = false;
    for c in cand.chars() {
        let Some(&q) = qchars.peek() else {
            score -= 1;
            continue;
        };
        if c.to_ascii_lowercase() == q {
            qchars.next();
            score += 4;
            if prev_hit {
                score += 16;
            }
            if is_boundary(prev) {
                score += 8;
            }
            prev_hit = true;
        } else {
            score -= 1;
            prev_hit = false;
        }
        prev = Some(c);
    }
    if qchars.peek().is_some() {
        return None;
    }
    Some(score)
}

/// Indices of matching candidates, best score first; ties keep the
/// original (recency) order.
pub fn rank(query: &str, cands: &[String]) -> Vec<usize> {
    let mut scored: Vec<(i64, usize)> = cands
        .iter()
        .enumerate()
        .filter_map(|(i, c)| fuzzy_score(query, c).map(|s| (s, i)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    Word,
    Line,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExtractEffect {
    Copy(String),
    Insert(String),
}

/// Screen split for the fzf-style bottom popup: rows above the popup
/// show the source pane as a backdrop (its last row is the top divider),
/// the popup itself is `clamp(rows * pct / 100, 4, rows)` tall with three
/// reserved rows below the list: the keybind hint, a separator rule, and
/// the prompt. Returns (backdrop_rows, list_rows).
pub fn popup_layout(rows: usize, pct: u8) -> (usize, usize) {
    let popup = (rows * pct as usize / 100).max(4).min(rows.max(1));
    (rows.saturating_sub(popup), popup.saturating_sub(3).max(1))
}

/// extrakto-style token picker: type to fuzzy-filter, Enter inserts
/// into the source pane, Tab copies via OSC 52.
pub struct ExtractApp {
    pub query: LineEdit,
    pub granularity: Granularity,
    words: Vec<String>,
    lines: Vec<String>,
    /// Indices into the active candidate list, best match first.
    pub filtered: Vec<usize>,
    /// Index into `filtered`; 0 is the best match, drawn next to the prompt.
    pub selected: usize,
    /// List rows inside the popup (excludes the hint, separator, and
    /// prompt rows).
    pub view_rows: usize,
    pub view_cols: usize,
    /// Rows above the popup showing the pane backdrop.
    pub backdrop_rows: usize,
    /// Total rows in the backdrop buffer, for scroll clamping.
    backdrop_len: usize,
    /// Rows the backdrop view is shifted up from the tail (0 = tail).
    pub backdrop_off: usize,
    height_pct: u8,
    pub theme: Theme,
    pub quit: bool,
}

impl ExtractApp {
    pub fn new(text: &str, height_pct: u8, backdrop_len: usize) -> Self {
        let mut app = ExtractApp {
            query: LineEdit::default(),
            granularity: Granularity::Word,
            words: tokenize_words(text, MIN_TOKEN_LEN),
            lines: tokenize_lines(text),
            filtered: Vec::new(),
            selected: 0,
            view_rows: 1,
            view_cols: 80,
            backdrop_rows: 0,
            backdrop_len,
            backdrop_off: 0,
            height_pct: height_pct.clamp(1, 100),
            theme: Theme::default(),
            quit: false,
        };
        app.refilter();
        app
    }

    /// Replace the active color theme (ui.rs defaults + user config).
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn set_size(&mut self, cols: usize, rows: usize) {
        self.view_cols = cols.max(1);
        let (backdrop, list) = popup_layout(rows, self.height_pct);
        self.backdrop_rows = backdrop;
        self.view_rows = list;
        if self.backdrop_rows == 0 {
            self.backdrop_off = 0;
        } else {
            self.scroll_backdrop(0);
        }
    }

    /// Backdrop rows that show content (its last row is the separator).
    fn backdrop_content_rows(&self) -> usize {
        self.backdrop_rows.saturating_sub(1)
    }

    /// Shift the backdrop view by `delta` rows (positive = older
    /// output), clamped between the tail and the oldest read line.
    pub fn scroll_backdrop(&mut self, delta: isize) {
        if self.backdrop_rows == 0 {
            return;
        }
        let max = self
            .backdrop_len
            .saturating_sub(self.backdrop_content_rows());
        let off = self.backdrop_off as isize + delta;
        self.backdrop_off = off.clamp(0, max as isize) as usize;
    }

    pub fn candidates(&self) -> &[String] {
        match self.granularity {
            Granularity::Word => &self.words,
            Granularity::Line => &self.lines,
        }
    }

    pub fn selected_text(&self) -> Option<&str> {
        let idx = *self.filtered.get(self.selected)?;
        Some(&self.candidates()[idx])
    }

    fn refilter(&mut self) {
        self.filtered = rank(&self.query.text(), self.candidates());
        self.selected = 0;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ExtractEffect> {
        use KeyCode::*;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            Char('c') if ctrl => self.quit = true,
            Char('t') if ctrl => {
                self.granularity = match self.granularity {
                    Granularity::Word => Granularity::Line,
                    Granularity::Line => Granularity::Word,
                };
                self.refilter();
            }
            Char('p') if ctrl => self.move_selection(1),
            Char('n') if ctrl => self.move_selection(-1),
            Up => self.move_selection(1),
            Down => self.move_selection(-1),
            PageUp => self.scroll_backdrop(self.backdrop_content_rows().max(1) as isize),
            PageDown => self.scroll_backdrop(-(self.backdrop_content_rows().max(1) as isize)),
            Esc => {
                if self.query.is_empty() {
                    self.quit = true;
                } else {
                    self.query.clear();
                    self.refilter();
                }
            }
            Enter => {
                if let Some(text) = self.selected_text() {
                    let text = text.to_string();
                    self.quit = true;
                    return Some(ExtractEffect::Insert(text));
                }
            }
            Tab => {
                if let Some(text) = self.selected_text() {
                    let text = text.to_string();
                    self.quit = true;
                    return Some(ExtractEffect::Copy(text));
                }
            }
            _ => {
                if self.query.handle_key(key) {
                    self.refilter();
                }
            }
        }
        None
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let max = self.filtered.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, max) as usize;
    }

    /// Wheel over the backdrop scrolls it; over the popup it moves the
    /// selection (up = older, same as the Up key).
    pub fn handle_mouse(&mut self, ev: MouseEvent) {
        let delta = match ev.kind {
            MouseEventKind::ScrollUp => 1,
            MouseEventKind::ScrollDown => -1,
            _ => return,
        };
        if (ev.row as usize) < self.backdrop_rows {
            self.scroll_backdrop(delta * 3);
        } else {
            self.move_selection(delta);
        }
    }

    /// The window of `filtered` to display, keeping the selection
    /// visible within view_rows entries.
    pub fn window(&self) -> &[usize] {
        let start = (self.selected + 1).saturating_sub(self.view_rows);
        let end = (start + self.view_rows).min(self.filtered.len());
        &self.filtered[start..end]
    }

    /// Position of the selected entry inside `window()`.
    pub fn window_selected(&self) -> usize {
        self.selected - (self.selected + 1).saturating_sub(self.view_rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn code(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    #[test]
    fn words_are_trimmed_deduped_and_newest_first() {
        let text = "run src/app.rs:12:5 (ok)\nsee https://x.dev, done\nsee run";
        assert_eq!(
            tokenize_words(text, 3),
            vec!["see", "run", "https://x.dev", "done", "src/app.rs:12:5"]
        );
    }

    #[test]
    fn short_tokens_are_dropped() {
        assert_eq!(tokenize_words("a bb ccc dddd", 3), vec!["ccc", "dddd"]);
    }

    #[test]
    fn lines_are_trimmed_deduped_and_newest_first() {
        let text = "first\n\n  second  \nfirst";
        assert_eq!(tokenize_lines(text), vec!["first", "second"]);
    }

    #[test]
    fn fuzzy_requires_subsequence() {
        assert!(fuzzy_score("xyz", "abc").is_none());
        assert!(fuzzy_score("abc", "a1b2c3").is_some());
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn fuzzy_is_case_insensitive() {
        assert!(fuzzy_score("ABC", "abc").is_some());
    }

    #[test]
    fn consecutive_hits_beat_gapped_hits() {
        assert!(fuzzy_score("abc", "abc").unwrap() > fuzzy_score("abc", "a_b_c").unwrap());
    }

    #[test]
    fn segment_starts_beat_interior_hits() {
        assert!(fuzzy_score("app", "src/app.rs").unwrap() > fuzzy_score("app", "zapp.rs").unwrap());
    }

    #[test]
    fn rank_keeps_recency_on_ties() {
        let cands = vec!["bbb".to_string(), "aaa".to_string()];
        assert_eq!(rank("", &cands), vec![0, 1]);
    }

    #[test]
    fn popup_layout_splits_forty_percent() {
        // popup = 12 rows: 9 list + hint + separator + prompt.
        assert_eq!(popup_layout(30, 40), (18, 9));
    }

    #[test]
    fn popup_layout_clamps_small_and_full() {
        assert_eq!(popup_layout(30, 100), (0, 27));
        assert_eq!(popup_layout(30, 1), (26, 1), "minimum popup of 4 rows");
        assert_eq!(popup_layout(2, 40), (0, 1), "tiny terminal survives");
    }

    #[test]
    fn typing_filters_and_enter_inserts_best_match() {
        let mut a = ExtractApp::new("alpha\nbeta\ngamma", 100, 0);
        assert_eq!(a.selected_text(), Some("gamma"));
        a.handle_key(key('b'));
        a.handle_key(key('e'));
        assert_eq!(a.selected_text(), Some("beta"));
        let eff = a.handle_key(code(KeyCode::Enter));
        assert_eq!(eff, Some(ExtractEffect::Insert("beta".into())));
        assert!(a.quit);
    }

    #[test]
    fn tab_copies_selected() {
        let mut a = ExtractApp::new("alpha\nbeta", 100, 0);
        let eff = a.handle_key(code(KeyCode::Tab));
        assert_eq!(eff, Some(ExtractEffect::Copy("beta".into())));
        assert!(a.quit);
    }

    #[test]
    fn ctrl_t_toggles_line_granularity() {
        let mut a = ExtractApp::new("one two\nthree four", 100, 0);
        a.handle_key(ctrl('t'));
        assert_eq!(a.granularity, Granularity::Line);
        assert_eq!(a.selected_text(), Some("three four"));
    }

    #[test]
    fn esc_clears_query_then_quits() {
        let mut a = ExtractApp::new("alpha", 100, 0);
        a.handle_key(key('a'));
        a.handle_key(code(KeyCode::Esc));
        assert_eq!(a.query.text(), "");
        assert!(!a.quit);
        a.handle_key(code(KeyCode::Esc));
        assert!(a.quit);
    }

    #[test]
    fn query_cursor_editing_refilters() {
        let mut a = ExtractApp::new("beta\nbta", 100, 0);
        a.handle_key(key('b'));
        a.handle_key(key('t'));
        a.handle_key(key('a'));
        assert_eq!(a.selected_text(), Some("bta"));
        a.handle_key(code(KeyCode::Left));
        a.handle_key(code(KeyCode::Left));
        a.handle_key(key('e'));
        assert_eq!(a.query.text(), "beta");
        assert_eq!(a.selected_text(), Some("beta"));
    }

    #[test]
    fn backdrop_scroll_clamps_to_history() {
        let mut a = ExtractApp::new("x", 40, 50);
        // popup_layout(30, 40) = (18, 11): 17 backdrop content rows.
        a.set_size(80, 30);
        a.scroll_backdrop(3);
        assert_eq!(a.backdrop_off, 3);
        a.scroll_backdrop(1000);
        assert_eq!(a.backdrop_off, 33, "oldest read line stays on screen");
        a.scroll_backdrop(-1000);
        assert_eq!(a.backdrop_off, 0);
    }

    #[test]
    fn backdrop_scroll_noop_without_backdrop_or_history() {
        let mut a = ExtractApp::new("x", 100, 50);
        a.set_size(80, 30); // full-height popup: no backdrop
        a.scroll_backdrop(5);
        assert_eq!(a.backdrop_off, 0);
        let mut b = ExtractApp::new("x", 40, 10);
        b.set_size(80, 30); // 17 content rows already show all 10 lines
        b.scroll_backdrop(5);
        assert_eq!(b.backdrop_off, 0);
    }

    #[test]
    fn page_keys_scroll_the_backdrop() {
        let mut a = ExtractApp::new("x", 40, 100);
        a.set_size(80, 30);
        a.handle_key(code(KeyCode::PageUp));
        assert_eq!(a.backdrop_off, 17);
        a.handle_key(code(KeyCode::PageDown));
        assert_eq!(a.backdrop_off, 0);
    }

    #[test]
    fn resize_reclamps_the_backdrop_offset() {
        let mut a = ExtractApp::new("x", 40, 20);
        a.set_size(80, 30); // 17 content rows: max offset 3
        a.scroll_backdrop(1000);
        assert_eq!(a.backdrop_off, 3);
        a.set_size(80, 40); // 23 content rows already show everything
        assert_eq!(a.backdrop_off, 0);
    }

    #[test]
    fn wheel_scrolls_backdrop_or_selection_by_region() {
        let mut a = ExtractApp::new("one\ntwo\nthree\nfour", 40, 100);
        a.set_size(80, 30); // backdrop rows 0..18, popup below
        let wheel = |row: u16, kind| MouseEvent {
            kind,
            column: 0,
            row,
            modifiers: KeyModifiers::NONE,
        };
        a.handle_mouse(wheel(0, MouseEventKind::ScrollUp));
        assert_eq!(a.backdrop_off, 3);
        a.handle_mouse(wheel(0, MouseEventKind::ScrollDown));
        assert_eq!(a.backdrop_off, 0);
        a.handle_mouse(wheel(20, MouseEventKind::ScrollUp));
        assert_eq!(a.selected, 1);
        a.handle_mouse(wheel(20, MouseEventKind::ScrollDown));
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn selection_moves_within_bounds_and_window_follows() {
        let mut a = ExtractApp::new("one\ntwo\nthree\nfour", 100, 0);
        a.set_size(80, 5); // 2 list rows (3 reserved: hint, separator, prompt)
        assert_eq!(a.filtered.len(), 4);
        a.handle_key(code(KeyCode::Up));
        a.handle_key(code(KeyCode::Up));
        a.handle_key(code(KeyCode::Up));
        assert_eq!(a.selected, 3);
        a.handle_key(code(KeyCode::Up));
        assert_eq!(a.selected, 3, "clamped at oldest");
        assert_eq!(a.window(), &a.filtered[2..4]);
        assert_eq!(a.window_selected(), 1);
        a.handle_key(ctrl('n'));
        assert_eq!(a.selected, 2);
    }
}
