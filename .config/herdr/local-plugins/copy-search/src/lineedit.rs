use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Single-line input with a movable cursor, shared by the search
/// prompt and the extract query. Positions are char indices (a wide
/// char is one position), matching the buffer's Pos.col convention.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineEdit {
    chars: Vec<char>,
    /// Insertion point in [0, chars.len()].
    cursor: usize,
}

impl LineEdit {
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Text before the cursor.
    pub fn before_text(&self) -> String {
        self.chars[..self.cursor].iter().collect()
    }

    /// The char under the cursor; None when the cursor is at the end.
    pub fn under(&self) -> Option<char> {
        self.chars.get(self.cursor).copied()
    }

    /// Text after the char under the cursor.
    pub fn after_text(&self) -> String {
        self.chars
            .get(self.cursor + 1..)
            .unwrap_or(&[])
            .iter()
            .collect()
    }

    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.chars.len());
    }

    /// Move to the start of the current or previous word (Ctrl+Left):
    /// skip any whitespace left of the cursor, then the run of
    /// non-whitespace before that. Whitespace-only boundaries, matching
    /// delete_word_back so "word" means the same for Ctrl-W and Ctrl+Left.
    fn word_left(&mut self) {
        let mut i = self.cursor;
        while i > 0 && self.chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !self.chars[i - 1].is_whitespace() {
            i -= 1;
        }
        self.cursor = i;
    }

    /// Move past the end of the current or next word (Ctrl+Right): skip
    /// any whitespace right of the cursor, then the run of non-whitespace
    /// after that. Symmetric with word_left.
    fn word_right(&mut self) {
        let len = self.chars.len();
        let mut i = self.cursor;
        while i < len && self.chars[i].is_whitespace() {
            i += 1;
        }
        while i < len && !self.chars[i].is_whitespace() {
            i += 1;
        }
        self.cursor = i;
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.chars.len();
    }

    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
    }

    /// Kill from the cursor to the end of the line (readline Ctrl-K).
    fn kill_to_end(&mut self) {
        self.chars.truncate(self.cursor);
    }

    /// Delete the word before the cursor, unix-word-rubout style (readline
    /// Ctrl-W): drop any whitespace right before the cursor, then the run
    /// of non-whitespace before that.
    fn delete_word_back(&mut self) {
        let mut i = self.cursor;
        while i > 0 && self.chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !self.chars[i - 1].is_whitespace() {
            i -= 1;
        }
        self.chars.drain(i..self.cursor);
        self.cursor = i;
    }

    /// Apply an editing or movement key; false when the key is not an
    /// editing key so the caller can run its own bindings.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('u') if ctrl => self.clear(),
            KeyCode::Char('a') if ctrl => self.home(),
            KeyCode::Char('e') if ctrl => self.end(),
            KeyCode::Char('b') if ctrl => self.left(),
            KeyCode::Char('f') if ctrl => self.right(),
            KeyCode::Char('d') if ctrl => self.delete(),
            KeyCode::Char('h') if ctrl => self.backspace(),
            KeyCode::Char('w') if ctrl => self.delete_word_back(),
            KeyCode::Char('k') if ctrl => self.kill_to_end(),
            KeyCode::Char(c) if !ctrl => self.insert(c),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left if ctrl => self.word_left(),
            KeyCode::Right if ctrl => self.word_right(),
            KeyCode::Left => self.left(),
            KeyCode::Right => self.right(),
            KeyCode::Home => self.home(),
            KeyCode::End => self.end(),
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(text: &str) -> LineEdit {
        let mut le = LineEdit::default();
        for c in text.chars() {
            le.insert(c);
        }
        le
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn insert_appends_at_the_end() {
        let le = edit("abc");
        assert_eq!(le.text(), "abc");
        assert_eq!(le.cursor(), 3);
        assert_eq!(le.under(), None);
    }

    #[test]
    fn left_then_insert_edits_the_middle() {
        let mut le = edit("ac");
        le.left();
        le.insert('b');
        assert_eq!(le.text(), "abc");
        assert_eq!(le.cursor(), 2);
        assert_eq!(le.under(), Some('c'));
    }

    #[test]
    fn backspace_removes_before_cursor_only() {
        let mut le = edit("abc");
        le.home();
        le.backspace();
        assert_eq!(le.text(), "abc", "backspace at start is a no-op");
        le.end();
        le.backspace();
        assert_eq!(le.text(), "ab");
    }

    #[test]
    fn delete_removes_under_cursor() {
        let mut le = edit("abc");
        le.home();
        le.delete();
        assert_eq!(le.text(), "bc");
        le.end();
        le.delete();
        assert_eq!(le.text(), "bc", "delete at end is a no-op");
    }

    #[test]
    fn movement_clamps_at_both_ends() {
        let mut le = edit("ab");
        le.right();
        assert_eq!(le.cursor(), 2);
        le.home();
        le.left();
        assert_eq!(le.cursor(), 0);
    }

    #[test]
    fn split_accessors_partition_the_text() {
        let mut le = edit("abcd");
        le.left();
        le.left();
        assert_eq!(le.before_text(), "ab");
        assert_eq!(le.under(), Some('c'));
        assert_eq!(le.after_text(), "d");
    }

    #[test]
    fn handle_key_maps_editing_keys() {
        let mut le = edit("ac");
        assert!(le.handle_key(key(KeyCode::Left)));
        assert!(le.handle_key(key(KeyCode::Char('b'))));
        assert_eq!(le.text(), "abc");
        assert!(le.handle_key(key(KeyCode::Home)));
        assert!(le.handle_key(key(KeyCode::Delete)));
        assert_eq!(le.text(), "bc");
        assert!(le.handle_key(ctrl('e')));
        assert_eq!(le.cursor(), 2);
        assert!(le.handle_key(ctrl('a')));
        assert_eq!(le.cursor(), 0);
        assert!(le.handle_key(ctrl('u')));
        assert!(le.is_empty());
    }

    #[test]
    fn ctrl_w_deletes_word_back() {
        let mut le = edit("foo bar");
        assert!(le.handle_key(ctrl('w')));
        assert_eq!(le.text(), "foo ");
        // Trailing whitespace before the cursor is eaten with the word.
        assert!(le.handle_key(ctrl('w')));
        assert_eq!(le.text(), "");
        // Mid-line: only the word left of the cursor goes.
        let mut le = edit("alpha beta");
        le.left(); // cursor before the last char
        le.left();
        le.left();
        le.left(); // cursor right after "alpha "
        assert!(le.handle_key(ctrl('w')));
        assert_eq!(le.text(), "beta");
    }

    #[test]
    fn ctrl_k_kills_to_end() {
        let mut le = edit("abcd");
        le.home();
        le.right();
        le.right();
        assert!(le.handle_key(ctrl('k')));
        assert_eq!(le.text(), "ab");
        assert_eq!(le.cursor(), 2);
    }

    #[test]
    fn ctrl_b_and_f_move_by_char() {
        let mut le = edit("ab");
        assert!(le.handle_key(ctrl('b')));
        assert_eq!(le.cursor(), 1);
        assert!(le.handle_key(ctrl('f')));
        assert_eq!(le.cursor(), 2);
    }

    #[test]
    fn ctrl_left_and_right_jump_by_word() {
        // Whitespace-only boundaries: "foo.bar" is a single word.
        let mut le = edit("foo.bar baz");
        assert_eq!(le.cursor(), 11);
        assert!(le.handle_key(ctrl_key(KeyCode::Left)));
        assert_eq!(le.cursor(), 8, "left lands at the start of 'baz'");
        assert!(le.handle_key(ctrl_key(KeyCode::Left)));
        assert_eq!(le.cursor(), 0, "left skips the space and all of 'foo.bar'");

        assert!(le.handle_key(ctrl_key(KeyCode::Right)));
        assert_eq!(le.cursor(), 7, "right lands just past 'foo.bar'");
        assert!(le.handle_key(ctrl_key(KeyCode::Right)));
        assert_eq!(le.cursor(), 11, "right skips the space and all of 'baz'");
    }

    #[test]
    fn word_jump_clamps_and_crosses_edge_whitespace() {
        // Trailing/leading whitespace is skipped before the word run.
        let mut le = edit("  hi  ");
        assert!(le.handle_key(ctrl_key(KeyCode::Left)));
        assert_eq!(
            le.cursor(),
            2,
            "left from the end reaches the start of 'hi'"
        );
        assert!(le.handle_key(ctrl_key(KeyCode::Left)));
        assert_eq!(le.cursor(), 0, "another left clamps at the start");

        assert!(le.handle_key(ctrl_key(KeyCode::Right)));
        assert_eq!(
            le.cursor(),
            4,
            "right from the start reaches the end of 'hi'"
        );
        assert!(le.handle_key(ctrl_key(KeyCode::Right)));
        assert_eq!(le.cursor(), 6, "another right clamps at the end");
    }

    #[test]
    fn ctrl_d_deletes_forward_and_ctrl_h_backspaces() {
        let mut le = edit("abc");
        le.home();
        assert!(le.handle_key(ctrl('d')));
        assert_eq!(le.text(), "bc");
        le.end();
        assert!(le.handle_key(ctrl('h')));
        assert_eq!(le.text(), "b");
    }

    #[test]
    fn handle_key_rejects_non_editing_keys() {
        let mut le = edit("a");
        assert!(!le.handle_key(key(KeyCode::Enter)));
        assert!(!le.handle_key(key(KeyCode::Esc)));
        assert!(!le.handle_key(ctrl('t')));
        assert_eq!(le.text(), "a");
    }
}
