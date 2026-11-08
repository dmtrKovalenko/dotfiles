use crate::buffer::{Buffer, Pos};
use crate::click::ClickTracker;
use crate::config::Theme;
use crate::lineedit::LineEdit;
use crate::motion;
use crate::patterns::{PatternDef, Patterns};
use crate::search::{Direction, Search};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::Instant;

pub enum Mode {
    Normal,
    Prompt {
        direction: Direction,
        input: LineEdit,
        origin: Pos,
        origin_top: usize,
    },
    PatternMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelKind {
    Char,
    Line,
}

/// A key waiting for its completion key (vi-style multi-key command).
#[derive(Debug, Clone, Copy)]
enum Pending {
    /// `i`/`a` pressed; the next key names the text object (`w W ( [ {
    /// < b B " ' ` q p l e`).
    TextObject { around: bool },
    /// `g` pressed; `gg` top, `ge`/`gE` previous word end, `g_` last
    /// non-blank, `gx` opens the current token.
    Goto,
    /// `f`/`F`/`t`/`T` pressed; the next key is the char to find.
    Find { forward: bool, till: bool },
    /// `z` pressed; `zz`/`zt`/`zb` scroll the cursor row into place.
    Scroll,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    Copy(String),
    Open(String),
}

pub struct App {
    pub buf: Buffer,
    pub cursor: Pos,
    pub top: usize,
    pub hoff: usize,
    pub view_rows: usize,
    pub view_cols: usize,
    pub search: Search,
    pub patterns: Patterns,
    pub theme: Theme,
    pub mode: Mode,
    pub sel: Option<(SelKind, Pos)>,
    pub message: Option<String>,
    pub quit: bool,
    want_col: usize,
    drag_origin: Option<Pos>,
    clicks: ClickTracker,
    /// The current selection came from a mouse gesture, so releasing the
    /// left button copies it.
    mouse_sel: bool,
    saved_pattern: String,
    pending: Option<Pending>,
    /// Count prefix typed so far (0 = none).
    count: usize,
    /// Last `f`/`F`/`t`/`T` (target, forward, till) for `;` and `,`.
    last_find: Option<(char, bool, bool)>,
    pending_bottom: bool,
    /// Rows the source pane was showing when captured. The bottom-most
    /// view keeps exactly this many tail rows on screen, so opening
    /// the overlay does not move the content and scrolling back down
    /// restores the original screen (None: fill the view, dev inputs).
    screen_rows: Option<usize>,
}

impl App {
    pub fn new(buf: Buffer, start_search: bool, screen_rows: Option<usize>) -> Self {
        let cursor = Pos {
            row: buf.last_row(),
            col: 0,
        };
        let mode = if start_search {
            Mode::Prompt {
                direction: Direction::Forward,
                input: LineEdit::default(),
                origin: cursor,
                origin_top: 0,
            }
        } else {
            Mode::Normal
        };
        App {
            buf,
            cursor,
            top: 0,
            hoff: 0,
            view_rows: 1,
            view_cols: 80,
            search: Search::default(),
            patterns: Patterns::default(),
            theme: Theme::default(),
            mode,
            sel: None,
            message: None,
            quit: false,
            want_col: 0,
            drag_origin: None,
            clicks: ClickTracker::default(),
            mouse_sel: false,
            saved_pattern: String::new(),
            pending: None,
            count: 0,
            last_find: None,
            pending_bottom: true,
            screen_rows,
        }
    }

    /// Replace the active pattern set (copycat defaults + user config).
    pub fn set_patterns(&mut self, patterns: Patterns) {
        self.patterns = patterns;
    }

    /// Replace the active color theme (ui.rs defaults + user config).
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Called by the UI before each draw with the terminal size.
    pub fn set_size(&mut self, cols: usize, rows: usize) {
        self.view_cols = cols.max(1);
        self.view_rows = rows.max(1);
        if self.pending_bottom {
            self.pending_bottom = false;
            self.top = self.max_top();
            if let Mode::Prompt { origin_top, .. } = &mut self.mode {
                *origin_top = self.top;
            }
        }
        self.ensure_visible();
    }

    /// The bottom row is the persistent mode row (herdr-native style),
    /// so it never counts as visible content.
    fn content_rows(&self) -> usize {
        (self.view_rows - 1).max(1)
    }

    /// Top row of the bottom-most view. Anchored to the rows the
    /// source pane was showing, so content that sat at the top of a
    /// partly filled screen stays there instead of being pulled down.
    fn max_top(&self) -> usize {
        let anchor = self
            .screen_rows
            .unwrap_or(usize::MAX)
            .min(self.content_rows())
            .max(1);
        self.buf.last_row().saturating_sub(anchor - 1)
    }

    fn ensure_visible(&mut self) {
        let rows = self.content_rows();
        if self.cursor.row < self.top {
            self.top = self.cursor.row;
        }
        if self.cursor.row >= self.top + rows {
            self.top = self.cursor.row + 1 - rows;
        }
        if self.top > self.max_top() {
            self.top = self.max_top();
        }
        self.ensure_visible_h();
    }

    /// Horizontal follow in display cells: the cursor's whole cell range
    /// must stay inside [hoff, hoff + view_cols).
    fn ensure_visible_h(&mut self) {
        let line = self.buf.line(self.cursor.row);
        let x = line.width_before(self.cursor.col);
        let cw = line.cell_width(self.cursor.col);
        if x < self.hoff {
            self.hoff = x;
        }
        if x + cw > self.hoff + self.view_cols {
            self.hoff = x + cw - self.view_cols;
        }
    }

    /// Wheel scrolling moves the view and drags the cursor along.
    fn scroll(&mut self, delta: isize) {
        let top = self.top as isize + delta;
        self.top = top.clamp(0, self.max_top() as isize) as usize;
        let low = self.top;
        let high = self.top + self.content_rows() - 1;
        let row = self.cursor.row.clamp(low, high.min(self.buf.last_row()));
        self.cursor = Pos {
            row,
            col: self.want_col.min(self.buf.max_col(row)),
        };
        self.ensure_visible_h();
    }

    fn move_vertical(&mut self, delta: isize) {
        let row =
            (self.cursor.row as isize + delta).clamp(0, self.buf.last_row() as isize) as usize;
        self.cursor = Pos {
            row,
            col: self.want_col.min(self.buf.max_col(row)),
        };
        self.ensure_visible();
    }

    fn goto(&mut self, p: Pos) {
        self.cursor = self.buf.clamp(p);
        self.want_col = self.cursor.col;
        self.ensure_visible();
    }

    pub fn selection_text(&self) -> Option<String> {
        let (kind, anchor) = self.sel?;
        Some(match kind {
            SelKind::Char => self.buf.slice_inclusive(anchor, self.cursor),
            SelKind::Line => self.buf.slice_lines(anchor.row, self.cursor.row),
        })
    }

    /// Inclusive selected col range for a row, if the selection covers it.
    pub fn sel_range(&self, row: usize) -> Option<(usize, usize)> {
        let (kind, anchor) = self.sel?;
        let (a, b) = if (anchor.row, anchor.col) <= (self.cursor.row, self.cursor.col) {
            (anchor, self.cursor)
        } else {
            (self.cursor, anchor)
        };
        if row < a.row || row > b.row {
            return None;
        }
        match kind {
            SelKind::Line => Some((0, usize::MAX)),
            SelKind::Char => {
                let s = if row == a.row { a.col } else { 0 };
                let e = if row == b.row { b.col } else { usize::MAX };
                Some((s, e))
            }
        }
    }

    fn yank(&mut self, exit: bool) -> Option<Effect> {
        let text = self.selection_text().or_else(|| {
            self.search
                .match_at(self.cursor)
                .map(|m| self.buf.slice_inclusive(m.start, m.end))
        })?;
        if exit {
            self.quit = true;
        } else {
            self.message = Some(format!("copied {} chars", text.chars().count()));
            self.sel = None;
        }
        Some(Effect::Copy(text))
    }

    /// Copy path for a finished mouse gesture: reports the same "copied
    /// N chars" as `y` but leaves the selection highlighted, the way a
    /// GUI selection survives a copy (Esc clears it). None without a
    /// selection, so a plain click copies nothing - unlike `yank`, there
    /// is deliberately no match-under-cursor fallback here.
    fn yank_keeping_selection(&mut self) -> Option<Effect> {
        let text = self.selection_text()?;
        self.message = Some(format!("copied {} chars", text.chars().count()));
        Some(Effect::Copy(text))
    }

    fn jump_match(&mut self, forward: bool) {
        if self.search.pattern.is_empty() {
            self.message = Some("no previous search".into());
            return;
        }
        let m = if forward {
            self.search.next(self.cursor)
        } else {
            self.search.prev(self.cursor)
        };
        match m {
            Some(m) => self.goto(m.start),
            None => self.message = Some(format!("no match: {}", self.search.pattern)),
        }
    }

    fn open_prompt(&mut self, direction: Direction) {
        self.saved_pattern = self.search.pattern.clone();
        self.mode = Mode::Prompt {
            direction,
            input: LineEdit::default(),
            origin: self.cursor,
            origin_top: self.top,
        };
    }

    fn incremental(&mut self, input: &str, dir: Direction, origin: Pos, origin_top: usize) {
        self.search.run(&self.buf, input, dir);
        if input.is_empty() {
            self.cursor = origin;
            self.top = origin_top;
            return;
        }
        match self.search.seek(origin, dir, true) {
            Some(m) => self.goto(m.start),
            None => {
                self.cursor = origin;
                self.top = origin_top;
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Effect> {
        self.message = None;
        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Prompt { .. } => {
                self.handle_prompt(key);
                None
            }
            Mode::PatternMenu => {
                self.handle_pattern_menu(key);
                None
            }
        }
    }

    /// Predefined pattern search, copycat style: backward from the
    /// cursor so `n` walks up through older output and `N` back down.
    fn activate_pattern(&mut self, def: &PatternDef) {
        self.search.run(&self.buf, &def.regex, Direction::Backward);
        let n = self.search.matches.len();
        if n == 0 {
            self.message = Some(format!("no match: {}", def.name));
            return;
        }
        if let Some(m) = self.search.seek(self.cursor, Direction::Backward, true) {
            self.goto(m.start);
        }
        self.message = Some(format!("{}: {} matches", def.name, n));
    }

    fn handle_pattern_menu(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.quit = true;
            }
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char(c) => {
                self.mode = Mode::Normal;
                match self.patterns.by_key(c).cloned() {
                    Some(def) => self.activate_pattern(&def),
                    None => self.message = Some(format!("no pattern: {c}")),
                }
            }
            _ => {}
        }
    }

    /// Charwise selection from `start` to `end`, cursor on `end`.
    fn select_chars(&mut self, start: Pos, end: Pos) {
        self.sel = Some((SelKind::Char, start));
        self.cursor = end;
        self.want_col = end.col;
        self.ensure_visible();
    }

    /// Linewise selection of rows `r0..=r1`, cursor on the last row.
    fn select_rows(&mut self, r0: usize, r1: usize) {
        self.sel = Some((SelKind::Line, Pos { row: r0, col: 0 }));
        self.cursor = Pos {
            row: r1,
            col: self.buf.max_col(r1),
        };
        self.want_col = self.cursor.col;
        self.ensure_visible();
    }

    /// Select the word/WORD text object around the cursor as a charwise
    /// selection. `around` = aw/aW (includes adjacent whitespace).
    fn apply_text_object(&mut self, around: bool, big: bool) {
        let (start, end) = motion::word_object(&self.buf, self.cursor, around, big);
        self.select_chars(start, end);
    }

    /// Ordered (start, end) of the current charwise selection, if any.
    fn char_sel_bounds(&self) -> Option<(Pos, Pos)> {
        match self.sel {
            Some((SelKind::Char, anchor)) => Some(if anchor <= self.cursor {
                (anchor, self.cursor)
            } else {
                (self.cursor, anchor)
            }),
            _ => None,
        }
    }

    /// Bracket objects `i( a( i[ a[ i{ a{ i< a< ib ab iB aB`. `levels`
    /// (the count) picks an enclosing pair further out; repeating the
    /// object while it is already selected expands outward one level,
    /// as in vi visual mode.
    fn apply_pair_object(&mut self, pair: Option<(char, char)>, around: bool, levels: usize) {
        let compute = |app: &Self, lv: usize| -> Option<(Pos, Pos)> {
            let bounds = match pair {
                Some((o, c)) => motion::pair_object(&app.buf, app.cursor, o, c, lv),
                None => motion::any_bracket_object(&app.buf, app.cursor, lv),
            }?;
            if around {
                Some(bounds)
            } else {
                motion::inner_of(&app.buf, bounds)
            }
        };
        let current = self.char_sel_bounds();
        let mut lv = levels.max(1);
        let mut result = compute(self, lv);
        // Expand outward while the object does not grow the selection.
        while let (Some((s, e)), Some((cs, ce))) = (result, current) {
            if s < cs || e > ce {
                break;
            }
            lv += 1;
            match compute(self, lv) {
                Some(next) => result = Some(next),
                None => break,
            }
        }
        match result {
            Some((s, e)) => self.select_chars(s, e),
            None => self.message = Some("no bracket object".into()),
        }
    }

    /// Quote objects `i" a" i' a' i` a` iq aq`.
    fn apply_quote_object(&mut self, quote: Option<char>, around: bool) {
        let bounds = match quote {
            Some(q) => motion::quote_bounds(&self.buf, self.cursor, q),
            None => motion::any_quote_bounds(&self.buf, self.cursor),
        };
        let result = bounds.and_then(|b| {
            if around {
                Some(motion::with_surrounding_space(&self.buf, b))
            } else {
                motion::inner_of(&self.buf, b)
            }
        });
        match result {
            Some((s, e)) => self.select_chars(s, e),
            None => self.message = Some("no quote object".into()),
        }
    }

    /// Complete an `i`/`a` text object with its object key.
    fn complete_text_object(&mut self, around: bool, key: char, count: usize) {
        match key {
            'w' => self.apply_text_object(around, false),
            'W' => self.apply_text_object(around, true),
            'b' => self.apply_pair_object(None, around, count),
            '(' | ')' | '[' | ']' | '{' | '}' | 'B' | '<' | '>' => {
                self.apply_pair_object(motion::pair_for(key), around, count)
            }
            '"' | '\'' | '`' => self.apply_quote_object(Some(key), around),
            'q' => self.apply_quote_object(None, around),
            'p' => {
                let (r0, r1) = motion::paragraph_rows(&self.buf, self.cursor.row, around);
                self.select_rows(r0, r1);
            }
            'l' => match motion::line_object(&self.buf, self.cursor.row, around) {
                Some((s, e)) => self.select_chars(s, e),
                None => self.message = Some("blank line".into()),
            },
            'e' => {
                let (r0, r1) = motion::entire_rows(&self.buf, around);
                self.select_rows(r0, r1);
            }
            _ => {}
        }
    }

    fn find(&mut self, target: char, forward: bool, till: bool, count: usize, repeat: bool) {
        let mut p = self.cursor;
        for i in 0..count.max(1) {
            match motion::find_char(&self.buf, p, target, forward, till, repeat || i > 0) {
                Some(n) => p = n,
                None => {
                    self.message = Some(format!("not found: {target}"));
                    return;
                }
            }
        }
        self.goto(p);
    }

    fn repeat_find(&mut self, reverse: bool, count: usize) {
        match self.last_find {
            Some((target, forward, till)) => {
                self.find(target, forward != reverse, till, count, true)
            }
            None => self.message = Some("no previous f/t".into()),
        }
    }

    fn match_bracket(&mut self) {
        match motion::match_bracket(&self.buf, self.cursor) {
            Some(p) => self.goto(p),
            None => self.message = Some("no matching bracket".into()),
        }
    }

    /// `*` / `#`: search for the word under (or after) the cursor.
    fn search_word_under_cursor(&mut self, forward: bool) {
        let row = self.cursor.row;
        let line = self.buf.line(row);
        let start = (self.cursor.col..line.len()).find(|&c| !line.chars[c].is_whitespace());
        let Some(col) = start else {
            self.message = Some("no word under cursor".into());
            return;
        };
        let at = Pos { row, col };
        let (s, e) = motion::word_object(&self.buf, at, false, false);
        let word = self.buf.slice_inclusive(s, e);
        let keyword = word.chars().all(|c| c.is_alphanumeric() || c == '_');
        let pattern = if keyword {
            format!(r"\b{}\b", regex::escape(&word))
        } else {
            regex::escape(&word)
        };
        let dir = if forward {
            Direction::Forward
        } else {
            Direction::Backward
        };
        self.search.run(&self.buf, &pattern, dir);
        match self.search.seek(at, dir, false) {
            Some(m) => self.goto(m.start),
            None => self.message = Some(format!("no match: {word}")),
        }
    }

    /// Row for H/M/L: top, middle or bottom of the visible content.
    fn screen_row(&self, which: char, count: usize) -> usize {
        let rows = self.content_rows();
        let last_visible = (self.top + rows - 1).min(self.buf.last_row());
        let offset = count.saturating_sub(1);
        match which {
            'H' => (self.top + offset).min(last_visible),
            'L' => last_visible.saturating_sub(offset).max(self.top),
            _ => self.top + (last_visible - self.top) / 2,
        }
    }

    /// zz / zt / zb: scroll so the cursor row sits mid-screen, on top or
    /// at the bottom, without moving the cursor.
    fn scroll_cursor_to(&mut self, where_: char) {
        let rows = self.content_rows();
        let row = self.cursor.row;
        let top = match where_ {
            't' => row,
            'b' => (row + 1).saturating_sub(rows),
            _ => row.saturating_sub(rows / 2),
        };
        self.top = top.min(self.max_top());
        self.ensure_visible();
    }

    /// Y: copy the current logical line (or the selected rows) without
    /// leaving copy mode.
    fn yank_lines(&mut self) -> Option<Effect> {
        let (r0, r1) = match self.sel {
            Some((_, anchor)) => (
                self.buf.logical_span(anchor.row.min(self.cursor.row)).0,
                self.buf.logical_span(anchor.row.max(self.cursor.row)).1,
            ),
            None => self.buf.logical_span(self.cursor.row),
        };
        let text = self.buf.slice_lines(r0, r1);
        self.message = Some(format!("copied {} chars", text.chars().count()));
        self.sel = None;
        Some(Effect::Copy(text))
    }

    fn handle_normal(&mut self, key: KeyEvent) -> Option<Effect> {
        use KeyCode::*;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let half = (self.content_rows() / 2).max(1) as isize;
        let page = self.content_rows() as isize;
        if ctrl && key.code == Char('c') {
            self.pending = None;
            self.count = 0;
            self.quit = true;
            return None;
        }
        // The count typed so far applies to this command; `n` is the
        // effective repeat (1 when none was typed).
        let count = self.count;
        let n = count.max(1);
        let repeat = n as isize;
        if let Some(pending) = self.pending.take() {
            self.count = 0;
            if ctrl || key.modifiers.contains(KeyModifiers::ALT) {
                return None;
            }
            match (pending, key.code) {
                (Pending::TextObject { around }, Char(c)) => {
                    self.complete_text_object(around, c, count)
                }
                (Pending::Goto, Char('g')) => {
                    let row = if count > 0 {
                        (count - 1).min(self.buf.last_row())
                    } else {
                        0
                    };
                    self.goto(Pos { row, col: 0 })
                }
                (Pending::Goto, Char('x')) => {
                    if let Some(text) = self.buf.token_at(self.cursor) {
                        return Some(Effect::Open(text));
                    }
                    self.message = Some("no word under cursor".into());
                }
                (Pending::Goto, Char('e')) => {
                    for _ in 0..n {
                        let p = motion::word_end_backward(&self.buf, self.cursor, false);
                        self.goto(p);
                    }
                }
                (Pending::Goto, Char('E')) => {
                    for _ in 0..n {
                        let p = motion::word_end_backward(&self.buf, self.cursor, true);
                        self.goto(p);
                    }
                }
                (Pending::Goto, Char('_')) => {
                    let line = self.buf.line(self.cursor.row);
                    let col = line
                        .chars
                        .iter()
                        .rposition(|c| !c.is_whitespace())
                        .unwrap_or(0);
                    self.goto(Pos {
                        row: self.cursor.row,
                        col,
                    });
                }
                (Pending::Find { forward, till }, Char(c)) => {
                    self.last_find = Some((c, forward, till));
                    self.find(c, forward, till, n, false);
                }
                (Pending::Scroll, Char(c @ ('z' | 't' | 'b'))) => self.scroll_cursor_to(c),
                _ => {}
            }
            return None;
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            if let Char(c) = key.code {
                if let Some(def) = self.patterns.by_key(c).cloned() {
                    self.activate_pattern(&def);
                }
                self.count = 0;
                return None;
            }
        }
        // Count prefix: 1-9 start one, 0 continues one (else `0` is a motion).
        if let Char(d @ '0'..='9') = key.code {
            if !ctrl && (d != '0' || self.count > 0) {
                let digit = d.to_digit(10).unwrap_or(0) as usize;
                self.count = self
                    .count
                    .saturating_mul(10)
                    .saturating_add(digit)
                    .min(1_000_000);
                return None;
            }
        }
        self.count = 0;
        match key.code {
            Char('i') if !ctrl => {
                self.pending = Some(Pending::TextObject { around: false });
                self.count = count;
            }
            Char('a') if !ctrl => {
                self.pending = Some(Pending::TextObject { around: true });
                self.count = count;
            }
            Char('g') if !ctrl => {
                self.pending = Some(Pending::Goto);
                self.count = count;
            }
            Char('z') if !ctrl => self.pending = Some(Pending::Scroll),
            Char(c @ ('f' | 'F' | 't' | 'T')) if !ctrl => {
                self.pending = Some(Pending::Find {
                    forward: c == 'f' || c == 't',
                    till: c == 't' || c == 'T',
                });
                self.count = count;
            }
            Char(';') => self.repeat_find(false, n),
            Char(',') => self.repeat_find(true, n),
            Char('p') if !ctrl => self.mode = Mode::PatternMenu,
            Char('q') if !ctrl => self.quit = true,
            Char('c') if ctrl => self.quit = true,
            Esc => {
                // A typed count is cancelled by Esc (already cleared above).
                if count > 0 {
                } else if self.sel.is_some() {
                    self.sel = None;
                } else {
                    self.quit = true;
                }
            }
            Char('v') if !ctrl => {
                self.sel = match self.sel {
                    Some((SelKind::Char, _)) => None,
                    Some((SelKind::Line, anchor)) => Some((SelKind::Char, anchor)),
                    None => Some((SelKind::Char, self.cursor)),
                };
            }
            Char('V') => {
                self.sel = match self.sel {
                    Some((SelKind::Line, _)) => None,
                    Some((SelKind::Char, anchor)) => Some((SelKind::Line, anchor)),
                    None => Some((SelKind::Line, self.cursor)),
                };
            }
            Char('o') if !ctrl => {
                if let Some((kind, anchor)) = self.sel {
                    self.sel = Some((kind, self.cursor));
                    self.goto(anchor);
                }
            }
            Char('y') if !ctrl => return self.yank(false),
            Char('Y') => return self.yank_lines(),
            Enter => {
                // Copy the selection or the match under the cursor and
                // exit; with nothing to copy, Enter still exits copy mode.
                let eff = self.yank(true);
                if eff.is_none() {
                    self.quit = true;
                }
                return eff;
            }
            Char('/') => self.open_prompt(Direction::Forward),
            Char('?') => self.open_prompt(Direction::Backward),
            Char('n') if !ctrl => {
                for _ in 0..n {
                    self.jump_match(true);
                }
            }
            Char('N') => {
                for _ in 0..n {
                    self.jump_match(false);
                }
            }
            Char('*') => self.search_word_under_cursor(true),
            Char('#') => self.search_word_under_cursor(false),
            Char('S') => {
                self.search.smartcase = !self.search.smartcase;
                let pattern = self.search.pattern.clone();
                let dir = self.search.direction.unwrap_or(Direction::Forward);
                if !pattern.is_empty() {
                    self.search.run(&self.buf, &pattern, dir);
                }
                self.message = Some(format!(
                    "smartcase: {}",
                    if self.search.smartcase { "on" } else { "off" }
                ));
            }
            Char('h') | Left if !ctrl => {
                let col = self.cursor.col.saturating_sub(n);
                self.goto(Pos {
                    row: self.cursor.row,
                    col,
                });
            }
            Char('l') | Right if !ctrl => {
                self.goto(Pos {
                    row: self.cursor.row,
                    col: self.cursor.col + n,
                });
            }
            Char('j') | Down if !ctrl => self.move_vertical(repeat),
            Char('k') | Up if !ctrl => self.move_vertical(-repeat),
            Char('0') | Home => self.goto(Pos {
                row: self.cursor.row,
                col: 0,
            }),
            Char('^') => {
                let col = self.buf.first_non_blank(self.cursor.row);
                self.goto(Pos {
                    row: self.cursor.row,
                    col,
                });
            }
            Char('$') | Char('-') | End => {
                // `-` mirrors the user's Neovim map of `-` to `$`.
                if count > 1 {
                    self.move_vertical(repeat - 1);
                }
                self.goto(Pos {
                    row: self.cursor.row,
                    col: self.buf.max_col(self.cursor.row),
                });
                self.want_col = usize::MAX;
            }
            Char('w') if !ctrl => self.repeat_motion(n, |b, p| motion::word_forward(b, p, false)),
            Char('W') => self.repeat_motion(n, |b, p| motion::word_forward(b, p, true)),
            Char('b') if !ctrl => self.repeat_motion(n, |b, p| motion::word_backward(b, p, false)),
            Char('B') => self.repeat_motion(n, |b, p| motion::word_backward(b, p, true)),
            Char('e') if !ctrl => self.repeat_motion(n, |b, p| motion::word_end(b, p, false)),
            Char('E') => self.repeat_motion(n, |b, p| motion::word_end(b, p, true)),
            // Ctrl-h / Ctrl-l (and Backspace, which many terminals send
            // for Ctrl-h) mirror the user's Neovim word maps.
            Char('l') if ctrl => self.repeat_motion(n, |b, p| motion::word_forward(b, p, false)),
            Char('h') if ctrl => self.repeat_motion(n, |b, p| motion::word_backward(b, p, false)),
            Backspace => self.repeat_motion(n, |b, p| motion::word_backward(b, p, false)),
            Char('{') => {
                for _ in 0..n {
                    let row = motion::para_backward(&self.buf, self.cursor.row);
                    self.goto(Pos { row, col: 0 });
                }
            }
            Char('}') => {
                for _ in 0..n {
                    let row = motion::para_forward(&self.buf, self.cursor.row);
                    self.goto(Pos { row, col: 0 });
                }
            }
            // Ctrl-j / Ctrl-k mirror the user's Neovim paragraph maps.
            Char('j') if ctrl => {
                for _ in 0..n {
                    let row = motion::para_forward(&self.buf, self.cursor.row);
                    self.goto(Pos { row, col: 0 });
                }
            }
            Char('k') if ctrl => {
                for _ in 0..n {
                    let row = motion::para_backward(&self.buf, self.cursor.row);
                    self.goto(Pos { row, col: 0 });
                }
            }
            Char('%') => self.match_bracket(),
            Char('t') if ctrl => self.match_bracket(),
            Char('G') => {
                let row = if count > 0 {
                    (count - 1).min(self.buf.last_row())
                } else {
                    self.buf.last_row()
                };
                self.goto(Pos { row, col: 0 });
            }
            Char(c @ ('H' | 'M' | 'L')) => {
                let row = self.screen_row(c, n);
                let col = self.buf.first_non_blank(row);
                self.goto(Pos { row, col });
            }
            Char('f') if ctrl => self.move_vertical(page * repeat),
            Char('b') if ctrl => self.move_vertical(-page * repeat),
            Char('d') if ctrl => self.move_vertical(if count > 0 { repeat } else { half }),
            Char('u') if ctrl => self.move_vertical(if count > 0 { -repeat } else { -half }),
            PageDown => self.move_vertical(page * repeat),
            PageUp => self.move_vertical(-page * repeat),
            _ => {}
        }
        None
    }

    /// Apply a cursor motion `n` times.
    fn repeat_motion(&mut self, n: usize, step: impl Fn(&Buffer, Pos) -> Pos) {
        for _ in 0..n.max(1) {
            let p = step(&self.buf, self.cursor);
            self.goto(p);
        }
    }

    fn handle_prompt(&mut self, key: KeyEvent) {
        use KeyCode::*;
        let Mode::Prompt {
            direction,
            mut input,
            origin,
            origin_top,
        } = std::mem::replace(&mut self.mode, Mode::Normal)
        else {
            return;
        };
        match key.code {
            Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.quit = true;
                return;
            }
            Esc => {
                // Cancel: restore view and the previous search highlights.
                self.cursor = origin;
                self.top = origin_top;
                let saved = self.saved_pattern.clone();
                let dir = self.search.direction.unwrap_or(direction);
                self.search.run(&self.buf, &saved, dir);
                return;
            }
            Enter => {
                if input.is_empty() {
                    // Repeat the previous search in this direction.
                    let saved = self.saved_pattern.clone();
                    if saved.is_empty() {
                        return;
                    }
                    self.search.run(&self.buf, &saved, direction);
                    match self.search.seek(origin, direction, false) {
                        Some(m) => self.goto(m.start),
                        None => self.message = Some(format!("no match: {}", saved)),
                    }
                } else if self.search.matches.is_empty() {
                    self.message = Some(format!("no match: {}", input.text()));
                }
                return;
            }
            Backspace if input.is_empty() => {
                // Backspace on an empty prompt closes it, tmux-style.
                self.cursor = origin;
                self.top = origin_top;
                let saved = self.saved_pattern.clone();
                let dir = self.search.direction.unwrap_or(direction);
                self.search.run(&self.buf, &saved, dir);
                return;
            }
            _ => {
                if !input.handle_key(key) {
                    self.mode = Mode::Prompt {
                        direction,
                        input,
                        origin,
                        origin_top,
                    };
                    return;
                }
            }
        }
        // Restore the prompt before jumping so the reserved bottom row
        // is accounted for while scrolling to the incremental hit.
        let query = input.text();
        self.mode = Mode::Prompt {
            direction,
            input,
            origin,
            origin_top,
        };
        self.incremental(&query, direction, origin, origin_top);
    }

    pub fn handle_mouse(&mut self, ev: MouseEvent, now: Instant) -> Option<Effect> {
        match ev.kind {
            MouseEventKind::ScrollUp => self.scroll(-3),
            MouseEventKind::ScrollDown => self.scroll(3),
            MouseEventKind::Down(MouseButton::Left) => {
                let count = self.clicks.press(ev.column, ev.row, now);
                if let Some(p) = self.pos_at(ev.column, ev.row) {
                    self.mouse_press(p, count);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(origin) = self.drag_origin {
                    if self.sel.is_none() {
                        self.sel = Some((SelKind::Char, origin));
                    }
                    self.mouse_sel = true;
                    if let Some(p) = self.pos_at(ev.column, ev.row) {
                        self.cursor = p;
                        self.want_col = p.col;
                        self.ensure_visible();
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.drag_origin = None;
                if self.mouse_sel {
                    self.mouse_sel = false;
                    return self.yank_keeping_selection();
                }
            }
            _ => {}
        }
        None
    }

    /// Left press dispatch by click count: 1 moves the cursor and arms a
    /// drag; 2 selects the WORD under the cursor (a whitespace cell reads
    /// as a plain click, so a lone space is never selected); 3 (and every
    /// clamped further click) selects the whole logical line, joining
    /// soft-wrapped rows.
    fn mouse_press(&mut self, p: Pos, count: u8) {
        self.cursor = p;
        self.want_col = p.col;
        self.sel = None;
        self.mouse_sel = false;
        self.drag_origin = None;
        let on_word = self
            .buf
            .line(p.row)
            .chars
            .get(p.col)
            .is_some_and(|c| !c.is_whitespace());
        match count {
            2 if on_word => {
                self.apply_text_object(false, true);
                self.mouse_sel = true;
            }
            3 => {
                let (first, last) = self.buf.logical_span(p.row);
                self.sel = Some((SelKind::Line, Pos { row: first, col: 0 }));
                self.goto(Pos {
                    row: last,
                    col: self.buf.max_col(last),
                });
                self.mouse_sel = true;
            }
            _ => self.drag_origin = Some(p),
        }
    }

    fn pos_at(&self, x: u16, y: u16) -> Option<Pos> {
        let y = y as usize;
        if y >= self.content_rows() {
            return None;
        }
        let row = (self.top + y).min(self.buf.last_row());
        let col = self.buf.line(row).col_at_x(self.hoff + x as usize);
        Some(Pos { row, col })
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

    fn app(text: &str) -> App {
        let mut a = App::new(Buffer::from_text(text), false, None);
        a.set_size(80, 11); // 10 content rows + status
        a
    }

    fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn down(col: u16, row: u16) -> MouseEvent {
        mouse(MouseEventKind::Down(MouseButton::Left), col, row)
    }

    fn drag_to(col: u16, row: u16) -> MouseEvent {
        mouse(MouseEventKind::Drag(MouseButton::Left), col, row)
    }

    fn up(col: u16, row: u16) -> MouseEvent {
        mouse(MouseEventKind::Up(MouseButton::Left), col, row)
    }

    #[test]
    fn starts_at_bottom() {
        let a = app("one\ntwo\nthree");
        assert_eq!(a.cursor, Pos { row: 2, col: 0 });
    }

    #[test]
    fn gx_opens_current_token_without_moving_on_g() {
        for token in [
            "src/main.rs:12:3",
            "https://example.com/a?q=';$HOME",
            "file:///tmp/a%20b.rs#L2",
        ] {
            let mut a = app(&format!("first row\nprefix {token} suffix"));
            a.goto(Pos { row: 1, col: 10 });
            let cursor = a.cursor;
            assert_eq!(a.handle_key(key('g')), None);
            assert_eq!(a.cursor, cursor);
            assert_eq!(a.handle_key(key('x')), Some(Effect::Open(token.into())));
            assert_eq!(a.cursor, cursor);
        }
    }

    #[test]
    fn gx_joins_soft_wrapped_tokens_and_keeps_real_lines_separate() {
        let mut a = App::new(
            Buffer::from_reads(
                "before\nhttps://example.com/path\nafter",
                "before\nhttps://example.\ncom/path\nafter",
            ),
            false,
            None,
        );
        a.goto(Pos { row: 2, col: 2 });
        a.handle_key(key('g'));
        assert_eq!(
            a.handle_key(key('x')),
            Some(Effect::Open("https://example.com/path".into()))
        );
    }

    #[test]
    fn gx_on_blank_does_nothing_and_ignores_selection_elsewhere() {
        let mut a = app("other   src/main.rs:7");
        a.goto(Pos { row: 0, col: 6 });
        a.handle_key(key('g'));
        assert_eq!(a.handle_key(key('x')), None);
        assert_eq!(a.message.as_deref(), Some("no word under cursor"));
        assert!(!a.quit);
        a.sel = Some((SelKind::Char, Pos { row: 0, col: 0 }));
        a.goto(Pos { row: 0, col: 12 });
        a.handle_key(key('g'));
        assert_eq!(
            a.handle_key(key('x')),
            Some(Effect::Open("src/main.rs:7".into()))
        );
    }

    #[test]
    fn gg_goes_to_top_and_cancelled_g_does_not_open() {
        let mut a = app("first\nsecond\nlast");
        a.handle_key(key('g'));
        a.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!a.quit);
        assert_eq!(a.handle_key(key('x')), None);
        assert_eq!(a.cursor.row, 2);
        a.handle_key(key('g'));
        a.handle_key(key('g'));
        assert_eq!(a.cursor, Pos { row: 0, col: 0 });
        a.handle_key(key('g'));
        a.handle_key(ctrl('c'));
        assert!(a.quit);
    }

    #[test]
    fn incremental_search_jumps_and_esc_restores() {
        let mut a = app("alpha\nbeta\ngamma");
        a.handle_key(key('/'));
        a.handle_key(key('b'));
        a.handle_key(key('e'));
        assert_eq!(a.cursor, Pos { row: 1, col: 0 });
        a.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(a.cursor, Pos { row: 2, col: 0 });
        assert!(matches!(a.mode, Mode::Normal));
    }

    #[test]
    fn commit_search_then_n_wraps() {
        let mut a = app("foo\nbar\nfoo\nbaz");
        a.handle_key(key('/'));
        a.handle_key(key('f'));
        a.handle_key(key('o'));
        a.handle_key(key('o'));
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // Search opened at the bottom row wraps to the first match.
        assert_eq!(a.cursor.row, 0);
        a.handle_key(key('n'));
        assert_eq!(a.cursor.row, 2);
        a.handle_key(key('n'));
        assert_eq!(a.cursor.row, 0);
    }

    #[test]
    fn charwise_selection_yank() {
        let mut a = app("hello world");
        a.handle_key(key('g'));
        a.handle_key(key('g'));
        a.handle_key(key('v'));
        for _ in 0..4 {
            a.handle_key(key('l'));
        }
        let eff = a.handle_key(key('y'));
        assert_eq!(eff, Some(Effect::Copy("hello".into())));
        assert!(a.sel.is_none());
        assert!(!a.quit);
    }

    #[test]
    fn linewise_selection_enter_yanks_and_quits() {
        let mut a = app("one\ntwo\nthree");
        a.handle_key(key('g'));
        a.handle_key(key('g'));
        a.handle_key(key('V'));
        a.handle_key(key('j'));
        let eff = a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(eff, Some(Effect::Copy("one\ntwo\n".into())));
        assert!(a.quit);
    }

    #[test]
    fn yank_without_selection_copies_match_under_cursor() {
        let mut a = app("foo bar");
        a.handle_key(key('/'));
        a.handle_key(key('b'));
        a.handle_key(key('a'));
        a.handle_key(key('r'));
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let eff = a.handle_key(key('y'));
        assert_eq!(eff, Some(Effect::Copy("bar".into())));
    }

    #[test]
    fn yank_match_across_soft_wrap_joins_rows() {
        // "foobar" wrapped as "foo"(soft) + "bar"; "oobar" crosses the wrap.
        let mut a = App::new(
            Buffer::from_reads("foobar\nend", "foo\nbar\nend"),
            false,
            None,
        );
        a.set_size(80, 11);
        a.handle_key(key('/'));
        for c in "oobar".chars() {
            a.handle_key(key(c));
        }
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // The cursor sits on the match start; yank rejoins the wrapped rows.
        let eff = a.handle_key(key('y'));
        assert_eq!(eff, Some(Effect::Copy("oobar".into())));
    }

    #[test]
    fn enter_without_selection_or_match_quits() {
        let mut a = app("text");
        let eff = a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(eff.is_none());
        assert!(a.quit);
    }

    #[test]
    fn enter_on_match_without_selection_copies_and_quits() {
        let mut a = app("foo bar");
        a.handle_key(key('/'));
        a.handle_key(key('b'));
        a.handle_key(key('a'));
        a.handle_key(key('r'));
        // Commit the search; the cursor lands on the match.
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // Enter in Normal with no selection copies the match and exits.
        let eff = a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(eff, Some(Effect::Copy("bar".into())));
        assert!(a.quit);
    }

    #[test]
    fn question_mark_searches_backward() {
        let mut a = app("foo\nmid\nfoo\nend");
        a.handle_key(key('G'));
        a.handle_key(key('?'));
        a.handle_key(key('f'));
        a.handle_key(key('o'));
        a.handle_key(key('o'));
        assert_eq!(a.cursor.row, 2);
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.handle_key(key('n'));
        assert_eq!(a.cursor.row, 0);
    }

    #[test]
    fn prompt_left_arrow_edits_mid_query() {
        let mut a = app("alpha\nbeta\ngamma");
        a.handle_key(key('/'));
        for c in "bta".chars() {
            a.handle_key(key(c));
        }
        // No match yet: the view stays at the origin.
        assert_eq!(a.cursor.row, 2);
        let left = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
        a.handle_key(left);
        a.handle_key(left);
        a.handle_key(key('e'));
        assert_eq!(a.cursor, Pos { row: 1, col: 0 });
        let Mode::Prompt { input, .. } = &a.mode else {
            panic!("prompt should stay open");
        };
        assert_eq!(input.text(), "beta");
    }

    #[test]
    fn prompt_home_and_delete_edit_the_query() {
        let mut a = app("xfoo\nfoo");
        a.handle_key(key('/'));
        for c in "xfoo".chars() {
            a.handle_key(key(c));
        }
        a.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        a.handle_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
        assert_eq!(a.search.pattern, "foo");
        assert_eq!(a.search.matches.len(), 2);
    }

    fn alt(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
    }

    #[test]
    fn pattern_menu_selects_url_search() {
        let mut a = app("visit https://x.dev now\nplain tail");
        a.handle_key(key('p'));
        assert!(matches!(a.mode, Mode::PatternMenu));
        a.handle_key(key('u'));
        assert!(matches!(a.mode, Mode::Normal));
        assert_eq!(a.cursor, Pos { row: 0, col: 6 });
        assert_eq!(a.search.matches.len(), 1);
    }

    #[test]
    fn pattern_menu_esc_cancels_and_unknown_key_reports() {
        let mut a = app("text");
        a.handle_key(key('p'));
        a.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(a.mode, Mode::Normal));
        assert!(!a.quit);
        a.handle_key(key('p'));
        a.handle_key(key('z'));
        assert_eq!(a.message.as_deref(), Some("no pattern: z"));
    }

    #[test]
    fn ctrl_c_quits_from_normal() {
        let mut a = app("text");
        a.handle_key(ctrl('c'));
        assert!(a.quit);
    }

    #[test]
    fn ctrl_c_quits_from_prompt() {
        let mut a = app("text");
        a.handle_key(key('/'));
        assert!(matches!(a.mode, Mode::Prompt { .. }));
        a.handle_key(ctrl('c'));
        assert!(a.quit);
    }

    #[test]
    fn ctrl_c_quits_from_pattern_menu() {
        let mut a = app("text");
        a.handle_key(key('p'));
        assert!(matches!(a.mode, Mode::PatternMenu));
        a.handle_key(ctrl('c'));
        assert!(a.quit);
    }

    #[test]
    fn alt_key_activates_pattern_backward_and_n_walks_up() {
        let mut a = app("num 1\nnum 2\nnum 3");
        a.handle_key(alt('d'));
        // Nearest match at or above the cursor (bottom row, col 0).
        assert_eq!(a.cursor, Pos { row: 1, col: 4 });
        a.handle_key(key('n'));
        assert_eq!(a.cursor, Pos { row: 0, col: 4 });
        a.handle_key(key('N'));
        assert_eq!(a.cursor, Pos { row: 1, col: 4 });
    }

    #[test]
    fn text_object_inner_word_selects_word() {
        let mut a = app("foo bar baz");
        a.handle_key(key('g'));
        a.handle_key(key('g'));
        for _ in 0..5 {
            a.handle_key(key('l'));
        }
        a.handle_key(key('i'));
        a.handle_key(key('w'));
        let eff = a.handle_key(key('y'));
        assert_eq!(eff, Some(Effect::Copy("bar".into())));
    }

    #[test]
    fn text_object_after_v_redefines_the_selection() {
        let mut a = app("foo bar baz");
        a.handle_key(key('g'));
        a.handle_key(key('g'));
        for _ in 0..5 {
            a.handle_key(key('l'));
        }
        a.handle_key(key('v'));
        a.handle_key(key('i'));
        a.handle_key(key('w'));
        let eff = a.handle_key(key('y'));
        assert_eq!(eff, Some(Effect::Copy("bar".into())));
    }

    #[test]
    fn text_object_a_word_includes_trailing_space() {
        let mut a = app("foo bar baz");
        a.handle_key(key('g'));
        a.handle_key(key('g'));
        for _ in 0..5 {
            a.handle_key(key('l'));
        }
        a.handle_key(key('a'));
        a.handle_key(key('w'));
        let eff = a.handle_key(key('y'));
        assert_eq!(eff, Some(Effect::Copy("bar ".into())));
    }

    #[test]
    fn text_object_inner_big_word_spans_punct() {
        let mut a = app("foo bar.baz qux");
        a.handle_key(key('g'));
        a.handle_key(key('g'));
        for _ in 0..5 {
            a.handle_key(key('l'));
        }
        a.handle_key(key('i'));
        a.handle_key(key('W'));
        let eff = a.handle_key(key('y'));
        assert_eq!(eff, Some(Effect::Copy("bar.baz".into())));
    }

    #[test]
    fn incomplete_text_object_is_cancelled() {
        let mut a = app("foo bar baz");
        a.handle_key(key('i'));
        // A non-object key cancels the pending object without quitting.
        a.handle_key(key('z'));
        assert!(a.sel.is_none());
        assert!(!a.quit);
    }

    #[test]
    fn smartcase_toggle_reruns_search_case_sensitively() {
        let mut a = app("Foo\nfoo\nFOO");
        a.handle_key(key('/'));
        for c in "foo".chars() {
            a.handle_key(key(c));
        }
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // Lowercase query under smartcase matches every case.
        assert_eq!(a.search.matches.len(), 3);
        a.handle_key(key('S'));
        assert!(!a.search.smartcase);
        // The same query is now case-sensitive.
        assert_eq!(a.search.matches.len(), 1);
        a.handle_key(key('S'));
        assert!(a.search.smartcase);
        assert_eq!(a.search.matches.len(), 3);
    }

    #[test]
    fn custom_pattern_activates_via_alt_key() {
        let mut a = app("id 42\nid abc\nid 7");
        // In config.toml this is: pattern_z = "id \d+"
        a.set_patterns(Patterns::from_config("pattern_z = \"id \\d+\"\n"));
        a.handle_key(alt('z'));
        // Backward seek from the bottom row lands on its own match.
        assert_eq!(a.cursor, Pos { row: 2, col: 0 });
        assert_eq!(a.search.matches.len(), 2);
    }

    #[test]
    fn view_reserves_the_mode_row() {
        let text = (0..30).map(|i| format!("l{i}\n")).collect::<String>();
        let mut a = App::new(Buffer::from_text(&text), false, None);
        a.set_size(80, 10);
        assert_eq!(a.view_rows, 10);
        assert_eq!(a.top, 21, "bottom row is the persistent mode row");
    }

    #[test]
    fn screen_rows_anchor_the_opening_view() {
        let text = (0..30).map(|i| format!("l{i}\n")).collect::<String>();
        let mut a = App::new(Buffer::from_text(&text), false, Some(4));
        a.set_size(80, 10);
        // The 4 rows the pane showed keep their top-of-screen spot;
        // the rows below stay blank like on the real screen.
        assert_eq!(a.top, 26);
        assert_eq!(a.cursor.row, 29);
        // The wheel cannot scroll below the original screen.
        let wheel = |kind| MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let t = Instant::now();
        a.handle_mouse(wheel(MouseEventKind::ScrollDown), t);
        assert_eq!(a.top, 26);
        a.handle_mouse(wheel(MouseEventKind::ScrollUp), t);
        assert_eq!(a.top, 23, "scrolling up into history is free");
    }

    #[test]
    fn double_click_selects_the_word_under_the_cursor() {
        // WORD (whitespace-delimited), so the whole URL is one object.
        let mut a = app("visit https://x.dev now");
        let t = Instant::now();
        a.handle_mouse(down(8, 0), t);
        a.handle_mouse(up(8, 0), t);
        a.handle_mouse(down(8, 0), t);
        assert_eq!(a.sel, Some((SelKind::Char, Pos { row: 0, col: 6 })));
        assert_eq!(a.cursor, Pos { row: 0, col: 18 });
        assert_eq!(a.selection_text().as_deref(), Some("https://x.dev"));
    }

    #[test]
    fn double_click_on_whitespace_is_a_plain_click() {
        let mut a = app("foo bar");
        let t = Instant::now();
        a.handle_mouse(down(3, 0), t);
        a.handle_mouse(up(3, 0), t);
        a.handle_mouse(down(3, 0), t);
        assert!(a.sel.is_none(), "no lone-space selection");
        assert_eq!(a.cursor, Pos { row: 0, col: 3 });
    }

    #[test]
    fn triple_click_selects_the_soft_wrapped_logical_line() {
        // "abcdefghijkl" wrapped over three rows, then a hard line; the
        // click lands on the MIDDLE row of the wrapped group.
        let mut a = App::new(
            Buffer::from_reads("abcdefghijkl\nend", "abcd\nefgh\nijkl\nend"),
            false,
            None,
        );
        a.set_size(80, 11);
        let t = Instant::now();
        for _ in 0..2 {
            a.handle_mouse(down(1, 1), t);
            a.handle_mouse(up(1, 1), t);
        }
        a.handle_mouse(down(1, 1), t);
        assert_eq!(a.sel, Some((SelKind::Line, Pos { row: 0, col: 0 })));
        assert_eq!(a.cursor, Pos { row: 2, col: 3 });
        assert_eq!(a.selection_text().as_deref(), Some("abcdefghijkl\n"));
    }

    #[test]
    fn mouse_click_release_copies_nothing() {
        let mut a = app("hello world");
        let t = Instant::now();
        assert_eq!(a.handle_mouse(down(6, 0), t), None);
        assert_eq!(a.cursor, Pos { row: 0, col: 6 });
        assert_eq!(a.handle_mouse(up(6, 0), t), None);
        assert!(a.sel.is_none());
    }

    #[test]
    fn mouse_drag_release_copies_the_selection() {
        let mut a = app("hello world");
        let t = Instant::now();
        a.handle_mouse(down(0, 0), t);
        a.handle_mouse(drag_to(4, 0), t);
        let eff = a.handle_mouse(up(4, 0), t);
        assert_eq!(eff, Some(Effect::Copy("hello".into())));
        assert_eq!(a.message.as_deref(), Some("copied 5 chars"));
        assert_eq!(
            a.sel,
            Some((SelKind::Char, Pos { row: 0, col: 0 })),
            "the selection stays highlighted after a mouse copy"
        );
        assert!(!a.quit, "mouse copy stays in copy mode");
    }

    #[test]
    fn double_click_release_copies_the_word() {
        let mut a = app("visit https://x.dev now");
        let t = Instant::now();
        a.handle_mouse(down(8, 0), t);
        assert_eq!(
            a.handle_mouse(up(8, 0), t),
            None,
            "first release is a click"
        );
        a.handle_mouse(down(8, 0), t);
        let eff = a.handle_mouse(up(8, 0), t);
        assert_eq!(eff, Some(Effect::Copy("https://x.dev".into())));
        assert_eq!(a.message.as_deref(), Some("copied 13 chars"));
        assert_eq!(
            a.sel,
            Some((SelKind::Char, Pos { row: 0, col: 6 })),
            "the word stays highlighted"
        );
        assert!(!a.quit);
    }

    #[test]
    fn double_click_on_whitespace_release_copies_nothing() {
        let mut a = app("foo bar");
        let t = Instant::now();
        a.handle_mouse(down(3, 0), t);
        a.handle_mouse(up(3, 0), t);
        a.handle_mouse(down(3, 0), t);
        // No selection and, crucially, no fallback copy of a search match:
        // the mouse_sel guard keeps yank()'s match-under-cursor branch off.
        assert_eq!(a.handle_mouse(up(3, 0), t), None);
        assert!(a.sel.is_none());
    }

    #[test]
    fn triple_click_release_copies_the_logical_line() {
        let mut a = App::new(
            Buffer::from_reads("abcdefghijkl\nend", "abcd\nefgh\nijkl\nend"),
            false,
            None,
        );
        a.set_size(80, 11);
        let t = Instant::now();
        for _ in 0..2 {
            a.handle_mouse(down(1, 1), t);
            a.handle_mouse(up(1, 1), t);
        }
        a.handle_mouse(down(1, 1), t);
        let eff = a.handle_mouse(up(1, 1), t);
        assert_eq!(eff, Some(Effect::Copy("abcdefghijkl\n".into())));
        assert_eq!(a.message.as_deref(), Some("copied 13 chars"));
        assert_eq!(
            a.sel,
            Some((SelKind::Line, Pos { row: 0, col: 0 })),
            "the line stays highlighted"
        );
    }

    #[test]
    fn fourth_click_repeats_the_line_copy() {
        let mut a = app("one line");
        let t = Instant::now();
        for _ in 0..3 {
            a.handle_mouse(down(2, 0), t);
            a.handle_mouse(up(2, 0), t);
        }
        // The count clamps at 3: the 4th click re-selects the line ...
        a.handle_mouse(down(2, 0), t);
        assert_eq!(a.sel, Some((SelKind::Line, Pos { row: 0, col: 0 })));
        // ... and its release re-copies it.
        assert_eq!(
            a.handle_mouse(up(2, 0), t),
            Some(Effect::Copy("one line\n".into()))
        );
    }

    #[test]
    fn full_screen_capture_fills_the_view() {
        let text = (0..30).map(|i| format!("l{i}\n")).collect::<String>();
        let mut a = App::new(Buffer::from_text(&text), false, Some(10));
        a.set_size(80, 10);
        assert_eq!(a.top, 21, "anchor clamps to the content rows");
    }

    #[test]
    fn search_hits_stay_above_the_mode_row() {
        let text = (0..30).map(|i| format!("l{i}\n")).collect::<String>();
        let mut a = App::new(Buffer::from_text(&text), false, None);
        a.set_size(80, 10);
        a.handle_key(key('/'));
        for c in "l29".chars() {
            a.handle_key(key(c));
        }
        // The hit on the last buffer row is fully visible right above
        // the mode row.
        assert_eq!(a.cursor.row, 29);
        assert_eq!(a.top, 21);
    }

    #[test]
    fn horizontal_scroll_follows_cursor() {
        let long = "x".repeat(100);
        let mut a = app(&long);
        a.handle_key(key('$'));
        // Cursor cell 99 must be inside [hoff, hoff + 80).
        assert_eq!(a.hoff, 20);
        a.handle_key(key('0'));
        assert_eq!(a.hoff, 0);
    }

    #[test]
    fn horizontal_scroll_keeps_wide_cursor_fully_visible() {
        let wide = "\u{4e2d}".repeat(50);
        let mut a = app(&wide);
        a.handle_key(key('$'));
        // The cursor char occupies display cells [98, 100).
        assert_eq!(a.hoff, 20);
    }

    fn esc() -> KeyEvent {
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
    }

    fn keys(a: &mut App, s: &str) {
        for c in s.chars() {
            a.handle_key(key(c));
        }
    }

    #[test]
    fn inner_paren_object_selects_contents_and_count_expands() {
        //          0123456789012345
        let mut a = app("f(a, g(b, c), d)");
        keys(&mut a, "gg");
        keys(&mut a, "8l"); // on 'b'
        assert_eq!(a.cursor, Pos { row: 0, col: 8 });
        keys(&mut a, "i(");
        assert_eq!(a.selection_text().as_deref(), Some("b, c"));
        // Repeating the object expands to the enclosing pair.
        keys(&mut a, "i(");
        assert_eq!(a.selection_text().as_deref(), Some("a, g(b, c), d"));
        // Nothing further out: the selection stays.
        keys(&mut a, "i(");
        assert_eq!(a.selection_text().as_deref(), Some("a, g(b, c), d"));
        a.handle_key(esc());
        // A count picks the level directly; `a(` includes the delimiters.
        keys(&mut a, "08l2a(");
        assert_eq!(a.selection_text().as_deref(), Some("(a, g(b, c), d)"));
        // The i) spelling works too, as do b (any bracket) and B.
        a.handle_key(esc());
        keys(&mut a, "08li)");
        assert_eq!(a.selection_text().as_deref(), Some("b, c"));
        a.handle_key(esc());
        keys(&mut a, "08lab");
        assert_eq!(a.selection_text().as_deref(), Some("(b, c)"));
    }

    #[test]
    fn bracket_objects_span_lines_and_look_ahead() {
        let mut a = app("x = [\n  1,\n  2\n] end {k}");
        a.goto(Pos { row: 1, col: 3 });
        keys(&mut a, "i[");
        assert_eq!(a.selection_text().as_deref(), Some("  1,\n  2"));
        a.handle_key(esc());
        keys(&mut a, "a[");
        assert_eq!(a.selection_text().as_deref(), Some("[\n  1,\n  2\n]"));
        a.handle_key(esc());
        // Outside any brace on the last row: lookahead finds {k}.
        a.goto(Pos { row: 3, col: 2 });
        keys(&mut a, "i{");
        assert_eq!(a.selection_text().as_deref(), Some("k"));
        a.handle_key(esc());
        keys(&mut a, "iB");
        assert_eq!(a.selection_text().as_deref(), Some("k"));
        a.handle_key(esc());
        keys(&mut a, "i<");
        assert!(a.sel.is_none());
        assert_eq!(a.message.as_deref(), Some("no bracket object"));
    }

    #[test]
    fn quote_objects_inner_around_and_any() {
        //          01234567890123456789
        let mut a = app("run \"a b\" then 'c'");
        keys(&mut a, "gg");
        keys(&mut a, "6l");
        keys(&mut a, "i\"");
        assert_eq!(a.selection_text().as_deref(), Some("a b"));
        a.handle_key(esc());
        keys(&mut a, "a\"");
        assert_eq!(a.selection_text().as_deref(), Some("\"a b\" "));
        a.handle_key(esc());
        // iq picks the nearest quoted string ahead when none covers.
        keys(&mut a, "0iq");
        assert_eq!(a.selection_text().as_deref(), Some("a b"));
        a.handle_key(esc());
        keys(&mut a, "$iq");
        assert_eq!(a.selection_text().as_deref(), Some("c"));
        a.handle_key(esc());
        keys(&mut a, "i'");
        assert_eq!(a.selection_text().as_deref(), Some("c"));
        a.handle_key(esc());
        keys(&mut a, "0i`");
        assert!(a.sel.is_none());
        assert_eq!(a.message.as_deref(), Some("no quote object"));
    }

    #[test]
    fn paragraph_line_and_entire_objects() {
        let mut a = app("a\nb\n\nc\n  d  \n\ne");
        a.goto(Pos { row: 0, col: 0 });
        keys(&mut a, "ip");
        assert_eq!(a.selection_text().as_deref(), Some("a\nb\n"));
        keys(&mut a, "ap");
        assert_eq!(a.selection_text().as_deref(), Some("a\nb\n\n"));
        a.handle_key(esc());
        a.goto(Pos { row: 4, col: 3 });
        keys(&mut a, "il");
        assert_eq!(a.selection_text().as_deref(), Some("d"));
        keys(&mut a, "al");
        assert_eq!(a.selection_text().as_deref(), Some("  d  "));
        keys(&mut a, "ie");
        assert_eq!(
            a.selection_text().as_deref(),
            Some("a\nb\n\nc\n  d  \n\ne\n")
        );
        a.handle_key(esc());
        a.goto(Pos { row: 2, col: 0 });
        keys(&mut a, "il");
        assert!(a.sel.is_none());
        assert_eq!(a.message.as_deref(), Some("blank line"));
    }

    #[test]
    fn counts_apply_to_motions_and_zero_is_still_line_start() {
        let mut a = app("one two three four\nx\ny\nz");
        keys(&mut a, "gg");
        keys(&mut a, "3w");
        assert_eq!(a.cursor, Pos { row: 0, col: 14 });
        keys(&mut a, "2b");
        assert_eq!(a.cursor, Pos { row: 0, col: 4 });
        keys(&mut a, "10l");
        assert_eq!(a.cursor, Pos { row: 0, col: 14 });
        keys(&mut a, "0");
        assert_eq!(a.cursor, Pos { row: 0, col: 0 });
        keys(&mut a, "2j");
        assert_eq!(a.cursor.row, 2);
        keys(&mut a, "4G");
        assert_eq!(a.cursor.row, 3);
        keys(&mut a, "2gg");
        assert_eq!(a.cursor.row, 1);
        keys(&mut a, "G");
        assert_eq!(a.cursor.row, 3);
        // Esc with only a count typed clears it without quitting.
        keys(&mut a, "3");
        a.handle_key(esc());
        assert!(!a.quit);
        keys(&mut a, "gg");
        assert_eq!(a.cursor.row, 0);
    }

    #[test]
    fn big_word_motions_and_ge() {
        let mut a = app("foo bar.baz qux");
        keys(&mut a, "gg");
        keys(&mut a, "W");
        assert_eq!(a.cursor.col, 4);
        keys(&mut a, "E");
        assert_eq!(a.cursor.col, 10);
        keys(&mut a, "W");
        assert_eq!(a.cursor.col, 12);
        keys(&mut a, "B");
        assert_eq!(a.cursor.col, 4);
        keys(&mut a, "ge");
        assert_eq!(a.cursor.col, 2);
        keys(&mut a, "$gE");
        assert_eq!(a.cursor.col, 10);
        keys(&mut a, "g_");
        assert_eq!(a.cursor.col, 14);
    }

    #[test]
    fn find_till_and_repeat() {
        //          0123456789
        let mut a = app("a,b,c,d");
        keys(&mut a, "gg");
        keys(&mut a, "f,");
        assert_eq!(a.cursor.col, 1);
        keys(&mut a, ";");
        assert_eq!(a.cursor.col, 3);
        keys(&mut a, ",");
        assert_eq!(a.cursor.col, 1);
        keys(&mut a, "2;");
        assert_eq!(a.cursor.col, 5);
        keys(&mut a, "0t,");
        assert_eq!(a.cursor.col, 0);
        keys(&mut a, ";");
        assert_eq!(a.cursor.col, 2, "repeated till skips the adjacent hit");
        keys(&mut a, "$Fa");
        assert_eq!(a.cursor.col, 0);
        keys(&mut a, "fz");
        assert_eq!(a.message.as_deref(), Some("not found: z"));
        // v f, selects through the comma.
        keys(&mut a, "0vf,");
        assert_eq!(a.selection_text().as_deref(), Some("a,"));
    }

    #[test]
    fn percent_and_ctrl_t_match_brackets() {
        let mut a = app("if (a[0] == b) {\n  x\n}");
        keys(&mut a, "gg");
        keys(&mut a, "%");
        assert_eq!(a.cursor, Pos { row: 0, col: 13 });
        keys(&mut a, "%");
        assert_eq!(a.cursor, Pos { row: 0, col: 3 });
        keys(&mut a, "$");
        a.handle_key(ctrl('t'));
        assert_eq!(a.cursor, Pos { row: 2, col: 0 });
        keys(&mut a, "v%");
        assert_eq!(a.selection_text().as_deref(), Some("{\n  x\n}"));
    }

    #[test]
    fn o_swaps_selection_ends_and_v_big_v_switch_kinds_keeping_anchor() {
        let mut a = app("one two\nthree");
        keys(&mut a, "gg");
        keys(&mut a, "wve");
        assert_eq!(a.selection_text().as_deref(), Some("two"));
        keys(&mut a, "o");
        assert_eq!(a.cursor, Pos { row: 0, col: 4 });
        assert_eq!(a.sel, Some((SelKind::Char, Pos { row: 0, col: 6 })));
        keys(&mut a, "V");
        assert_eq!(a.sel, Some((SelKind::Line, Pos { row: 0, col: 6 })));
        assert_eq!(a.selection_text().as_deref(), Some("one two\n"));
        keys(&mut a, "v");
        assert_eq!(a.sel, Some((SelKind::Char, Pos { row: 0, col: 6 })));
        keys(&mut a, "v");
        assert!(a.sel.is_none());
        keys(&mut a, "o");
        assert!(!a.quit);
    }

    #[test]
    fn capital_y_copies_the_line_or_selected_rows() {
        let mut a = App::new(
            Buffer::from_reads("abcdefgh\nok\nend", "abcd\nefgh\nok\nend"),
            false,
            None,
        );
        a.set_size(80, 11);
        a.goto(Pos { row: 1, col: 1 });
        assert_eq!(
            a.handle_key(key('Y')),
            Some(Effect::Copy("abcdefgh\n".into()))
        );
        assert!(!a.quit);
        keys(&mut a, "vj");
        assert_eq!(
            a.handle_key(key('Y')),
            Some(Effect::Copy("abcdefgh\nok\n".into()))
        );
        assert!(a.sel.is_none());
    }

    #[test]
    fn star_and_hash_search_the_word_under_cursor() {
        let mut a = app("foo foobar foo\nfoo");
        keys(&mut a, "gg");
        keys(&mut a, "*");
        // Whole-word: skips "foobar".
        assert_eq!(a.cursor, Pos { row: 0, col: 11 });
        keys(&mut a, "n");
        assert_eq!(a.cursor, Pos { row: 1, col: 0 });
        keys(&mut a, "#");
        assert_eq!(a.cursor, Pos { row: 0, col: 11 });
        keys(&mut a, "n");
        assert_eq!(a.cursor, Pos { row: 0, col: 0 });
        // Yank without a selection copies the match under the cursor.
        assert_eq!(a.handle_key(key('y')), Some(Effect::Copy("foo".into())));
    }

    #[test]
    fn hml_and_z_scrolling() {
        let text = (0..30).map(|i| format!("l{i}\n")).collect::<String>();
        let mut a = App::new(Buffer::from_text(&text), false, None);
        a.set_size(80, 11); // 10 content rows: top is 20, rows 20..=29
        keys(&mut a, "H");
        assert_eq!(a.cursor.row, 20);
        keys(&mut a, "L");
        assert_eq!(a.cursor.row, 29);
        keys(&mut a, "M");
        assert_eq!(a.cursor.row, 24);
        keys(&mut a, "3H");
        assert_eq!(a.cursor.row, 22);
        keys(&mut a, "2L");
        assert_eq!(a.cursor.row, 28);
        keys(&mut a, "15Gzt");
        assert_eq!(a.top, 14);
        keys(&mut a, "zb");
        assert_eq!(a.top, 5);
        keys(&mut a, "zz");
        assert_eq!(a.top, 9);
        // zt near the bottom cannot scroll past the bottom-most view.
        keys(&mut a, "Gzt");
        assert_eq!(a.top, 20);
    }

    #[test]
    fn personal_neovim_maps_dash_ctrl_hjkl() {
        let mut a = app("one two\n\nthree four");
        keys(&mut a, "gg");
        keys(&mut a, "-");
        assert_eq!(a.cursor.col, 6);
        a.handle_key(ctrl('h'));
        assert_eq!(a.cursor.col, 4);
        a.handle_key(ctrl('l'));
        assert_eq!(a.cursor, Pos { row: 2, col: 0 });
        a.handle_key(ctrl('k'));
        assert_eq!(a.cursor.row, 1);
        a.handle_key(ctrl('j'));
        assert_eq!(a.cursor.row, 2);
        a.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(a.cursor, Pos { row: 0, col: 4 });
    }

    #[test]
    fn want_col_is_kept_across_short_lines() {
        let mut a = app("longline\nab\nlongline");
        a.handle_key(key('g'));
        a.handle_key(key('g'));
        a.handle_key(key('$'));
        a.handle_key(key('j'));
        assert_eq!(a.cursor.col, 1);
        a.handle_key(key('j'));
        assert_eq!(a.cursor.col, 7);
    }
}
