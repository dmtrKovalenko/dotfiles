//! The composition root: builds the adapters and hands them to a use case,
//! one function per CLI subcommand.

use std::io::Read;

use thiserror::Error;

use crate::adapters::clipboard::SystemClipboard;
use crate::adapters::config::{self, ConfigError};
use crate::adapters::herdr::{HerdrClient, HerdrError, PluginContext};
use crate::adapters::log;
use crate::adapters::system::SystemLauncher;
use crate::adapters::tui::{self, TerminalPicker};
use crate::domain::geometry::{GeometryError, OverlayGeometry};
use crate::domain::matcher::find_candidates;
use crate::domain::patterns::{self, PatternError, PatternSet};
use crate::domain::screen::Screen;
use crate::domain::session::Session;
use crate::domain::settings::Settings;
use crate::usecases::pick::{self, Deps, PickError, PickRequest, Picked};
use crate::usecases::start::{self, GEOMETRY_ENV, PATTERNS_ENV, StartError};

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Herdr(#[from] HerdrError),
    #[error(transparent)]
    Start(#[from] StartError),
    #[error(transparent)]
    Pick(#[from] PickError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error(transparent)]
    Pattern(#[from] PatternError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("terminal error: {0}")]
    Terminal(#[from] std::io::Error),
    #[error("no focused pane: Herdr did not pass HERDR_PANE_ID")]
    NoFocusedPane,
    #[error("unknown option `{0}`")]
    UnknownOption(String),
}

/// `start`: the plugin action.
pub fn start(args: &[String]) -> Result<(), AppError> {
    let context = PluginContext::from_env();
    let client = HerdrClient::from_env()?;
    let pane_id = context.focused_pane_id.ok_or(AppError::NoFocusedPane)?;
    let patterns = option_value(args, "--patterns")?
        .map(|names| names.split(',').map(str::trim).collect::<Vec<_>>());
    start::start(&client, &context.plugin_id, &pane_id, patterns.as_deref())?;
    Ok(())
}

/// `ui`: runs inside the overlay pane. Errors are shown on screen before the
/// pane closes, and logged to the plugin's state directory.
pub fn ui() -> Result<(), AppError> {
    let context = PluginContext::from_env();
    match run_overlay(&context) {
        Ok(()) => Ok(()),
        Err(error) => {
            log::append(context.state_dir.as_deref(), &format!("error: {error}"));
            tui::show_error_and_wait(&error.to_string())?;
            Err(error)
        }
    }
}

fn run_overlay(context: &PluginContext) -> Result<(), AppError> {
    let client = HerdrClient::from_env()?;
    let geometry = match std::env::var(GEOMETRY_ENV) {
        Ok(text) => Some(OverlayGeometry::decode(&text)?),
        Err(_) => None,
    };
    let pane_id = geometry
        .as_ref()
        .map(|g| g.pane_id.clone())
        .or_else(|| context.focused_pane_id.clone())
        .ok_or(AppError::NoFocusedPane)?;

    let (mut settings, notice) = match config::load(context.config_dir.as_deref()) {
        Ok(settings) => (settings, None),
        Err(error) => {
            log::append(context.state_dir.as_deref(), &format!("config: {error}"));
            (
                Settings::default(),
                Some(format!("config.toml ignored: {error}")),
            )
        }
    };
    if let Ok(names) = std::env::var(PATTERNS_ENV) {
        let names: Vec<&str> = names.split(',').map(str::trim).collect();
        settings.patterns = PatternSet::compile(&patterns::builtin_specs(&names)?)?;
    }

    let mut clipboard =
        SystemClipboard::new(settings.clipboard, settings.clipboard_command.clone());
    let mut picker = TerminalPicker;
    let request = PickRequest {
        pane_id: &pane_id,
        geometry: geometry.as_ref(),
        settings: &settings,
        notice: notice.as_deref(),
    };
    let mut deps = Deps {
        host: &client,
        clipboard: &mut clipboard,
        launcher: &SystemLauncher,
        picker: &mut picker,
    };
    if let Picked::Done(message) = pick::pick(&request, &mut deps)? {
        log::append(context.state_dir.as_deref(), &message);
    }
    Ok(())
}

/// `scan`: reads a screen dump on stdin and prints what would get a hint.
/// Handy for trying custom patterns: `herdr pane read <id> | herdr-fingers scan`.
pub fn scan(args: &[String]) -> Result<(), AppError> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text)?;
    let width = option_value(args, "--width")?
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let context = PluginContext::from_env();
    let settings = config::load(context.config_dir.as_deref())?;
    let screen = Screen::from_ansi(&text, width);
    let session = Session::new(
        find_candidates(&screen, &settings.patterns),
        &settings.alphabet,
    );
    for target in session.targets() {
        let first = target.segments[0];
        println!(
            "{}\t{}:{}\t{}",
            target.hint,
            first.row + 1,
            first.col + 1,
            target.text
        );
    }
    Ok(())
}

fn option_value<'a>(args: &'a [String], name: &str) -> Result<Option<&'a str>, AppError> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return Ok(Some(value));
        }
        if arg == name {
            return Ok(iter.next().map(String::as_str));
        }
        if arg.starts_with("--") {
            return Err(AppError::UnknownOption(arg.clone()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options_accept_both_spellings_and_reject_unknown_ones() {
        assert_eq!(
            option_value(&args(&["--patterns", "url,path"]), "--patterns").unwrap(),
            Some("url,path")
        );
        assert_eq!(
            option_value(&args(&["--patterns=url"]), "--patterns").unwrap(),
            Some("url")
        );
        assert_eq!(option_value(&args(&[]), "--patterns").unwrap(), None);
        assert!(matches!(
            option_value(&args(&["--bogus"]), "--patterns"),
            Err(AppError::UnknownOption(name)) if name == "--bogus"
        ));
    }
}
