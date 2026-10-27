//! Draws the captured pane back, hints on top, and turns key presses into
//! kernel [`Key`]s.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect as UiRect;
use ratatui::style::{Color as UiColor, Modifier as UiModifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

use crate::domain::geometry::{OverlayGeometry, Rect};
use crate::domain::screen::Screen;
use crate::domain::session::{Key, Modifier, Outcome, Session, Target};
use crate::domain::settings::{HintPosition, Theme};
use crate::domain::style::{Color, TextStyle};
use crate::usecases::ports::{PickView, Picker, PortError};

/// Everything the renderer needs, borrowed for one frame.
pub struct View<'a> {
    pub screen: &'a Screen,
    pub session: &'a Session,
    pub theme: &'a Theme,
    pub geometry: Option<&'a OverlayGeometry>,
    /// A one-line notice (configuration problem, nothing to pick…).
    pub notice: Option<&'a str>,
}

const MIN_STATUS_WIDTH: u16 = 24;

/// The real overlay: ratatui on the pane's terminal.
#[derive(Debug, Default, Clone, Copy)]
pub struct TerminalPicker;

impl Picker for TerminalPicker {
    fn pick(&mut self, view: &PickView<'_>, session: &mut Session) -> Result<Outcome, PortError> {
        run(view.screen, session, view.theme, view.geometry, view.notice).map_err(PortError::new)
    }
}

/// Runs the overlay until the user picks something or gives up.
pub fn run(
    screen: &Screen,
    session: &mut Session,
    theme: &Theme,
    geometry: Option<&OverlayGeometry>,
    notice: Option<&str>,
) -> io::Result<Outcome> {
    let mut terminal = ratatui::init();
    let outcome = loop {
        terminal.draw(|frame| {
            let view = View {
                screen,
                session,
                theme,
                geometry,
                notice,
            };
            render(frame.area(), frame.buffer_mut(), &view);
        })?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if session.targets().is_empty() {
                    break Outcome::Cancelled;
                }
                if let Some(key) = key_from_event(key) {
                    match session.press(key) {
                        Outcome::Continue => {}
                        other => break other,
                    }
                }
            }
            _ => {}
        }
    };
    if std::env::var("HERDR_PLUGIN_ENTRYPOINT_ID").as_deref() == Ok("overlay") {
        // This pane has its own disposable PTY. Keep the last frame visible
        // while the selected action runs; leaving the alternate screen here
        // exposes a blank pane until the action finishes and Herdr closes it.
        // Release raw mode so commands with an interactive chooser still work.
        crossterm::terminal::disable_raw_mode()?;
    } else {
        ratatui::restore();
    }
    Ok(outcome)
}

/// Shows `message` full screen and waits for a key, so an error is read
/// before the overlay closes.
pub fn show_error_and_wait(message: &str) -> io::Result<()> {
    let mut terminal = ratatui::init();
    terminal.draw(|frame| {
        let area = frame.area();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" herdr-fingers ");
        let text = Paragraph::new(vec![
            Line::from(message.to_string()),
            Line::from(""),
            Line::from("press any key to close"),
        ])
        .block(block)
        .style(Style::default().fg(UiColor::Red));
        frame.render_widget(text, area);
    })?;
    loop {
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            break;
        }
    }
    ratatui::restore();
    Ok(())
}

/// Maps a terminal key press to a kernel key.
pub fn key_from_event(key: KeyEvent) -> Option<Key> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Esc => Some(Key::Escape),
        KeyCode::Tab | KeyCode::BackTab => Some(Key::Tab),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Char('c') if ctrl => Some(Key::Escape),
        KeyCode::Char('q') if !ctrl && !alt => Some(Key::Escape),
        KeyCode::Char('?') => Some(Key::Help),
        KeyCode::Char(ch) if ch.is_alphanumeric() => {
            let modifier = if ctrl {
                Modifier::Ctrl
            } else if alt {
                Modifier::Alt
            } else if ch.is_uppercase() || key.modifiers.contains(KeyModifiers::SHIFT) {
                Modifier::Shift
            } else {
                Modifier::Main
            };
            Some(Key::Hint(ch.to_ascii_lowercase(), modifier))
        }
        _ => None,
    }
}

/// Paints one frame: the pane, the highlights, the hints, the status line
/// and, when asked, the help box.
pub fn render(area: UiRect, buf: &mut Buffer, view: &View<'_>) {
    let frame = from_ui(area);
    let content = view
        .geometry
        .map_or(frame, |geometry| geometry.content_rect(frame));
    paint_screen(buf, content, view);
    for target in view.session.targets() {
        let selected = view.session.is_selected(&target.hint);
        if !(selected || view.session.is_reachable(target)) {
            continue;
        }
        paint_target(buf, content, view, target, selected);
    }
    if let Some(status) = status_rect(frame, content, view) {
        paint_status(buf, status, view);
    }
    if view.session.shows_help() {
        paint_help(buf, area, view.theme);
    } else if view.session.targets().is_empty() {
        paint_notice(buf, area, "Nothing to pick on this screen — press any key");
    }
}

fn paint_screen(buf: &mut Buffer, content: Rect, view: &View<'_>) {
    for (index, row) in view.screen.rows().iter().enumerate() {
        let Ok(offset) = u16::try_from(index) else {
            break;
        };
        if offset >= content.height {
            break;
        }
        let y = content.y + offset;
        let mut x = content.x;
        for cell in &row.cells {
            let width = u16::from(cell.width);
            if x.saturating_add(width) > content.right() {
                break;
            }
            let style = match view.theme.backdrop {
                Some(backdrop) => backdrop.over(cell.style),
                None => cell.style,
            };
            buf.set_string(x, y, &cell.text, to_ui_style(style));
            x += width;
        }
    }
}

fn paint_target(buf: &mut Buffer, content: Rect, view: &View<'_>, target: &Target, selected: bool) {
    let (hint_style, highlight_style) = if selected {
        (view.theme.selected_hint, view.theme.selected_highlight)
    } else {
        (view.theme.hint, view.theme.highlight)
    };
    for segment in &target.segments {
        let Ok(row) = u16::try_from(segment.row) else {
            continue;
        };
        if row >= content.height {
            continue;
        }
        let y = content.y + row;
        for col in segment.col..segment.end() {
            let x = content.x.saturating_add(col);
            if x >= content.right() {
                break;
            }
            let cell = &mut buf[(x, y)];
            let patched = cell.style().patch(to_ui_style(highlight_style));
            cell.set_style(patched);
        }
    }
    let hint_width = u16::try_from(target.hint.chars().count()).unwrap_or(u16::MAX);
    let anchor = match view.theme.hint_position {
        HintPosition::Left => target.segments.first().map(|s| (s.row, s.col)),
        HintPosition::Right => target
            .segments
            .last()
            .map(|s| (s.row, s.end().saturating_sub(hint_width).max(s.col))),
    };
    let Some((row, col)) = anchor else { return };
    let Ok(row) = u16::try_from(row) else { return };
    if row >= content.height {
        return;
    }
    let y = content.y + row;
    let x = content.x.saturating_add(col);
    if x.saturating_add(hint_width) > content.right() {
        return;
    }
    let typed = view.session.input().chars().count();
    for (index, ch) in target.hint.chars().enumerate() {
        let style = if index < typed {
            view.theme.selected_hint
        } else {
            hint_style
        };
        buf.set_string(x + index as u16, y, ch.to_string(), to_ui_style(style));
    }
}

/// Where the status line goes: a blank strip of the tab outside the pane, or
/// the free end of the pane's last row when the pane fills the tab.
fn status_rect(frame: Rect, content: Rect, view: &View<'_>) -> Option<Rect> {
    if let Some(strip) = view.geometry.and_then(|g| g.status_rect(frame)) {
        return Some(strip);
    }
    if content.is_empty() {
        return None;
    }
    let last_row = usize::from(content.height) - 1;
    let occupied = view
        .screen
        .rows()
        .get(last_row)
        .map_or(0, |row| row.display_width());
    let occupied = u16::try_from(occupied)
        .unwrap_or(u16::MAX)
        .saturating_add(1);
    let free = content.width.saturating_sub(occupied);
    if free < MIN_STATUS_WIDTH {
        return None;
    }
    Some(Rect::new(
        content.x + occupied,
        content.bottom() - 1,
        free,
        1,
    ))
}

fn paint_status(buf: &mut Buffer, area: Rect, view: &View<'_>) {
    let session = view.session;
    let mut parts: Vec<String> = Vec::new();
    if let Some(notice) = view.notice {
        parts.push(format!("⚠ {notice}"));
    }
    if session.is_multi() {
        parts.push(format!("multi ({} picked)", session.selected_count()));
    }
    if !session.input().is_empty() {
        parts.push(format!("typed: {}", session.input()));
    }
    parts.push("tab multi · esc quit · ? help".to_string());
    let mut text = parts.join(" · ");
    while text.width() > usize::from(area.width).saturating_sub(2) && text.contains(" · ") {
        let cut = text.rfind(" · ").expect("checked above");
        text.truncate(cut);
    }
    let text: String = text
        .chars()
        .take(usize::from(area.width).saturating_sub(1))
        .collect();
    let style = Style::default()
        .fg(UiColor::Indexed(8))
        .add_modifier(UiModifier::ITALIC);
    let x = area
        .right()
        .saturating_sub(u16::try_from(text.width()).unwrap_or(0) + 1);
    buf.set_string(x.max(area.x), area.y, text, style);
}

fn paint_help(buf: &mut Buffer, area: UiRect, theme: &Theme) {
    let lines = [
        "type a hint      pick the match under it",
        "SHIFT + hint     shift action (default: paste into the pane)",
        "CTRL + hint      ctrl action (default: open URL or file)",
        "ALT + hint       alt action (default: none)",
        "TAB              toggle multi-select; TAB or ENTER again to confirm",
        "BACKSPACE        erase the last typed key",
        "ESC / q / ^C     close without picking",
        "",
        "config: herdr plugin config-dir nathan-poncet.herdr-fingers",
    ];
    let width = lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16 + 4;
    let height = lines.len() as u16 + 2;
    let popup = UiRect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width.min(area.width),
        height.min(area.height),
    );
    Clear.render(popup, buf);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" herdr-fingers ")
        .style(to_ui_style(theme.hint.over(TextStyle::PLAIN)).remove_modifier(UiModifier::BOLD));
    let paragraph = Paragraph::new(
        lines
            .iter()
            .map(|line| Line::from(Span::raw(format!(" {line}"))))
            .collect::<Vec<_>>(),
    )
    .block(block);
    paragraph.render(popup, buf);
}

fn paint_notice(buf: &mut Buffer, area: UiRect, message: &str) {
    let width = (message.width() as u16 + 4).min(area.width);
    let popup = UiRect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(3) / 2,
        width,
        3.min(area.height),
    );
    Clear.render(popup, buf);
    Paragraph::new(Line::from(format!(" {message} ")))
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(UiColor::Yellow))
        .render(popup, buf);
}

fn from_ui(area: UiRect) -> Rect {
    Rect::new(area.x, area.y, area.width, area.height)
}

fn to_ui_color(color: Color) -> UiColor {
    match color {
        Color::Indexed(index) => UiColor::Indexed(index),
        Color::Rgb(r, g, b) => UiColor::Rgb(r, g, b),
    }
}

/// The kernel style as ratatui sees it. Unset colors reset to the terminal
/// default so a highlight never inherits a stray background.
pub fn to_ui_style(style: TextStyle) -> Style {
    let mut ui = Style::default();
    ui = ui.fg(style.fg.map_or(UiColor::Reset, to_ui_color));
    ui = ui.bg(style.bg.map_or(UiColor::Reset, to_ui_color));
    let flags = [
        (style.bold, UiModifier::BOLD),
        (style.dim, UiModifier::DIM),
        (style.italic, UiModifier::ITALIC),
        (style.underline, UiModifier::UNDERLINED),
        (style.reverse, UiModifier::REVERSED),
        (style.strikethrough, UiModifier::CROSSED_OUT),
    ];
    for (on, modifier) in flags {
        if on {
            ui = ui.add_modifier(modifier);
        }
    }
    ui
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::alphabet::Alphabet;
    use crate::domain::geometry::{Layout, PanePlacement};
    use crate::domain::matcher::find_candidates;
    use crate::domain::patterns::PatternSet;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn session_for(screen: &Screen) -> Session {
        Session::new(
            find_candidates(screen, &PatternSet::builtin()),
            &Alphabet::custom("asdf").unwrap(),
        )
    }

    fn draw(view: &View<'_>, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render(frame.area(), frame.buffer_mut(), view))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn the_pane_is_redrawn_with_hints_over_the_matches() {
        let screen = Screen::from_ansi("edit /etc/hosts now\nsee https://x.io", 40);
        let session = session_for(&screen);
        let theme = Theme::default();
        let view = View {
            screen: &screen,
            session: &session,
            theme: &theme,
            geometry: None,
            notice: None,
        };
        let buf = draw(&view, 40, 3);
        assert_eq!(row_text(&buf, 0), "edit setc/hosts now");
        assert_eq!(row_text(&buf, 1), "see attps://x.io");
        assert_eq!(buf[(5, 0)].style().bg, Some(UiColor::Indexed(3)));
        assert_eq!(buf[(6, 0)].style().fg, Some(UiColor::Indexed(3)));
        assert_eq!(buf[(0, 0)].style().fg, Some(UiColor::Reset));
    }

    #[test]
    fn the_pane_keeps_its_own_colors_and_the_geometry_offsets_it() {
        let screen = Screen::from_ansi("\x1b[32mgreen\x1b[0m /tmp/x", 20);
        let session = session_for(&screen);
        let theme = Theme::default();
        let layout = Layout {
            area: Rect::new(0, 0, 40, 10),
            zoomed: false,
            panes: vec![PanePlacement {
                pane_id: "p".into(),
                rect: Rect::new(20, 5, 20, 5),
            }],
        };
        let geometry = OverlayGeometry::locate(&layout, "p").unwrap();
        let view = View {
            screen: &screen,
            session: &session,
            theme: &theme,
            geometry: Some(&geometry),
            notice: None,
        };
        let buf = draw(&view, 40, 10);
        assert_eq!(row_text(&buf, 5), format!("{}green atmp/x", " ".repeat(20)));
        assert_eq!(buf[(20, 5)].style().fg, Some(UiColor::Indexed(2)));
        assert!(
            row_text(&buf, 9).contains("tab multi"),
            "{}",
            row_text(&buf, 9)
        );
    }

    #[test]
    fn typing_hides_unreachable_hints_and_marks_the_typed_prefix() {
        let screen = Screen::from_ansi("/1111 /2222 /3333 /4444 /5555", 60);
        let mut session = session_for(&screen);
        session.press(Key::Hint('f', Modifier::Main));
        let theme = Theme::default();
        let view = View {
            screen: &screen,
            session: &session,
            theme: &theme,
            geometry: None,
            notice: None,
        };
        let buf = draw(&view, 60, 2);
        let line = row_text(&buf, 0);
        assert!(line.starts_with("fs111 fa222 /3333 /4444 /5555"), "{line}");
        assert_eq!(buf[(0, 0)].style().bg, Some(UiColor::Indexed(12)));
        assert_eq!(buf[(1, 0)].style().bg, Some(UiColor::Indexed(3)));
    }

    #[test]
    fn right_positioned_hints_end_where_the_match_ends() {
        let screen = Screen::from_ansi("x /etc/hosts", 40);
        let session = session_for(&screen);
        let theme = Theme {
            hint_position: HintPosition::Right,
            ..Theme::default()
        };
        let view = View {
            screen: &screen,
            session: &session,
            theme: &theme,
            geometry: None,
            notice: None,
        };
        let buf = draw(&view, 40, 2);
        assert_eq!(row_text(&buf, 0), "x /etc/hosta");
    }

    #[test]
    fn an_empty_screen_shows_a_notice_and_help_draws_a_box() {
        let screen = Screen::from_ansi("nothing here", 40);
        let mut session = session_for(&screen);
        let theme = Theme::default();
        let view = View {
            screen: &screen,
            session: &session,
            theme: &theme,
            geometry: None,
            notice: None,
        };
        let buf = draw(&view, 60, 7);
        let all: String = (0..7)
            .map(|y| row_text(&buf, y))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all.contains("Nothing to pick"), "{all}");

        let screen = Screen::from_ansi("/tmp/x", 40);
        session = session_for(&screen);
        session.press(Key::Help);
        let view = View {
            screen: &screen,
            session: &session,
            theme: &theme,
            geometry: None,
            notice: None,
        };
        let buf = draw(&view, 80, 14);
        let all: String = (0..14)
            .map(|y| row_text(&buf, y))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all.contains("toggle multi-select"), "{all}");
    }

    #[test]
    fn key_events_become_kernel_keys_with_their_modifier() {
        let press = |code, modifiers| KeyEvent::new(code, modifiers);
        assert_eq!(
            key_from_event(press(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(Key::Hint('a', Modifier::Main))
        );
        assert_eq!(
            key_from_event(press(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            Some(Key::Hint('a', Modifier::Shift))
        );
        assert_eq!(
            key_from_event(press(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            Some(Key::Hint('a', Modifier::Ctrl))
        );
        assert_eq!(
            key_from_event(press(KeyCode::Char('a'), KeyModifiers::ALT)),
            Some(Key::Hint('a', Modifier::Alt))
        );
        assert_eq!(
            key_from_event(press(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Key::Escape)
        );
        assert_eq!(
            key_from_event(press(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(Key::Escape)
        );
        assert_eq!(
            key_from_event(press(KeyCode::Char('?'), KeyModifiers::NONE)),
            Some(Key::Help)
        );
        assert_eq!(
            key_from_event(press(KeyCode::Tab, KeyModifiers::NONE)),
            Some(Key::Tab)
        );
        assert_eq!(
            key_from_event(press(KeyCode::Enter, KeyModifiers::NONE)),
            Some(Key::Enter)
        );
        assert_eq!(
            key_from_event(press(KeyCode::Backspace, KeyModifiers::NONE)),
            Some(Key::Backspace)
        );
        assert_eq!(
            key_from_event(press(KeyCode::Esc, KeyModifiers::NONE)),
            Some(Key::Escape)
        );
        assert_eq!(
            key_from_event(press(KeyCode::F(1), KeyModifiers::NONE)),
            None
        );
    }
}
