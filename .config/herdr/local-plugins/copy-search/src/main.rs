use crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};
use herdr_copy_search::app::{App, Effect};
use herdr_copy_search::buffer::Buffer;
use herdr_copy_search::config::Theme;
use herdr_copy_search::extract::{ExtractApp, ExtractEffect};
use herdr_copy_search::herdr::{self, Herdr};
use herdr_copy_search::nested;
use herdr_copy_search::patterns::Patterns;
use herdr_copy_search::{osc52, ui};
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

const USAGE: &str = "usage: herdr-copy-search [--mode copy|search|extract] [--pane ID] [--lines N] [--height PCT] [--input FILE]";

/// The herdr server caps pane reads at ~1000 lines (spike-verified),
/// so asking for more only future-proofs against a raised cap.
const DEFAULT_LINES: usize = 1000;

#[derive(Debug, PartialEq, Eq)]
enum RunMode {
    Copy,
    Search,
    Extract,
}

#[derive(Debug, PartialEq, Eq)]
struct Cli {
    mode: RunMode,
    input: Option<PathBuf>,
    pane: Option<String>,
    lines: usize,
    /// Extract popup height percent; overrides the config file.
    height: Option<u8>,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut cli = Cli {
        mode: RunMode::Copy,
        input: None,
        pane: None,
        lines: DEFAULT_LINES,
        height: None,
    };
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--mode" => {
                cli.mode = match value("--mode")?.as_str() {
                    "copy" => RunMode::Copy,
                    "search" => RunMode::Search,
                    "extract" => RunMode::Extract,
                    other => return Err(format!("unknown mode: {other}")),
                }
            }
            "--input" => cli.input = Some(PathBuf::from(value("--input")?)),
            "--pane" => cli.pane = Some(value("--pane")?),
            "--lines" => {
                cli.lines = value("--lines")?
                    .parse()
                    .map_err(|_| "--lines needs a number".to_string())?
            }
            "--height" => {
                let pct: u8 = value("--height")?
                    .parse()
                    .map_err(|_| "--height needs a number".to_string())?;
                if !(1..=100).contains(&pct) {
                    return Err("--height must be 1-100".to_string());
                }
                cli.height = Some(pct);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(cli)
}

/// Input order: explicit file, then a herdr source pane, then piped
/// stdin (dev use; crossterm reads events from /dev/tty instead).
/// The herdr path uses passive ANSI snapshots only: logical text for
/// search/yank/tokens, styled wrapped rows for display (composed by
/// Buffer::from_reads), and the visible screen so the opening view
/// can keep the pane content in place (None outside herdr).
fn acquire_buffer(cli: &Cli, source_pane: Option<&str>) -> io::Result<(Buffer, Option<usize>)> {
    if let Some(path) = &cli.input {
        return Ok((Buffer::from_ansi(&std::fs::read_to_string(path)?), None));
    }
    if let Some(pane) = source_pane {
        let herdr = Herdr::from_env();
        let unwrapped = herdr.pane_read(pane, cli.lines)?;
        let wrapped_ansi = herdr.pane_read_ansi(pane, cli.lines).unwrap_or_default();
        if wrapped_ansi.trim().is_empty() {
            return Ok((Buffer::from_text(&unwrapped), None));
        }
        let buf = Buffer::from_reads(&unwrapped, &wrapped_ansi);
        let visible = herdr.pane_read_visible(pane, cli.lines).unwrap_or_default();
        let rows = screen_rows(&buf, &visible);
        return Ok((buf, rows));
    }
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        return Ok((Buffer::from_ansi(&io::read_to_string(stdin)?), None));
    }
    Err(io::Error::other(
        "no input: pass --input FILE, run inside herdr, or pipe text on stdin",
    ))
}

/// Rows of the source pane's screen, verified against the buffer: the
/// visible read must equal the buffer tail row by row (modulo trailing
/// whitespace). None when they do not line up (the pane changed
/// between reads, or rows come from an unwrapped fallback), so the
/// opening view falls back to filling the screen with the tail.
fn screen_rows(buf: &Buffer, visible: &str) -> Option<usize> {
    let rows: Vec<&str> = visible.lines().collect();
    let n = rows.len();
    if n == 0 || n > buf.lines.len() {
        return None;
    }
    let start = buf.lines.len() - n;
    for (i, want) in rows.iter().enumerate() {
        if buf.lines[start + i].text().trim_end() != want.trim_end() {
            return None;
        }
    }
    Some(n)
}

/// Write `text` to the clipboard via OSC 52, surfacing the >100KB
/// truncation warning and failures in the mode row. Returns whether the
/// write reached the terminal so the caller can hold the pane open for
/// the forwarding grace. Shared by the keyboard and mouse copy paths.
fn emit_copy(app: &mut App, text: &str) -> bool {
    match osc52::emit(text) {
        Ok(false) => true,
        Ok(true) => {
            app.message = Some(format!(
                "copied {} chars (large; terminal may truncate)",
                text.chars().count()
            ));
            true
        }
        Err(e) => {
            app.message = Some(format!("copy failed: {e}"));
            false
        }
    }
}

/// Return an open request after restoring the terminal. Fingers may show
/// an interactive file/editor chooser using this same overlay's TTY.
fn run(app: &mut App) -> io::Result<(bool, Option<String>)> {
    let _guard = ui::TerminalGuard::enter()?;
    let mut out = io::BufWriter::new(io::stdout());
    let (w, h) = crossterm::terminal::size()?;
    app.set_size(w as usize, h as usize);
    let mut copied = false;
    ui::draw(&mut out, app)?;
    while !app.quit {
        match event::read()? {
            Event::Key(k) if k.kind != KeyEventKind::Release => match app.handle_key(k) {
                Some(Effect::Copy(text)) => copied |= emit_copy(app, &text),
                Some(Effect::Open(text)) => return Ok((copied, Some(text))),
                None => {}
            },
            Event::Mouse(m)
                if matches!(
                    m.kind,
                    MouseEventKind::Down(MouseButton::Left)
                        | MouseEventKind::Up(MouseButton::Left)
                        | MouseEventKind::Drag(MouseButton::Left)
                        | MouseEventKind::ScrollUp
                        | MouseEventKind::ScrollDown
                ) =>
            {
                if let Some(Effect::Copy(text)) = app.handle_mouse(m, Instant::now()) {
                    copied |= emit_copy(app, &text);
                }
            }
            Event::Resize(w, h) => app.set_size(w as usize, h as usize),
            // Passive mouse movement changes no state; repainting it floods
            // the PTY and exposes intermediate frames to the host terminal.
            _ => continue,
        }
        if !app.quit {
            ui::draw(&mut out, app)?;
        }
    }
    Ok((copied, None))
}

/// Extract loop; the resulting effect runs after the terminal is
/// restored so errors stay visible and send-text hits a live pane.
fn run_extract(app: &mut ExtractApp, backdrop: &Buffer) -> io::Result<Option<ExtractEffect>> {
    let _guard = ui::TerminalGuard::enter()?;
    let mut out = io::BufWriter::new(io::stdout());
    let (w, h) = crossterm::terminal::size()?;
    app.set_size(w as usize, h as usize);
    ui::draw_extract(&mut out, app, backdrop)?;
    while !app.quit {
        match event::read()? {
            Event::Key(k) if k.kind != KeyEventKind::Release => {
                if let Some(effect) = app.handle_key(k) {
                    return Ok(Some(effect));
                }
            }
            Event::Mouse(m)
                if matches!(
                    m.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                ) =>
            {
                app.handle_mouse(m);
            }
            Event::Resize(w, h) => app.set_size(w as usize, h as usize),
            _ => continue,
        }
        if !app.quit {
            ui::draw_extract(&mut out, app, backdrop)?;
        }
    }
    Ok(None)
}

fn extract_mode(
    buf: &Buffer,
    height: u8,
    source_pane: Option<&str>,
    theme: Theme,
) -> io::Result<()> {
    let mut app = ExtractApp::new(&buf.text(), height, buf.lines.len());
    app.set_theme(theme);
    match run_extract(&mut app, buf)? {
        Some(ExtractEffect::Copy(text)) => {
            if osc52::emit(&text)? {
                eprintln!("copied (large; terminal may truncate)");
            }
            std::thread::sleep(osc52::FORWARD_GRACE);
        }
        Some(ExtractEffect::Insert(text)) => {
            let pane =
                source_pane.ok_or_else(|| io::Error::other("no source pane to insert into"))?;
            Herdr::from_env().send_text(pane, &text)?;
        }
        None => {}
    }
    Ok(())
}

fn copy_mode(
    buf: Buffer,
    start_search: bool,
    screen_rows: Option<usize>,
    patterns: Patterns,
    theme: Theme,
    source_pane: Option<&str>,
) -> io::Result<()> {
    let mut app = App::new(buf, start_search, screen_rows);
    app.set_patterns(patterns);
    app.set_theme(theme);
    loop {
        let (copied, open) = run(&mut app)?;
        if let Some(text) = open {
            match herdr_copy_search::fingers::open(&text, source_pane) {
                Ok(()) => {
                    // Fingers can fall back to OSC 52 for unresolved text.
                    std::thread::sleep(osc52::FORWARD_GRACE);
                    break;
                }
                Err(error) => app.message = Some(format!("open failed: {error}")),
            }
        } else {
            if copied {
                std::thread::sleep(osc52::FORWARD_GRACE);
            }
            break;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let cli = match parse_args(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("herdr-copy-search: {e}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let source_pane = herdr::resolve_source_pane(cli.pane.as_deref(), env);
    // Refuse to stack a copy-search overlay on top of itself: when our
    // source pane is a live overlay, this is a nested launch - exit
    // before touching the terminal so nothing is drawn. Outside herdr
    // (no HERDR_PANE_ID) there is nothing to register or guard against.
    let registry = nested::registry_dir(env);
    if nested::is_nested(source_pane.as_deref(), |p| {
        nested::is_active_overlay(&registry, p)
    }) {
        return ExitCode::SUCCESS;
    }
    let _registry_guard =
        env("HERDR_PANE_ID").and_then(|id| nested::register(&registry, &id, std::process::id()));
    let (buf, screen_rows) = match acquire_buffer(&cli, source_pane.as_deref()) {
        Ok(acquired) => acquired,
        Err(e) => {
            eprintln!("herdr-copy-search: {e}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let cfg_text = herdr_copy_search::config::read_config_text(|k| std::env::var(k).ok());
    let height = cli
        .height
        .unwrap_or_else(|| herdr_copy_search::config::Config::from_toml(&cfg_text).extract_height);
    let theme = Theme::from_toml(&cfg_text);
    let result = match cli.mode {
        RunMode::Extract => extract_mode(&buf, height, source_pane.as_deref(), theme),
        RunMode::Copy => copy_mode(
            buf,
            false,
            screen_rows,
            Patterns::from_config(&cfg_text),
            theme,
            source_pane.as_deref(),
        ),
        RunMode::Search => copy_mode(
            buf,
            true,
            screen_rows,
            Patterns::from_config(&cfg_text),
            theme,
            source_pane.as_deref(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("herdr-copy-search: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_to_copy_mode() {
        let cli = parse(&[]).unwrap();
        assert_eq!(cli.mode, RunMode::Copy);
        assert_eq!(cli.input, None);
        assert_eq!(cli.pane, None);
        assert_eq!(cli.lines, DEFAULT_LINES);
    }

    #[test]
    fn parses_mode_and_input() {
        let cli = parse(&["--mode", "search", "--input", "/tmp/x"]).unwrap();
        assert_eq!(cli.mode, RunMode::Search);
        assert_eq!(cli.input, Some(PathBuf::from("/tmp/x")));
    }

    #[test]
    fn parses_pane_and_lines() {
        let cli = parse(&["--pane", "w8:p1", "--lines", "500"]).unwrap();
        assert_eq!(cli.pane.as_deref(), Some("w8:p1"));
        assert_eq!(cli.lines, 500);
    }

    #[test]
    fn rejects_unknown_arguments_and_modes() {
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["--mode", "bogus"]).is_err());
        assert!(parse(&["--mode"]).is_err());
        assert!(parse(&["--lines", "abc"]).is_err());
    }

    #[test]
    fn screen_rows_verifies_the_buffer_tail() {
        let buf = Buffer::from_text("a\nb\nc\nd");
        assert_eq!(screen_rows(&buf, "c\nd"), Some(2));
        assert_eq!(screen_rows(&buf, "a\nb\nc\nd"), Some(4));
        assert_eq!(
            screen_rows(&buf, "x\nd"),
            None,
            "pane changed between reads"
        );
        assert_eq!(screen_rows(&buf, ""), None);
    }

    #[test]
    fn screen_rows_ignores_trailing_whitespace() {
        let buf = Buffer::from_text("a  \nb");
        assert_eq!(screen_rows(&buf, "a\nb   "), Some(2));
    }

    #[test]
    fn screen_rows_rejects_more_rows_than_the_buffer() {
        let buf = Buffer::from_text("a");
        assert_eq!(screen_rows(&buf, "a\nb"), None);
    }
}
