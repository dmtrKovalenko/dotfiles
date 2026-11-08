use crate::buffer::{Buffer, Pos};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Punct,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

/// WORD class: whitespace vs everything else.
fn big_class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else {
        Class::Word
    }
}

fn char_at(buf: &Buffer, p: Pos) -> Option<char> {
    buf.lines.get(p.row)?.chars.get(p.col).copied()
}

/// Line breaks and empty lines read as whitespace, like vi word motions.
/// `big` selects WORD classes (any non-blank run is one word).
fn class_at(buf: &Buffer, p: Pos, big: bool) -> Class {
    match char_at(buf, p) {
        Some(c) if big => big_class(c),
        Some(c) => class(c),
        None => Class::Space,
    }
}

/// Steps through a virtual position at col == len (the newline), which
/// reads as whitespace via class_at, so word runs break at line ends.
/// A soft-wrapped row has no real newline, so that virtual position is
/// skipped and the wrapped word reads as one continuous run.
fn advance(buf: &Buffer, p: Pos) -> Option<Pos> {
    let len = buf.line(p.row).len();
    if p.col < len {
        let next = p.col + 1;
        if next == len && p.row < buf.last_row() && buf.line(p.row).soft_wrap {
            return Some(Pos {
                row: p.row + 1,
                col: 0,
            });
        }
        return Some(Pos {
            row: p.row,
            col: next,
        });
    }
    if p.row < buf.last_row() {
        return Some(Pos {
            row: p.row + 1,
            col: 0,
        });
    }
    None
}

/// Mirror of `advance`: a hard break exposes the previous row's virtual
/// newline (reads as whitespace, so words split there); a soft wrap
/// skips it, landing on the previous row's last char so the wrapped
/// word stays one run.
fn retreat(buf: &Buffer, p: Pos) -> Option<Pos> {
    if p.col > 0 {
        return Some(Pos {
            row: p.row,
            col: p.col - 1,
        });
    }
    if p.row > 0 {
        let row = p.row - 1;
        let col = if buf.line(row).soft_wrap {
            buf.max_col(row)
        } else {
            buf.line(row).len()
        };
        return Some(Pos { row, col });
    }
    None
}

/// Next real character position after `p` (skips empty rows and the
/// virtual newline), or None at the end of the buffer.
pub fn next_char(buf: &Buffer, p: Pos) -> Option<Pos> {
    let mut row = p.row;
    let mut col = p.col + 1;
    loop {
        if col < buf.line(row).len() {
            return Some(Pos { row, col });
        }
        if row >= buf.last_row() {
            return None;
        }
        row += 1;
        col = 0;
    }
}

/// Previous real character position before `p`, or None at the start.
pub fn prev_char(buf: &Buffer, p: Pos) -> Option<Pos> {
    let mut row = p.row;
    let mut col = p.col.min(buf.line(row).len());
    loop {
        if col > 0 {
            return Some(Pos { row, col: col - 1 });
        }
        if row == 0 {
            return None;
        }
        row -= 1;
        col = buf.line(row).len();
    }
}

/// w / W: start of the next word.
pub fn word_forward(buf: &Buffer, p: Pos, big: bool) -> Pos {
    let mut cur = p;
    let start = class_at(buf, cur, big);
    if start != Class::Space {
        loop {
            match advance(buf, cur) {
                Some(n) => {
                    let moved_class = class_at(buf, n, big);
                    cur = n;
                    if moved_class != start {
                        break;
                    }
                }
                None => return buf.clamp(cur),
            }
        }
    }
    while class_at(buf, cur, big) == Class::Space {
        match advance(buf, cur) {
            Some(n) => cur = n,
            None => return buf.clamp(cur),
        }
    }
    cur
}

/// b / B: start of the current or previous word.
pub fn word_backward(buf: &Buffer, p: Pos, big: bool) -> Pos {
    let mut cur = match retreat(buf, p) {
        Some(n) => n,
        None => return p,
    };
    while class_at(buf, cur, big) == Class::Space {
        match retreat(buf, cur) {
            Some(n) => cur = n,
            None => return cur,
        }
    }
    let cls = class_at(buf, cur, big);
    loop {
        match retreat(buf, cur) {
            Some(n) => {
                if class_at(buf, n, big) != cls {
                    return cur;
                }
                cur = n;
            }
            None => return cur,
        }
    }
}

/// e / E: end of the current or next word.
pub fn word_end(buf: &Buffer, p: Pos, big: bool) -> Pos {
    let mut cur = match advance(buf, p) {
        Some(n) => n,
        None => return p,
    };
    while class_at(buf, cur, big) == Class::Space {
        match advance(buf, cur) {
            Some(n) => cur = n,
            None => return buf.clamp(cur),
        }
    }
    let cls = class_at(buf, cur, big);
    loop {
        match advance(buf, cur) {
            Some(n) => {
                if class_at(buf, n, big) != cls {
                    return cur;
                }
                cur = n;
            }
            None => return cur,
        }
    }
}

/// ge / gE: end of the previous word.
pub fn word_end_backward(buf: &Buffer, p: Pos, big: bool) -> Pos {
    let mut cur = p;
    let start = class_at(buf, cur, big);
    if start != Class::Space {
        // Back to the start of the current run, then step off it.
        loop {
            match retreat(buf, cur) {
                Some(n) if class_at(buf, n, big) == start => cur = n,
                _ => break,
            }
        }
        match retreat(buf, cur) {
            Some(n) => cur = n,
            None => return buf.clamp(cur),
        }
    }
    while class_at(buf, cur, big) == Class::Space {
        match retreat(buf, cur) {
            Some(n) => cur = n,
            None => break,
        }
    }
    buf.clamp(cur)
}

/// Inclusive char-index run of the class at `col` within `chars`.
/// `chars` must be non-empty; `col` is clamped to the last index.
fn run_bounds(chars: &[char], col: usize, cls: impl Fn(char) -> u8) -> (usize, usize) {
    let last = chars.len() - 1;
    let col = col.min(last);
    let target = cls(chars[col]);
    let mut s = col;
    while s > 0 && cls(chars[s - 1]) == target {
        s -= 1;
    }
    let mut e = col;
    while e < last && cls(chars[e + 1]) == target {
        e += 1;
    }
    (s, e)
}

/// The logical line (soft-wrap group) containing `row`, flattened to one
/// char vector plus the `(row, offset)` table needed to map indices back.
struct Flat {
    chars: Vec<char>,
    starts: Vec<(usize, usize)>,
}

impl Flat {
    fn new(buf: &Buffer, row: usize) -> Self {
        let (r0, r1) = buf.logical_span(row);
        let mut chars: Vec<char> = Vec::new();
        let mut starts: Vec<(usize, usize)> = Vec::new();
        for r in r0..=r1 {
            starts.push((r, chars.len()));
            chars.extend(buf.line(r).chars.iter().copied());
        }
        Flat { chars, starts }
    }

    /// Flat index of a buffer position inside this logical line.
    fn index(&self, p: Pos) -> usize {
        let base = self
            .starts
            .iter()
            .find(|(row, _)| *row == p.row)
            .map_or(0, |(_, off)| *off);
        base + p.col
    }

    /// Map a flat index back to its buffer Pos.
    fn pos(&self, i: usize) -> Pos {
        let mut chosen = self.starts[0];
        for &pair in &self.starts {
            if pair.1 <= i {
                chosen = pair;
            } else {
                break;
            }
        }
        Pos {
            row: chosen.0,
            col: i - chosen.1,
        }
    }
}

/// vi word/WORD text object around `p`, returning the inclusive
/// (start, end) positions. `around` (aw/aW) extends over adjacent
/// whitespace (trailing preferred, else leading; on a space run it
/// takes the following word instead); `big` (iW/aW) treats any
/// non-whitespace run as one WORD. Operates over the whole logical line
/// (soft-wrapped rows flattened), so a wrapped word is one object and
/// the returned positions may land on different rows.
pub fn word_object(buf: &Buffer, p: Pos, around: bool, big: bool) -> (Pos, Pos) {
    let flat = Flat::new(buf, p.row);
    if flat.chars.is_empty() {
        let z = Pos { row: p.row, col: 0 };
        return (z, z);
    }
    let col = flat.index(p);
    // 0 = whitespace; small words split word/punct (1/2), WORDs do not.
    let small = |c: char| match class(c) {
        Class::Space => 0u8,
        Class::Word => 1,
        Class::Punct => 2,
    };
    let big_cls = |c: char| u8::from(class(c) != Class::Space);
    let (mut s, mut e) = if big {
        run_bounds(&flat.chars, col, big_cls)
    } else {
        run_bounds(&flat.chars, col, small)
    };
    if around {
        let last = flat.chars.len() - 1;
        let on_space = class(flat.chars[col.min(last)]) == Class::Space;
        if on_space {
            // Whitespace object: take the run of the word that follows.
            if e < last {
                let (_, e2) = if big {
                    run_bounds(&flat.chars, e + 1, big_cls)
                } else {
                    run_bounds(&flat.chars, e + 1, small)
                };
                e = e2;
            }
        } else if e < last && class(flat.chars[e + 1]) == Class::Space {
            // Prefer trailing whitespace.
            while e < last && class(flat.chars[e + 1]) == Class::Space {
                e += 1;
            }
        } else {
            // No trailing whitespace: fall back to leading whitespace.
            while s > 0 && class(flat.chars[s - 1]) == Class::Space {
                s -= 1;
            }
        }
    }
    (flat.pos(s), flat.pos(e))
}

/// Bracket pairs understood by `%` and the `ib`/`ab` any-bracket object.
pub const BRACKETS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

/// Resolve a text-object key to its delimiter pair: `( ) b` → `()`,
/// `[ ]` → `[]`, `{ } B` → `{}`, `< >` → `<>`.
pub fn pair_for(key: char) -> Option<(char, char)> {
    Some(match key {
        '(' | ')' => ('(', ')'),
        '[' | ']' => ('[', ']'),
        '{' | '}' | 'B' => ('{', '}'),
        '<' | '>' => ('<', '>'),
        _ => return None,
    })
}

/// Scan backward from `from` (inclusive) for the `open` that encloses
/// it, skipping balanced inner pairs.
fn enclosing_open(buf: &Buffer, from: Pos, open: char, close: char) -> Option<Pos> {
    let mut depth = 0usize;
    let mut cur = Some(from);
    while let Some(p) = cur {
        match char_at(buf, p) {
            Some(c) if c == close => depth += 1,
            Some(c) if c == open => {
                if depth == 0 {
                    return Some(p);
                }
                depth -= 1;
            }
            _ => {}
        }
        cur = prev_char(buf, p);
    }
    None
}

/// Scan forward from `from` (inclusive) for the `close` matching an
/// already-seen `open`, skipping balanced inner pairs.
fn matching_close(buf: &Buffer, from: Pos, open: char, close: char) -> Option<Pos> {
    let mut depth = 0usize;
    let mut cur = Some(from);
    while let Some(p) = cur {
        match char_at(buf, p) {
            Some(c) if c == open => depth += 1,
            Some(c) if c == close => {
                if depth == 0 {
                    return Some(p);
                }
                depth -= 1;
            }
            _ => {}
        }
        cur = next_char(buf, p);
    }
    None
}

/// Delimiter positions `(open, close)` of the `levels`-th bracket pair
/// enclosing `p` (1 = innermost), searching the whole buffer with
/// nesting. A cursor on a delimiter belongs to that pair. When nothing
/// encloses the cursor, the first pair opening later on the same logical
/// line is used instead (text-object lookahead).
pub fn pair_bounds(
    buf: &Buffer,
    p: Pos,
    open: char,
    close: char,
    levels: usize,
) -> Option<(Pos, Pos)> {
    let levels = levels.max(1);
    let mut found: Option<Pos> = None;
    let mut from = Some(p);
    // A cursor on the closing delimiter belongs to its pair: start the
    // backward scan just inside it so the pair reads as level 1.
    if char_at(buf, p) == Some(close) {
        from = Some(prev_char(buf, p)?);
    }
    for _ in 0..levels {
        let start = from?;
        let o = enclosing_open(buf, start, open, close)?;
        found = Some(o);
        from = prev_char(buf, o);
    }
    let open_pos = found?;
    let close_pos = matching_close(buf, next_char(buf, open_pos)?, open, close)?;
    Some((open_pos, close_pos))
}

/// Lookahead: the first `open`...`close` pair starting after `p` on the
/// same logical line.
fn pair_lookahead(buf: &Buffer, p: Pos, open: char, close: char) -> Option<(Pos, Pos)> {
    let (_, r1) = buf.logical_span(p.row);
    let mut cur = next_char(buf, p);
    while let Some(q) = cur {
        if q.row > r1 {
            return None;
        }
        if char_at(buf, q) == Some(open) {
            let c = matching_close(buf, next_char(buf, q)?, open, close)?;
            return Some((q, c));
        }
        cur = next_char(buf, q);
    }
    None
}

/// `pair_bounds` with lookahead when the cursor is outside every pair.
pub fn pair_object(
    buf: &Buffer,
    p: Pos,
    open: char,
    close: char,
    levels: usize,
) -> Option<(Pos, Pos)> {
    match pair_bounds(buf, p, open, close, levels) {
        Some(b) => Some(b),
        None if levels <= 1 => pair_lookahead(buf, p, open, close),
        None => None,
    }
}

/// The `levels`-th enclosing pair among `()`, `[]` and `{}` counted
/// across all three kinds (1 = innermost), or, when nothing encloses the
/// cursor, the nearest pair opening later on the line: the mini.ai-style
/// `ib`/`ab` object.
pub fn any_bracket_object(buf: &Buffer, p: Pos, levels: usize) -> Option<(Pos, Pos)> {
    let mut covering: Vec<(Pos, Pos)> = Vec::new();
    for &(o, c) in &BRACKETS {
        let mut lv = 1;
        while let Some(b) = pair_bounds(buf, p, o, c, lv) {
            covering.push(b);
            lv += 1;
        }
    }
    // Innermost first.
    covering.sort_by_key(|a| std::cmp::Reverse(a.0));
    covering.dedup();
    let levels = levels.max(1);
    if let Some(&b) = covering.get(levels - 1) {
        return Some(b);
    }
    if !covering.is_empty() || levels > 1 {
        return None;
    }
    BRACKETS
        .iter()
        .filter_map(|&(o, c)| pair_lookahead(buf, p, o, c))
        .min_by_key(|&(o, _)| o)
}

/// The text strictly between two delimiter positions, or None when the
/// delimiters are adjacent.
pub fn inner_of(buf: &Buffer, (open, close): (Pos, Pos)) -> Option<(Pos, Pos)> {
    let s = next_char(buf, open)?;
    let e = prev_char(buf, close)?;
    (s <= e && s != close && e != open).then_some((s, e))
}

/// Index pairs of unescaped `q` quotes on a flattened line, paired left
/// to right the way vi does. A quote preceded by an odd number of
/// backslashes is skipped.
fn quote_pairs(chars: &[char], q: char) -> Vec<(usize, usize)> {
    let mut idx = Vec::new();
    for (i, &c) in chars.iter().enumerate() {
        if c != q {
            continue;
        }
        let mut bs = 0;
        while bs < i && chars[i - 1 - bs] == '\\' {
            bs += 1;
        }
        if bs % 2 == 0 {
            idx.push(i);
        }
    }
    idx.chunks_exact(2).map(|w| (w[0], w[1])).collect()
}

/// Quote text object for `q` on the logical line: delimiter positions
/// `(open, close)` of the quoted string covering the cursor, else the
/// first one starting after it (vi looks ahead for quotes too).
pub fn quote_bounds(buf: &Buffer, p: Pos, q: char) -> Option<(Pos, Pos)> {
    let flat = Flat::new(buf, p.row);
    if flat.chars.is_empty() {
        return None;
    }
    let col = flat.index(p);
    let pairs = quote_pairs(&flat.chars, q);
    let hit = pairs
        .iter()
        .find(|&&(a, b)| a <= col && col <= b)
        .or_else(|| pairs.iter().find(|&&(a, _)| a > col))?;
    Some((flat.pos(hit.0), flat.pos(hit.1)))
}

/// `iq`/`aq`: the innermost quoted string covering the cursor among
/// `"`, `'` and `` ` ``, else the nearest one starting after it.
pub fn any_quote_bounds(buf: &Buffer, p: Pos) -> Option<(Pos, Pos)> {
    let all: Vec<(Pos, Pos)> = ['"', '\'', '`']
        .iter()
        .filter_map(|&q| quote_bounds(buf, p, q))
        .collect();
    all.iter()
        .copied()
        .filter(|&(a, b)| a <= p && p <= b)
        .max_by_key(|&(a, _)| a)
        .or_else(|| all.iter().copied().min_by_key(|&(a, _)| a))
}

/// Extend a charwise object over adjacent whitespace on its logical
/// line: trailing first, else leading (the `a"` rule).
pub fn with_surrounding_space(buf: &Buffer, (s, e): (Pos, Pos)) -> (Pos, Pos) {
    let flat = Flat::new(buf, e.row);
    let last = flat.chars.len().saturating_sub(1);
    let mut ei = flat.index(e);
    let same_line = buf.logical_span(s.row) == buf.logical_span(e.row);
    if ei < last && flat.chars[ei + 1].is_whitespace() {
        while ei < last && flat.chars[ei + 1].is_whitespace() {
            ei += 1;
        }
        return (s, flat.pos(ei));
    }
    if same_line {
        let mut si = flat.index(s);
        while si > 0 && flat.chars[si - 1].is_whitespace() {
            si -= 1;
        }
        return (flat.pos(si), e);
    }
    (s, e)
}

/// `%`: the bracket matching the one under the cursor; with no bracket
/// under the cursor, the first bracket later on the row is matched (vi
/// behaviour). `<>` are not match pairs, as in vi's default matchpairs.
pub fn match_bracket(buf: &Buffer, p: Pos) -> Option<Pos> {
    let line = buf.line(p.row);
    let start = (p.col..line.len()).find(|&c| {
        let ch = line.chars[c];
        BRACKETS.iter().any(|&(o, cl)| ch == o || ch == cl)
    })?;
    let at = Pos {
        row: p.row,
        col: start,
    };
    let ch = line.chars[start];
    for &(o, c) in &BRACKETS {
        if ch == o {
            return matching_close(buf, next_char(buf, at)?, o, c);
        }
        if ch == c {
            return enclosing_open(buf, prev_char(buf, at)?, o, c);
        }
    }
    None
}

/// f/F/t/T (and `;`/`,` repeats) on the logical line. `till` stops one
/// char short of the hit; when repeating a till that would not move,
/// the next hit is used instead, like vi without `;` in 'cpoptions'.
pub fn find_char(
    buf: &Buffer,
    p: Pos,
    target: char,
    forward: bool,
    till: bool,
    repeat: bool,
) -> Option<Pos> {
    let flat = Flat::new(buf, p.row);
    if flat.chars.is_empty() {
        return None;
    }
    let col = flat.index(p);
    let last = flat.chars.len() - 1;
    let mut skip_adjacent = till && repeat;
    if forward {
        let mut i = col + 1;
        while i <= last {
            if flat.chars[i] == target {
                if skip_adjacent && i == col + 1 {
                    skip_adjacent = false;
                } else {
                    let land = if till { i - 1 } else { i };
                    return Some(flat.pos(land));
                }
            }
            i += 1;
        }
        None
    } else {
        let mut i = col;
        while i > 0 {
            i -= 1;
            if flat.chars[i] == target {
                if skip_adjacent && i + 1 == col {
                    skip_adjacent = false;
                } else {
                    let land = if till { i + 1 } else { i };
                    return Some(flat.pos(land));
                }
            }
        }
        None
    }
}

/// }: next blank line (or the last row).
pub fn para_forward(buf: &Buffer, row: usize) -> usize {
    let mut r = row;
    while r < buf.last_row() && buf.line(r).is_blank() {
        r += 1;
    }
    while r < buf.last_row() && !buf.line(r).is_blank() {
        r += 1;
    }
    r
}

/// {: previous blank line (or row 0).
pub fn para_backward(buf: &Buffer, row: usize) -> usize {
    let mut r = row;
    while r > 0 && buf.line(r).is_blank() {
        r -= 1;
    }
    while r > 0 && !buf.line(r).is_blank() {
        r -= 1;
    }
    r
}

/// Inclusive row range of the run of rows sharing `row`'s blankness.
fn block_bounds(buf: &Buffer, row: usize) -> (usize, usize) {
    let blank = buf.line(row).is_blank();
    let mut r0 = row;
    while r0 > 0 && buf.line(r0 - 1).is_blank() == blank {
        r0 -= 1;
    }
    let mut r1 = row;
    while r1 < buf.last_row() && buf.line(r1 + 1).is_blank() == blank {
        r1 += 1;
    }
    (r0, r1)
}

/// `ip`/`ap`: inclusive row range of the paragraph at `row`. Inner is
/// the block of non-blank rows (or the blank block when on one); around
/// adds the blank rows after it, else the blank rows before it. On a
/// blank block, around adds the following paragraph instead.
pub fn paragraph_rows(buf: &Buffer, row: usize, around: bool) -> (usize, usize) {
    let (r0, r1) = block_bounds(buf, row);
    if !around {
        return (r0, r1);
    }
    if r1 < buf.last_row() {
        let (_, next_end) = block_bounds(buf, r1 + 1);
        return (r0, next_end);
    }
    if r0 > 0 && !buf.line(row).is_blank() {
        let (prev_start, _) = block_bounds(buf, r0 - 1);
        return (prev_start, r1);
    }
    (r0, r1)
}

/// `il`/`al`: the logical line as a charwise object. Around is the whole
/// line without its newline; inner trims leading and trailing blanks.
/// None for a blank line.
pub fn line_object(buf: &Buffer, row: usize, around: bool) -> Option<(Pos, Pos)> {
    let flat = Flat::new(buf, row);
    if flat.chars.is_empty() {
        return None;
    }
    let last = flat.chars.len() - 1;
    if around {
        return Some((flat.pos(0), flat.pos(last)));
    }
    let s = flat.chars.iter().position(|c| !c.is_whitespace())?;
    let e = flat.chars.iter().rposition(|c| !c.is_whitespace())?;
    Some((flat.pos(s), flat.pos(e)))
}

/// `ie`/`ae`: the whole buffer as rows; inner skips leading and
/// trailing blank rows.
pub fn entire_rows(buf: &Buffer, around: bool) -> (usize, usize) {
    if around {
        return (0, buf.last_row());
    }
    let r0 = (0..=buf.last_row())
        .find(|&r| !buf.line(r).is_blank())
        .unwrap_or(0);
    let r1 = (0..=buf.last_row())
        .rev()
        .find(|&r| !buf.line(r).is_blank())
        .unwrap_or(buf.last_row());
    (r0, r1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(row: usize, col: usize) -> Pos {
        Pos { row, col }
    }

    #[test]
    fn word_forward_walks_runs_and_lines() {
        let b = Buffer::from_text("foo  bar.baz\nqux");
        assert_eq!(word_forward(&b, p(0, 0), false), p(0, 5));
        assert_eq!(word_forward(&b, p(0, 5), false), p(0, 8));
        assert_eq!(word_forward(&b, p(0, 8), false), p(0, 9));
        assert_eq!(word_forward(&b, p(0, 9), false), p(1, 0));
        assert_eq!(word_forward(&b, p(1, 0), false), p(1, 2));
    }

    #[test]
    fn big_word_motions_ignore_punctuation() {
        let b = Buffer::from_text("foo  bar.baz qux\nend");
        assert_eq!(word_forward(&b, p(0, 0), true), p(0, 5));
        assert_eq!(word_forward(&b, p(0, 5), true), p(0, 13));
        assert_eq!(word_forward(&b, p(0, 13), true), p(1, 0));
        assert_eq!(word_backward(&b, p(0, 13), true), p(0, 5));
        assert_eq!(word_backward(&b, p(0, 10), true), p(0, 5));
        assert_eq!(word_end(&b, p(0, 5), true), p(0, 11));
        assert_eq!(word_end(&b, p(0, 0), true), p(0, 2));
    }

    #[test]
    fn word_backward_walks_runs_and_lines() {
        let b = Buffer::from_text("foo  bar.baz\nqux");
        assert_eq!(word_backward(&b, p(1, 0), false), p(0, 9));
        assert_eq!(word_backward(&b, p(0, 9), false), p(0, 8));
        assert_eq!(word_backward(&b, p(0, 8), false), p(0, 5));
        assert_eq!(word_backward(&b, p(0, 5), false), p(0, 0));
        assert_eq!(word_backward(&b, p(0, 0), false), p(0, 0));
    }

    #[test]
    fn word_end_stops_at_run_ends() {
        let b = Buffer::from_text("foo bar\nbaz");
        assert_eq!(word_end(&b, p(0, 0), false), p(0, 2));
        assert_eq!(word_end(&b, p(0, 2), false), p(0, 6));
        assert_eq!(word_end(&b, p(0, 6), false), p(1, 2));
    }

    #[test]
    fn word_end_backward_lands_on_previous_word_end() {
        let b = Buffer::from_text("foo bar.baz\nqux");
        // From inside "baz": the previous run is the dot.
        assert_eq!(word_end_backward(&b, p(0, 9), false), p(0, 7));
        // gE treats "bar.baz" as one WORD, so it lands on "foo".
        assert_eq!(word_end_backward(&b, p(0, 9), true), p(0, 2));
        // From the start of the next line, across the hard newline.
        assert_eq!(word_end_backward(&b, p(1, 0), false), p(0, 10));
        // From whitespace: the end of the word before it.
        assert_eq!(word_end_backward(&b, p(0, 3), false), p(0, 2));
        // At the first word, there is nothing before: go to its start.
        assert_eq!(word_end_backward(&b, p(0, 1), false), p(0, 0));
    }

    #[test]
    fn word_motions_span_soft_wrapped_words() {
        // "foobar" wrapped as "foo"(soft) + "bar", then a real newline.
        let b = Buffer::from_reads("foobar\nend", "foo\nbar\nend");
        assert!(b.line(0).soft_wrap);
        assert!(!b.line(1).soft_wrap);
        // w from the wrapped word start skips the whole "foobar".
        assert_eq!(word_forward(&b, p(0, 0), false), p(2, 0));
        // e lands on the last char of the wrapped word (the 'r' of "bar").
        assert_eq!(word_end(&b, p(0, 0), false), p(1, 2));
        // b from the next word returns to the wrapped word start.
        assert_eq!(word_backward(&b, p(2, 0), false), p(0, 0));
        // b from inside the continuation row reaches the same start.
        assert_eq!(word_backward(&b, p(1, 1), false), p(0, 0));
    }

    #[test]
    fn word_backward_respects_hard_newline() {
        // A real newline is a word boundary even between two word chars.
        let b = Buffer::from_text("foo\nbar");
        assert_eq!(word_backward(&b, p(1, 1), false), p(1, 0));
    }

    #[test]
    fn word_motions_skip_empty_lines() {
        let b = Buffer::from_text("foo\n\nbar");
        assert_eq!(word_forward(&b, p(0, 2), false), p(2, 0));
        assert_eq!(word_backward(&b, p(2, 0), false), p(0, 0));
    }

    #[test]
    fn inner_word_spans_the_word_under_cursor() {
        let b = Buffer::from_text("foo bar.baz");
        assert_eq!(word_object(&b, p(0, 5), false, false), (p(0, 4), p(0, 6)));
        assert_eq!(word_object(&b, p(0, 1), false, false), (p(0, 0), p(0, 2)));
        // A lone punctuation run is its own inner word.
        assert_eq!(word_object(&b, p(0, 7), false, false), (p(0, 7), p(0, 7)));
    }

    #[test]
    fn word_object_spans_soft_wrapped_word() {
        // "foobar" wrapped as "foo"(soft) + "bar".
        let b = Buffer::from_reads("foobar\nend", "foo\nbar\nend");
        // iw on the continuation row selects the whole wrapped word.
        assert_eq!(word_object(&b, p(1, 1), false, false), (p(0, 0), p(1, 2)));
        // iw on the first row spans the whole word too.
        assert_eq!(word_object(&b, p(0, 1), false, false), (p(0, 0), p(1, 2)));
    }

    #[test]
    fn inner_big_word_spans_to_whitespace() {
        let b = Buffer::from_text("foo bar.baz qux");
        assert_eq!(word_object(&b, p(0, 5), false, true), (p(0, 4), p(0, 10)));
    }

    #[test]
    fn around_word_includes_trailing_space() {
        let b = Buffer::from_text("foo bar baz");
        assert_eq!(word_object(&b, p(0, 5), true, false), (p(0, 4), p(0, 7)));
    }

    #[test]
    fn around_word_at_line_end_includes_leading_space() {
        let b = Buffer::from_text("foo bar");
        assert_eq!(word_object(&b, p(0, 5), true, false), (p(0, 3), p(0, 6)));
    }

    #[test]
    fn inner_word_on_space_selects_the_space_run() {
        let b = Buffer::from_text("foo   bar");
        assert_eq!(word_object(&b, p(0, 4), false, false), (p(0, 3), p(0, 5)));
    }

    #[test]
    fn around_word_on_space_includes_following_word() {
        let b = Buffer::from_text("foo   bar.baz");
        // small aw stops at the punctuation boundary of the next word.
        assert_eq!(word_object(&b, p(0, 4), true, false), (p(0, 3), p(0, 8)));
    }

    #[test]
    fn word_object_on_empty_line_is_a_point() {
        let b = Buffer::from_text("foo\n\nbar");
        assert_eq!(word_object(&b, p(1, 0), false, false), (p(1, 0), p(1, 0)));
    }

    #[test]
    fn paragraph_motions_find_blank_lines() {
        let b = Buffer::from_text("a\nb\n\nc\nd\n\ne");
        assert_eq!(para_forward(&b, 0), 2);
        assert_eq!(para_forward(&b, 2), 5);
        assert_eq!(para_backward(&b, 6), 5);
        assert_eq!(para_backward(&b, 5), 2);
        assert_eq!(para_backward(&b, 1), 0);
    }

    #[test]
    fn pair_bounds_finds_enclosing_parens_with_nesting() {
        //                     0123456789012345678
        let b = Buffer::from_text("f(a, g(b, c), d) x");
        // Inside the inner call.
        assert_eq!(
            pair_bounds(&b, p(0, 8), '(', ')', 1),
            Some((p(0, 6), p(0, 11)))
        );
        // Second level: the outer call.
        assert_eq!(
            pair_bounds(&b, p(0, 8), '(', ')', 2),
            Some((p(0, 1), p(0, 15)))
        );
        // No third level.
        assert_eq!(pair_bounds(&b, p(0, 8), '(', ')', 3), None);
        // On the opening and closing delimiters themselves.
        assert_eq!(
            pair_bounds(&b, p(0, 6), '(', ')', 1),
            Some((p(0, 6), p(0, 11)))
        );
        assert_eq!(
            pair_bounds(&b, p(0, 11), '(', ')', 1),
            Some((p(0, 6), p(0, 11)))
        );
        // Between the inner pair and the outer close: skips the balanced inner pair.
        assert_eq!(
            pair_bounds(&b, p(0, 14), '(', ')', 1),
            Some((p(0, 1), p(0, 15)))
        );
        // Outside all pairs.
        assert_eq!(pair_bounds(&b, p(0, 17), '(', ')', 1), None);
    }

    #[test]
    fn pair_object_spans_lines_and_looks_ahead() {
        let b = Buffer::from_text("call(\n  one,\n  two\n) tail (x)");
        assert_eq!(
            pair_bounds(&b, p(1, 3), '(', ')', 1),
            Some((p(0, 4), p(3, 0)))
        );
        let inner = inner_of(&b, (p(0, 4), p(3, 0))).unwrap();
        assert_eq!(inner, (p(1, 0), p(2, 4)));
        assert_eq!(b.slice_inclusive(inner.0, inner.1), "  one,\n  two");
        // Cursor before "(x)" on the last row: lookahead picks it.
        assert_eq!(
            pair_object(&b, p(3, 2), '(', ')', 1),
            Some((p(3, 7), p(3, 9)))
        );
        // Lookahead does not cross to another logical line.
        let b2 = Buffer::from_text("none here\n(x)");
        assert_eq!(pair_object(&b2, p(0, 0), '(', ')', 1), None);
    }

    #[test]
    fn inner_of_empty_pair_is_none() {
        let b = Buffer::from_text("f()");
        assert_eq!(inner_of(&b, (p(0, 1), p(0, 2))), None);
        let b = Buffer::from_text("[x]");
        assert_eq!(inner_of(&b, (p(0, 0), p(0, 2))), Some((p(0, 1), p(0, 1))));
    }

    #[test]
    fn any_bracket_picks_the_innermost_cover_then_the_nearest_ahead() {
        //                     0123456789012
        let b = Buffer::from_text("a[b(c{d}e)f] (z)");
        assert_eq!(any_bracket_object(&b, p(0, 6), 1), Some((p(0, 5), p(0, 7))));
        assert_eq!(any_bracket_object(&b, p(0, 8), 1), Some((p(0, 3), p(0, 9))));
        assert_eq!(any_bracket_object(&b, p(0, 6), 2), Some((p(0, 3), p(0, 9))));
        assert_eq!(
            any_bracket_object(&b, p(0, 6), 3),
            Some((p(0, 1), p(0, 11)))
        );
        assert_eq!(
            any_bracket_object(&b, p(0, 12), 1),
            Some((p(0, 13), p(0, 15)))
        );
        assert_eq!(
            any_bracket_object(&b, p(0, 0), 1),
            Some((p(0, 1), p(0, 11)))
        );
    }

    #[test]
    fn quote_bounds_pairs_left_to_right_and_skips_escapes() {
        //                     0123456789012345678901
        let b = Buffer::from_text(r#"x "a\"b" y "c" z"#);
        assert_eq!(quote_bounds(&b, p(0, 4), '"'), Some((p(0, 2), p(0, 7))));
        // On a quote char: pairing from the line start decides.
        assert_eq!(quote_bounds(&b, p(0, 7), '"'), Some((p(0, 2), p(0, 7))));
        // Between strings: lookahead to the next one.
        assert_eq!(quote_bounds(&b, p(0, 9), '"'), Some((p(0, 11), p(0, 13))));
        // After the last string: nothing.
        assert_eq!(quote_bounds(&b, p(0, 15), '"'), None);
        // Other quote kinds.
        let b = Buffer::from_text("say 'hi' and `cmd`");
        assert_eq!(quote_bounds(&b, p(0, 5), '\''), Some((p(0, 4), p(0, 7))));
        assert_eq!(quote_bounds(&b, p(0, 14), '`'), Some((p(0, 13), p(0, 17))));
    }

    #[test]
    fn quote_bounds_on_soft_wrapped_line() {
        let b = Buffer::from_reads("ab \"cdef\" gh", "ab \"cd\nef\" gh");
        assert!(b.line(0).soft_wrap);
        assert_eq!(quote_bounds(&b, p(1, 0), '"'), Some((p(0, 3), p(1, 2))));
    }

    #[test]
    fn any_quote_picks_innermost_then_nearest() {
        //                     012345678901234
        let b = Buffer::from_text("\"a 'b' c\" 'd'");
        assert_eq!(any_quote_bounds(&b, p(0, 4)), Some((p(0, 3), p(0, 5))));
        assert_eq!(any_quote_bounds(&b, p(0, 1)), Some((p(0, 0), p(0, 8))));
        assert_eq!(any_quote_bounds(&b, p(0, 9)), Some((p(0, 10), p(0, 12))));
    }

    #[test]
    fn surrounding_space_prefers_trailing_then_leading() {
        let b = Buffer::from_text("a \"b\"  c");
        assert_eq!(
            with_surrounding_space(&b, (p(0, 2), p(0, 4))),
            (p(0, 2), p(0, 6))
        );
        let b = Buffer::from_text("a \"b\"");
        assert_eq!(
            with_surrounding_space(&b, (p(0, 2), p(0, 4))),
            (p(0, 1), p(0, 4))
        );
        let b = Buffer::from_text("\"b\"");
        assert_eq!(
            with_surrounding_space(&b, (p(0, 0), p(0, 2))),
            (p(0, 0), p(0, 2))
        );
    }

    #[test]
    fn match_bracket_jumps_both_ways_and_scans_forward() {
        //                     0123456789
        let b = Buffer::from_text("x = (a[1] + f(2))\ny");
        assert_eq!(match_bracket(&b, p(0, 4)), Some(p(0, 16)));
        assert_eq!(match_bracket(&b, p(0, 16)), Some(p(0, 4)));
        assert_eq!(match_bracket(&b, p(0, 6)), Some(p(0, 8)));
        // Not on a bracket: the first one to the right on the row.
        assert_eq!(match_bracket(&b, p(0, 0)), Some(p(0, 16)));
        assert_eq!(match_bracket(&b, p(1, 0)), None);
        // Multi-line pairs.
        let b = Buffer::from_text("{\n  a\n}");
        assert_eq!(match_bracket(&b, p(0, 0)), Some(p(2, 0)));
        assert_eq!(match_bracket(&b, p(2, 0)), Some(p(0, 0)));
        // Unbalanced: nothing.
        assert_eq!(match_bracket(&Buffer::from_text("(("), p(0, 0)), None);
    }

    #[test]
    fn find_char_forward_backward_till_and_repeat() {
        //                     0123456789
        let b = Buffer::from_text("a,b,c,d");
        assert_eq!(
            find_char(&b, p(0, 0), ',', true, false, false),
            Some(p(0, 1))
        );
        assert_eq!(
            find_char(&b, p(0, 1), ',', true, false, false),
            Some(p(0, 3))
        );
        assert_eq!(
            find_char(&b, p(0, 6), ',', false, false, false),
            Some(p(0, 5))
        );
        // t stops before the hit.
        assert_eq!(
            find_char(&b, p(0, 0), ',', true, true, false),
            Some(p(0, 0))
        );
        assert_eq!(
            find_char(&b, p(0, 2), ',', true, true, false),
            Some(p(0, 2))
        );
        // Repeating a till that would not move skips to the next hit.
        assert_eq!(find_char(&b, p(0, 2), ',', true, true, true), Some(p(0, 4)));
        assert_eq!(
            find_char(&b, p(0, 4), ',', false, true, true),
            Some(p(0, 2))
        );
        // Missing target.
        assert_eq!(find_char(&b, p(0, 0), 'z', true, false, false), None);
        // Does not leave the logical line, but does follow soft wraps.
        let b = Buffer::from_text("abc\nxa");
        assert_eq!(find_char(&b, p(0, 0), 'x', true, false, false), None);
        let b = Buffer::from_reads("abcxa\nq", "abc\nxa\nq");
        assert_eq!(
            find_char(&b, p(0, 0), 'x', true, false, false),
            Some(p(1, 0))
        );
    }

    #[test]
    fn paragraph_rows_inner_and_around() {
        //                            0  1  2   3  4   5   6
        let b = Buffer::from_text("a\nb\n\nc\nd\n\n\ne");
        assert_eq!(paragraph_rows(&b, 0, false), (0, 1));
        assert_eq!(paragraph_rows(&b, 0, true), (0, 2));
        assert_eq!(paragraph_rows(&b, 4, true), (3, 6));
        // On a blank block: inner is the blanks, around adds the next paragraph.
        assert_eq!(paragraph_rows(&b, 5, false), (5, 6));
        assert_eq!(paragraph_rows(&b, 5, true), (5, 7));
        // Last paragraph without trailing blanks takes the blanks before it.
        assert_eq!(paragraph_rows(&b, 7, true), (5, 7));
    }

    #[test]
    fn line_object_trims_inner_and_joins_wraps() {
        let b = Buffer::from_text("  hello world  ");
        assert_eq!(line_object(&b, 0, false), Some((p(0, 2), p(0, 12))));
        assert_eq!(line_object(&b, 0, true), Some((p(0, 0), p(0, 14))));
        assert_eq!(line_object(&Buffer::from_text("   "), 0, false), None);
        let b = Buffer::from_reads("abcdefgh\nok", "abcd\nefgh\nok");
        assert_eq!(line_object(&b, 1, true), Some((p(0, 0), p(1, 3))));
    }

    #[test]
    fn entire_rows_skips_blank_edges_for_inner() {
        let b = Buffer::from_text("\n\na\nb\n\nc\n\n");
        assert_eq!(entire_rows(&b, true), (0, 5));
        assert_eq!(entire_rows(&b, false), (2, 5));
    }
}
