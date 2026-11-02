use crate::ansi;
use crate::app::{App, Mode, SelKind};
use crate::buffer::{Buffer, Pos};
use crate::config::{ColorPair, Theme};
use crate::extract::{ExtractApp, Granularity};
use crate::lineedit::LineEdit;
use crate::search::Direction;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::style::{
    Attribute, Color, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::{execute, queue};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

/// Highlight class of a rendered cell. Precedence when several apply:
/// Cursor > Sel > CurrentMatch > Match > None.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hl {
    None,
    Match,
    CurrentMatch,
    Sel,
    Cursor,
}

fn hl_at(app: &App, row: usize, col: usize, sel: Option<(usize, usize)>) -> Hl {
    if row == app.cursor.row && col == app.cursor.col {
        return Hl::Cursor;
    }
    if let Some((s, e)) = sel {
        if col >= s && col <= e {
            return Hl::Sel;
        }
    }
    // `.get` not `[i]`: `current` is kept in bounds by Search, but a
    // draw is the worst place to panic, so index defensively.
    if let Some(m) = app.search.current.and_then(|i| app.search.matches.get(i)) {
        if m.covers(Pos { row, col }) {
            return Hl::CurrentMatch;
        }
    }
    if app.search.match_at(Pos { row, col }).is_some() {
        return Hl::Match;
    }
    Hl::None
}

/// Merge key for rendered spans. Normal and selected cells retain their
/// source style; search matches and the cursor use the themed colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanAttrs {
    pub hl: Hl,
    pub style: ansi::Style,
}

fn push_char<K: Copy + PartialEq>(spans: &mut Vec<(K, String)>, key: K, c: char) {
    match spans.last_mut() {
        Some((k, s)) if *k == key => s.push(c),
        _ => spans.push((key, c.to_string())),
    }
}

/// Per-row spans in char space, merging selection, search matches, the
/// current match, the cursor cell, and the source style. The cursor
/// gets a virtual space cell when it sits past the end of the line.
pub fn row_spans(app: &App, row: usize) -> Vec<(SpanAttrs, String)> {
    let line = app.buf.line(row);
    let sel = app.sel_range(row);
    let mut spans = Vec::new();
    for (col, &c) in line.chars.iter().enumerate() {
        let hl = hl_at(app, row, col, sel);
        let style = if matches!(hl, Hl::None | Hl::Sel) {
            line.style_at(col)
        } else {
            ansi::Style::default()
        };
        push_char(&mut spans, SpanAttrs { hl, style }, c);
    }
    if row == app.cursor.row && app.cursor.col >= line.len() {
        spans.push((
            SpanAttrs {
                hl: Hl::Cursor,
                style: ansi::Style::default(),
            },
            " ".into(),
        ));
    }
    spans
}

/// Raw buffer row merged by source style only (extract backdrop).
pub fn buffer_row_spans(buf: &Buffer, row: usize) -> Vec<(ansi::Style, String)> {
    let line = buf.line(row);
    let mut spans = Vec::new();
    for (col, &c) in line.chars.iter().enumerate() {
        push_char(&mut spans, line.style_at(col), c);
    }
    spans
}

/// Clip spans to the viewport [hoff, hoff + view_cols) in display cells.
/// A wide char straddling the left edge renders as a space; a char that
/// would cross the right edge is dropped.
pub fn clip_spans<K: Copy + PartialEq>(
    spans: Vec<(K, String)>,
    hoff: usize,
    view_cols: usize,
) -> Vec<(K, String)> {
    let end = hoff + view_cols;
    let mut out = Vec::new();
    let mut x = 0usize;
    for (key, text) in spans {
        for c in text.chars() {
            let w = c.width().unwrap_or(0).max(1);
            if x + w <= hoff {
                x += w;
                continue;
            }
            if x >= end || x + w > end {
                return out;
            }
            if x < hoff {
                push_char(&mut out, key, ' ');
            } else {
                push_char(&mut out, key, c);
            }
            x += w;
        }
    }
    out
}

/// Badge naming the current mode, leading the persistent bottom row.
pub fn mode_badge(app: &App) -> &'static str {
    match app.mode {
        Mode::Prompt { .. } => "[search]",
        _ => "[copy]",
    }
}

/// Render a mode badge with styling shared across every mode: the themed
/// fg/bg pair plus bold in color mode, a reverse-video bold bar under
/// NO_COLOR. The label is truncated to `cols`; returns its display width.
fn print_badge(
    out: &mut impl Write,
    label: &str,
    pair: ColorPair,
    cols: usize,
) -> io::Result<usize> {
    let badge = truncate_to_width(label.to_string(), cols);
    let width: usize = badge.chars().map(|c| c.width().unwrap_or(0).max(1)).sum();
    if use_color() {
        apply_pair(out, pair)?;
        queue!(
            out,
            SetAttribute(Attribute::Bold),
            crossterm::style::Print(&badge),
            ResetColor,
            SetAttribute(Attribute::Reset)
        )?;
    } else {
        queue!(
            out,
            SetAttribute(Attribute::Reverse),
            SetAttribute(Attribute::Bold),
            crossterm::style::Print(&badge),
            SetAttribute(Attribute::Reset)
        )?;
    }
    Ok(width)
}

/// Extra text for the mode row after the badge: the pattern menu while
/// it is open, or a transient message. The prompt renders separately
/// via line_edit_spans; committed searches live in the top-right
/// indicator.
pub fn mode_row_message(app: &App) -> Option<String> {
    if matches!(app.mode, Mode::PatternMenu) {
        return Some(app.patterns.menu_line());
    }
    app.message.clone()
}

/// Contextual keybinding hint for the mode row, shown dim when the row
/// is otherwise idle (no prompt, pattern menu, or transient message).
/// None outside Normal mode - the prompt shows the query and the
/// pattern menu is its own hint.
pub fn mode_hint(app: &App) -> Option<String> {
    if !matches!(app.mode, Mode::Normal) {
        return None;
    }
    Some(if app.sel.is_some() {
        "y copy  Enter copy+exit  gx open  Esc clear".to_string()
    } else {
        "/ search  n/N next  v select  gx open  p patterns  y copy  q quit".to_string()
    })
}

/// Keybind hint for the extract popup, shown dim on the separator row.
pub fn extract_hint() -> &'static str {
    "Tab copy  Enter insert  ^t word/line  Esc cancel"
}

/// A full-width box-drawing rule. Used both as the divider between the
/// backdrop and the candidate list and as the separator above the input.
fn separator_rule(cols: usize) -> String {
    truncate_to_width("\u{2500}".repeat(cols), cols)
}

/// Always-on top-right indicator, tmux mode-style: selection state,
/// the active search summary, and the cursor position.
pub fn indicator(app: &App) -> String {
    let mut out = String::new();
    match app.sel {
        Some((SelKind::Char, _)) => out.push_str("VISUAL "),
        Some((SelKind::Line, _)) => out.push_str("VISUAL LINE "),
        None => {}
    }
    if !app.search.pattern.is_empty() {
        match app.search.current {
            Some(i) => out.push_str(&format!(
                "/{} [{}/{}] ",
                app.search.pattern,
                i + 1,
                app.search.matches.len()
            )),
            None => out.push_str(&format!(
                "/{} [{}] ",
                app.search.pattern,
                app.search.matches.len()
            )),
        }
    }
    out.push_str(&format!(
        "[{}/{}]",
        app.cursor.row + 1,
        app.buf.last_row() + 1
    ));
    out
}

fn term_color(c: ansi::Color) -> Option<Color> {
    match c {
        ansi::Color::Default => None,
        ansi::Color::Indexed(n) => Some(Color::AnsiValue(n)),
        ansi::Color::Rgb(r, g, b) => Some(Color::Rgb { r, g, b }),
    }
}

fn apply_source_style(out: &mut impl Write, s: ansi::Style) -> io::Result<()> {
    if let Some(c) = term_color(s.fg) {
        queue!(out, SetForegroundColor(c))?;
    }
    if let Some(c) = term_color(s.bg) {
        queue!(out, SetBackgroundColor(c))?;
    }
    for (on, attr) in [
        (s.bold, Attribute::Bold),
        (s.dim, Attribute::Dim),
        (s.italic, Attribute::Italic),
        (s.underline, Attribute::Underlined),
        (s.reverse, Attribute::Reverse),
    ] {
        if on {
            queue!(out, SetAttribute(attr))?;
        }
    }
    Ok(())
}

/// Emit a themed fg/bg color pair, skipping either side that is Default
/// (leave the terminal's own color in place).
fn apply_pair(out: &mut impl Write, pair: ColorPair) -> io::Result<()> {
    if let Some(c) = term_color(pair.fg) {
        queue!(out, SetForegroundColor(c))?;
    }
    if let Some(c) = term_color(pair.bg) {
        queue!(out, SetBackgroundColor(c))?;
    }
    Ok(())
}

fn apply_attrs(out: &mut impl Write, a: SpanAttrs, theme: &Theme) -> io::Result<()> {
    match a.hl {
        Hl::None => apply_source_style(out, a.style)?,
        Hl::Match => apply_pair(out, theme.match_)?,
        Hl::CurrentMatch => apply_pair(out, theme.match_current)?,
        Hl::Sel => {
            apply_source_style(out, a.style)?;
            if let Some(bg) = term_color(theme.selection_bg) {
                // Reverse video swaps displayed foreground/background.
                // Change only the displayed background in either case.
                if a.style.reverse {
                    queue!(out, SetForegroundColor(bg))?;
                } else {
                    queue!(out, SetBackgroundColor(bg))?;
                }
            }
        }
        Hl::Cursor => apply_pair(out, theme.cursor)?,
    }
    Ok(())
}

fn truncate_to_width(mut text: String, cols: usize) -> String {
    let mut w = 0usize;
    text.retain(|c| {
        w += c.width().unwrap_or(0).max(1);
        w < cols
    });
    text
}

/// Spans (reverse-video?, text) for a one-line prompt: the prefix and
/// the text around the cursor, with the cursor cell (the char under
/// it, or a trailing space) in reverse video. Text left of the cursor
/// is trimmed from its start so the cursor always stays on screen.
pub fn line_edit_spans(prefix: &str, le: &LineEdit, cols: usize) -> Vec<(bool, String)> {
    let cell = |c: char| c.width().unwrap_or(0).max(1);
    let pw: usize = prefix.chars().map(cell).sum();
    let cursor = le.under().unwrap_or(' ');
    if pw + cell(cursor) > cols {
        return vec![(false, truncate_to_width(prefix.to_string(), cols))];
    }
    let mut avail = cols - pw - cell(cursor);
    let mut before: Vec<char> = Vec::new();
    for c in le.before_text().chars().rev() {
        if cell(c) > avail {
            break;
        }
        avail -= cell(c);
        before.push(c);
    }
    let before: String = before.into_iter().rev().collect();
    let mut after = String::new();
    for c in le.after_text().chars() {
        if cell(c) > avail {
            break;
        }
        avail -= cell(c);
        after.push(c);
    }
    let mut spans = vec![
        (false, format!("{prefix}{before}")),
        (true, cursor.to_string()),
    ];
    if !after.is_empty() {
        spans.push((false, after));
    }
    spans
}

fn print_line_edit(
    out: &mut impl Write,
    prefix: &str,
    le: &LineEdit,
    cols: usize,
) -> io::Result<()> {
    for (reverse, text) in line_edit_spans(prefix, le, cols) {
        if reverse {
            queue!(
                out,
                SetAttribute(Attribute::Reverse),
                crossterm::style::Print(text),
                SetAttribute(Attribute::Reset)
            )?;
        } else {
            queue!(out, crossterm::style::Print(text))?;
        }
    }
    Ok(())
}

/// Colors are emitted unless NO_COLOR is set (https://no-color.org);
/// styling then falls back to attributes (reverse/dim) that follow the
/// terminal's own theme.
fn use_color() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}

/// Draw the whole screen: content rows then the bottom line.
pub fn draw(out: &mut impl Write, app: &App) -> io::Result<()> {
    // Frames can exceed the writer/PTY buffer. Publish only after all cleared
    // rows have received their replacement text.
    queue!(out, BeginSynchronizedUpdate)?;
    for y in 0..app.view_rows {
        queue!(out, MoveTo(0, y as u16), Clear(ClearType::CurrentLine))?;
        let row = app.top + y;
        if row <= app.buf.last_row() {
            for (attrs, text) in clip_spans(row_spans(app, row), app.hoff, app.view_cols) {
                apply_attrs(out, attrs, &app.theme)?;
                queue!(out, crossterm::style::Print(text), ResetColor)?;
                queue!(out, SetAttribute(Attribute::Reset))?;
            }
        }
    }
    // Top-right position indicator, over the first content row.
    let ind = truncate_to_width(indicator(app), app.view_cols.saturating_sub(1));
    let w = ind.chars().count() + 1; // leading pad masks a cut wide char
    if w <= app.view_cols {
        queue!(out, MoveTo((app.view_cols - w) as u16, 0))?;
        if use_color() {
            apply_pair(out, app.theme.indicator)?;
            queue!(out, crossterm::style::Print(format!(" {ind}")), ResetColor)?;
        } else {
            queue!(
                out,
                SetAttribute(Attribute::Reverse),
                crossterm::style::Print(format!(" {ind}")),
                SetAttribute(Attribute::Reset)
            )?;
        }
    }
    // Persistent bottom mode row: the badge, then the prompt, the
    // pattern menu, or a transient message (herdr-native mode style).
    queue!(
        out,
        MoveTo(0, (app.view_rows - 1) as u16),
        Clear(ClearType::CurrentLine)
    )?;
    // Search vs copy get distinct accent colors (terminal-themed); with
    // NO_COLOR the badge falls back to a reverse-video bar.
    let pair = if matches!(app.mode, Mode::Prompt { .. }) {
        app.theme.badge_search
    } else {
        app.theme.badge_copy
    };
    let badge_w = print_badge(out, mode_badge(app), pair, app.view_cols)?;
    queue!(out, crossterm::style::Print(" "))?;
    let rest = app.view_cols.saturating_sub(badge_w + 1);
    if let Mode::Prompt {
        direction, input, ..
    } = &app.mode
    {
        let prefix = match direction {
            Direction::Forward => "/",
            Direction::Backward => "?",
        };
        print_line_edit(out, prefix, input, rest)?;
    } else if let Some(text) = mode_row_message(app) {
        let text = truncate_to_width(text, rest);
        queue!(out, crossterm::style::Print(&text))?;
    } else if let Some(hint) = mode_hint(app) {
        // Idle row: teach the keys, dim so it stays in the background.
        let hint = truncate_to_width(hint, rest);
        queue!(
            out,
            SetAttribute(Attribute::Dim),
            crossterm::style::Print(&hint),
            SetAttribute(Attribute::Reset)
        )?;
    }
    queue!(out, EndSynchronizedUpdate)?;
    out.flush()
}

/// Extract-mode screen, fzf --height style. Top to bottom: the source
/// pane shows through as a styled, tail-aligned backdrop, closed by a
/// dim divider rule; then the candidate list (best match at the bottom,
/// next to the hint); then a dim keybind hint; then a dim separator
/// rule; then the input row with its `[word]`/`[line]` badge.
pub fn draw_extract(out: &mut impl Write, app: &ExtractApp, backdrop: &Buffer) -> io::Result<()> {
    queue!(out, BeginSynchronizedUpdate)?;
    if app.backdrop_rows > 0 {
        let content_rows = app.backdrop_rows - 1;
        let start = backdrop
            .lines
            .len()
            .saturating_sub(content_rows + app.backdrop_off);
        for y in 0..content_rows {
            queue!(out, MoveTo(0, y as u16), Clear(ClearType::CurrentLine))?;
            let row = start + y;
            if row < backdrop.lines.len() {
                for (style, text) in clip_spans(buffer_row_spans(backdrop, row), 0, app.view_cols) {
                    apply_source_style(out, style)?;
                    queue!(out, crossterm::style::Print(text), ResetColor)?;
                    queue!(out, SetAttribute(Attribute::Reset))?;
                }
            }
        }
        // Divider between the backdrop and the candidate list.
        queue!(
            out,
            MoveTo(0, (app.backdrop_rows - 1) as u16),
            Clear(ClearType::CurrentLine),
            SetAttribute(Attribute::Dim),
            crossterm::style::Print(separator_rule(app.view_cols)),
            SetAttribute(Attribute::Reset)
        )?;
    }
    let window = app.window();
    for y in 0..app.view_rows {
        queue!(
            out,
            MoveTo(0, (app.backdrop_rows + y) as u16),
            Clear(ClearType::CurrentLine)
        )?;
        let k = app.view_rows - 1 - y;
        if let Some(&idx) = window.get(k) {
            let text = truncate_to_width(app.candidates()[idx].clone(), app.view_cols);
            if k == app.window_selected() {
                if use_color() {
                    apply_pair(out, app.theme.extract_selected)?;
                    queue!(out, crossterm::style::Print(text), ResetColor)?;
                } else {
                    queue!(
                        out,
                        SetAttribute(Attribute::Reverse),
                        crossterm::style::Print(text),
                        SetAttribute(Attribute::Reset)
                    )?;
                }
            } else {
                queue!(out, crossterm::style::Print(text))?;
            }
        }
    }
    // Keybind hint, then a separator rule, sitting below the list and
    // above the input.
    let hint_row = app.backdrop_rows + app.view_rows;
    queue!(
        out,
        MoveTo(0, hint_row as u16),
        Clear(ClearType::CurrentLine),
        SetAttribute(Attribute::Dim),
        crossterm::style::Print(truncate_to_width(extract_hint().to_string(), app.view_cols)),
        SetAttribute(Attribute::Reset),
        MoveTo(0, (hint_row + 1) as u16),
        Clear(ClearType::CurrentLine),
        SetAttribute(Attribute::Dim),
        crossterm::style::Print(separator_rule(app.view_cols)),
        SetAttribute(Attribute::Reset)
    )?;
    // Input row.
    queue!(
        out,
        MoveTo(0, (hint_row + 2) as u16),
        Clear(ClearType::CurrentLine)
    )?;
    let (badge, badge_pair) = match app.granularity {
        Granularity::Word => ("[word]", app.theme.badge_word),
        Granularity::Line => ("[line]", app.theme.badge_line),
    };
    let badge_w = print_badge(out, badge, badge_pair, app.view_cols)?;
    let prefix = format!(" {}> ", app.filtered.len());
    print_line_edit(
        out,
        &prefix,
        &app.query,
        app.view_cols.saturating_sub(badge_w),
    )?;
    queue!(out, EndSynchronizedUpdate)?;
    out.flush()
}

/// RAII terminal state: raw mode, alternate screen, mouse capture, and
/// a panic hook that restores the terminal before printing the panic.
pub struct TerminalGuard;

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture, Hide)?;
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = TerminalGuard::restore();
            hook(info);
        }));
        Ok(TerminalGuard)
    }

    fn restore() -> io::Result<()> {
        execute!(
            io::stdout(),
            EndSynchronizedUpdate,
            Show,
            DisableMouseCapture,
            LeaveAlternateScreen
        )?;
        terminal::disable_raw_mode()
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = Self::restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::buffer::Buffer;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn app(text: &str) -> App {
        let mut a = App::new(Buffer::from_text(text), false, None);
        a.set_size(80, 11);
        a
    }

    fn searched(text: &str, pat: &str) -> App {
        let mut a = app(text);
        a.handle_key(key('/'));
        for c in pat.chars() {
            a.handle_key(key(c));
        }
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a
    }

    /// Project styled spans down to their highlight class.
    fn hls(spans: Vec<(SpanAttrs, String)>) -> Vec<(Hl, String)> {
        spans.into_iter().map(|(a, s)| (a.hl, s)).collect()
    }

    #[test]
    fn mode_hint_normal_lists_core_keys() {
        let a = app("hello");
        let h = mode_hint(&a).expect("hint in normal mode");
        assert!(h.contains("search"), "hint mentions search: {h}");
        assert!(h.contains("copy"), "hint mentions copy: {h}");
        assert!(h.contains("quit"), "hint mentions quit: {h}");
    }

    #[test]
    fn mode_hint_switches_when_selecting() {
        let mut a = app("hello world");
        let normal = mode_hint(&a).unwrap();
        a.handle_key(key('v'));
        let selecting = mode_hint(&a).expect("hint while selecting");
        assert_ne!(normal, selecting);
        assert!(selecting.contains("copy"));
    }

    #[test]
    fn mode_hint_hidden_in_prompt_and_menu() {
        let mut a = app("hello");
        a.handle_key(key('/'));
        assert_eq!(mode_hint(&a), None, "prompt shows the query, not a hint");
        a.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        a.handle_key(key('p'));
        assert_eq!(mode_hint(&a), None, "pattern menu is its own hint");
    }

    #[test]
    fn mode_hint_truncates_to_row_width() {
        // The draw path clips the hint with truncate_to_width when the
        // mode row is narrower than the hint (ui.rs draw). Rendering is
        // TTY-only, so verify that same composition here.
        let a = app("hello");
        let hint = mode_hint(&a).expect("hint in normal mode");
        assert!(hint.chars().count() > 20, "full hint is wide: {hint}");
        let clipped = truncate_to_width(hint.clone(), 20);
        assert!(
            clipped.chars().count() < 20,
            "clip fits the row: {clipped:?}"
        );
        assert!(
            clipped.chars().count() < hint.chars().count(),
            "clip is shorter"
        );
        assert!(hint.starts_with(&clipped), "clip is a prefix of the hint");
        assert!(!clipped.is_empty(), "some hint survives at width 20");
    }

    #[test]
    fn spans_mark_current_match_and_other_matches() {
        // Search opened from the bottom wraps to row 0 as current.
        let a = searched("foo bar foo", "foo");
        assert_eq!(
            hls(row_spans(&a, 0)),
            vec![
                (Hl::Cursor, "f".into()),
                (Hl::CurrentMatch, "oo".into()),
                (Hl::None, " bar ".into()),
                (Hl::Match, "foo".into()),
            ]
        );
    }

    #[test]
    fn selection_wins_over_match_but_not_cursor() {
        let mut a = searched("foo bar foo", "bar");
        a.handle_key(key('v'));
        for _ in 0..2 {
            a.handle_key(key('l'));
        }
        // Selection covers cols 4..=6 ("bar"), cursor at col 6.
        assert_eq!(
            hls(row_spans(&a, 0)),
            vec![
                (Hl::None, "foo ".into()),
                (Hl::Sel, "ba".into()),
                (Hl::Cursor, "r".into()),
                (Hl::None, " foo".into()),
            ]
        );
    }

    #[test]
    fn cursor_on_empty_line_gets_virtual_cell() {
        let mut a = app("foo\n\nbar");
        a.handle_key(key('g'));
        a.handle_key(key('g'));
        a.handle_key(key('j'));
        assert_eq!(hls(row_spans(&a, 1)), vec![(Hl::Cursor, " ".into())]);
    }

    fn styled_app(ansi_text: &str) -> App {
        let mut a = App::new(Buffer::from_ansi(ansi_text), false, None);
        a.set_size(80, 11);
        a
    }

    #[test]
    fn spans_split_on_source_style_change() {
        // Cursor starts at col 0 and masks 'r'; the rest of the red run
        // keeps its source style, then the default-styled tail follows.
        let a = styled_app("\x1b[31mred\x1b[0m plain");
        let spans = row_spans(&a, 0);
        assert_eq!(spans[0].0.hl, Hl::Cursor);
        assert_eq!(spans[1].1, "ed");
        assert_eq!(spans[1].0.style.fg, ansi::Color::Indexed(1));
        assert_eq!(spans[2].1, " plain");
        assert_eq!(spans[2].0.style, ansi::Style::default());
    }

    #[test]
    fn selection_keeps_source_colors_and_attributes() {
        let mut a =
            styled_app("\x1b[1;31mred\x1b[0;38;5;215m orange\x1b[38;2;100;150;200m blue\x1b[0m!");
        a.handle_key(key('0'));
        a.handle_key(key('v'));
        a.handle_key(key('$'));
        let selected: Vec<_> = row_spans(&a, 0)
            .into_iter()
            .filter(|(attrs, _)| attrs.hl == Hl::Sel)
            .collect();
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].0.style.fg, ansi::Color::Indexed(1));
        assert!(selected[0].0.style.bold);
        assert_eq!(selected[1].0.style.fg, ansi::Color::Indexed(215));
        assert_eq!(selected[2].0.style.fg, ansi::Color::Rgb(100, 150, 200));

        // The test harness may set NO_COLOR; this assertion exercises
        // the colored terminal path explicitly.
        crossterm::style::force_color_output(true);
        let mut rendered = Vec::new();
        apply_attrs(&mut rendered, selected[0].0, &a.theme).unwrap();
        rendered.extend_from_slice(b"red");
        let parsed = ansi::parse(std::str::from_utf8(&rendered).unwrap());
        assert_eq!(parsed[0].styles[0].fg, ansi::Color::Indexed(1));
        assert!(parsed[0].styles[0].bold);
        assert_eq!(parsed[0].styles[0].bg, ansi::Color::Rgb(60, 64, 71));
        assert!(!parsed[0].styles[0].reverse);
    }

    #[test]
    fn highlight_merges_across_source_colors() {
        // "ab" spans two source colors; a search match must stay one span.
        let mut a = styled_app("\x1b[31ma\x1b[32mb\x1b[0m tail");
        a.handle_key(key('/'));
        a.handle_key(key('a'));
        a.handle_key(key('b'));
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let spans = row_spans(&a, 0);
        // Cursor covers 'a'; 'b' continues the current match.
        assert_eq!(hls(spans.clone())[0], (Hl::Cursor, "a".into()));
        assert_eq!(hls(spans)[1], (Hl::CurrentMatch, "b".into()));
    }

    #[test]
    fn buffer_row_spans_merge_by_style() {
        let b = Buffer::from_ansi("\x1b[31mab\x1b[0mcd");
        let spans = buffer_row_spans(&b, 0);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].1, "ab");
        assert_eq!(spans[0].0.fg, ansi::Color::Indexed(1));
        assert_eq!(spans[1].1, "cd");
        assert_eq!(spans[1].0, ansi::Style::default());
    }

    #[test]
    fn clip_drops_cells_left_of_hoff_and_pads_straddling_wide_char() {
        let spans = vec![(Hl::None, "\u{4e2d}\u{6587}abc".into())];
        // hoff=1 cuts through the first wide char.
        assert_eq!(
            clip_spans(spans, 1, 80),
            vec![(Hl::None, " \u{6587}abc".into())]
        );
    }

    #[test]
    fn clip_stops_at_right_edge_without_splitting_wide_char() {
        let spans = vec![(Hl::None, "ab\u{4e2d}cd".into())];
        // Viewport of 3 cells: 'a', 'b' fit; the wide char would span
        // cells 2..4 and is dropped along with everything after it.
        assert_eq!(clip_spans(spans, 0, 3), vec![(Hl::None, "ab".into())]);
    }

    #[test]
    fn prompt_spans_show_cursor_then_row_yields_back() {
        let mut a = app("foo bar");
        a.handle_key(key('/'));
        a.handle_key(key('b'));
        let Mode::Prompt { input, .. } = &a.mode else {
            panic!("prompt should be open");
        };
        assert_eq!(
            line_edit_spans("/", input, 80),
            vec![(false, "/b".into()), (true, " ".into())]
        );
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(mode_row_message(&a), None);
        assert_eq!(indicator(&a), "/b [1/1] [1/1]");
    }

    #[test]
    fn mode_badge_names_search_while_prompting() {
        let mut a = app("foo");
        assert_eq!(mode_badge(&a), "[copy]");
        a.handle_key(key('/'));
        assert_eq!(mode_badge(&a), "[search]");
        a.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(mode_badge(&a), "[copy]");
    }

    fn line_edit(text: &str, left: usize) -> LineEdit {
        let mut le = LineEdit::default();
        for c in text.chars() {
            le.insert(c);
        }
        for _ in 0..left {
            le.left();
        }
        le
    }

    #[test]
    fn line_edit_spans_mark_the_char_under_the_cursor() {
        assert_eq!(
            line_edit_spans("/", &line_edit("abc", 2), 80),
            vec![
                (false, "/a".into()),
                (true, "b".into()),
                (false, "c".into()),
            ]
        );
    }

    #[test]
    fn line_edit_spans_trim_the_left_to_keep_the_cursor_visible() {
        assert_eq!(
            line_edit_spans("/", &line_edit("abcdef", 0), 5),
            vec![(false, "/def".into()), (true, " ".into())]
        );
    }

    #[test]
    fn line_edit_spans_clip_the_tail_after_the_cursor() {
        assert_eq!(
            line_edit_spans("/", &line_edit("abcdef", 6), 5),
            vec![
                (false, "/".into()),
                (true, "a".into()),
                (false, "bcd".into()),
            ]
        );
    }

    #[test]
    fn line_edit_spans_handle_wide_chars() {
        // Each CJK char is 2 cells; cols 4 fits the prefix, one char,
        // and the cursor cell.
        let wide = "\u{4e2d}\u{6587}";
        assert_eq!(
            line_edit_spans("/", &line_edit(wide, 0), 4),
            vec![(false, "/\u{6587}".into()), (true, " ".into())]
        );
    }

    #[test]
    fn mode_row_shows_messages_and_menu() {
        let mut a = app("foo");
        a.handle_key(key('n'));
        assert_eq!(mode_row_message(&a).as_deref(), Some("no previous search"));
        a.handle_key(key('p'));
        assert!(mode_row_message(&a).unwrap().starts_with("pattern:"));
    }

    #[test]
    fn indicator_reflects_selection_and_position() {
        let mut a = app("one\ntwo\nthree");
        assert_eq!(indicator(&a), "[3/3]");
        a.handle_key(key('V'));
        assert_eq!(indicator(&a), "VISUAL LINE [3/3]");
    }

    #[test]
    fn extract_hint_names_core_keys() {
        let h = extract_hint();
        for k in ["copy", "insert", "word/line", "Esc"] {
            assert!(h.contains(k), "extract hint mentions {k}: {h}");
        }
    }
}
