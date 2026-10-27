//! Reads the plugin's `config.toml` into validated [`Settings`].

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::domain::alphabet::{Alphabet, AlphabetError};
use crate::domain::patterns::{self, PatternError, PatternSet, PatternSpec};
use crate::domain::settings::{Action, Actions, ClipboardMode, HintPosition, Settings, Theme};
use crate::domain::style::{Color, ColorParseError, TextStyle};

/// The commented default configuration, written on first run.
pub const DEFAULT_CONFIG: &str = include_str!("../../examples/config.toml");

pub const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("config.toml is not valid: {0}")]
    Parse(#[from] toml::de::Error),
    #[error(transparent)]
    Alphabet(#[from] AlphabetError),
    #[error(transparent)]
    Pattern(#[from] PatternError),
    #[error("in [style]: {0}")]
    Color(#[from] ColorParseError),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    keyboard_layout: Option<String>,
    alphabet: Option<String>,
    hint_position: Option<String>,
    main_action: Option<String>,
    ctrl_action: Option<String>,
    shift_action: Option<String>,
    alt_action: Option<String>,
    enabled_builtin_patterns: Option<Vec<String>>,
    #[serde(default)]
    patterns: Vec<PatternEntry>,
    clipboard: Option<String>,
    #[serde(default)]
    clipboard_command: Vec<String>,
    show_copied_notification: Option<bool>,
    multi_separator: Option<String>,
    #[serde(default)]
    style: StyleConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatternEntry {
    name: String,
    regex: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleConfig {
    hint: Option<StyleEntry>,
    highlight: Option<StyleEntry>,
    selected_hint: Option<StyleEntry>,
    selected_highlight: Option<StyleEntry>,
    backdrop: Option<StyleEntry>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleEntry {
    fg: Option<String>,
    bg: Option<String>,
    #[serde(default)]
    bold: bool,
    #[serde(default)]
    dim: bool,
    #[serde(default)]
    italic: bool,
    #[serde(default)]
    underline: bool,
    #[serde(default)]
    reverse: bool,
    #[serde(default)]
    strikethrough: bool,
}

impl StyleEntry {
    fn to_style(&self) -> Result<TextStyle, ColorParseError> {
        Ok(TextStyle {
            fg: self.fg.as_deref().map(Color::parse).transpose()?,
            bg: self.bg.as_deref().map(Color::parse).transpose()?,
            bold: self.bold,
            dim: self.dim,
            italic: self.italic,
            underline: self.underline,
            reverse: self.reverse,
            strikethrough: self.strikethrough,
        })
    }
}

/// Loads `config.toml` from `dir`. A missing file means defaults, and the
/// commented default file is written there so the user can find the knobs.
pub fn load(dir: Option<&Path>) -> Result<Settings, ConfigError> {
    let Some(dir) = dir else {
        return Ok(Settings::default());
    };
    let path = dir.join(CONFIG_FILE_NAME);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            write_default(&path);
            Ok(Settings::default())
        }
        Err(source) => Err(ConfigError::Read { path, source }),
    }
}

fn write_default(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, DEFAULT_CONFIG);
}

/// Parses and validates a configuration file's text.
pub fn parse(text: &str) -> Result<Settings, ConfigError> {
    let file: FileConfig = toml::from_str(text)?;
    let defaults = Settings::default();

    let alphabet = match (&file.alphabet, &file.keyboard_layout) {
        (Some(keys), _) => Alphabet::custom(keys)?,
        (None, Some(layout)) => Alphabet::layout(layout)?,
        (None, None) => defaults.alphabet,
    };

    let mut specs = match &file.enabled_builtin_patterns {
        Some(names) => {
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            patterns::builtin_specs(&names)?
        }
        None => patterns::builtin_specs(&patterns::builtin_names())?,
    };
    let custom: Vec<PatternSpec> = file
        .patterns
        .iter()
        .map(|entry| PatternSpec::new(entry.name.clone(), entry.regex.clone()))
        .collect();
    specs.splice(0..0, custom);
    let pattern_set = PatternSet::compile(&specs)?;

    let hint_position = match file.hint_position.as_deref().map(str::trim) {
        None | Some("left") => HintPosition::Left,
        Some("right") => HintPosition::Right,
        Some(other) => {
            return Err(ConfigError::Invalid(format!(
                "hint_position must be \"left\" or \"right\", got \"{other}\""
            )));
        }
    };

    let clipboard = match file.clipboard.as_deref().map(str::trim) {
        None | Some("osc52") => ClipboardMode::Osc52,
        Some("system") => ClipboardMode::System,
        Some("both") => ClipboardMode::Both,
        Some(other) => {
            return Err(ConfigError::Invalid(format!(
                "clipboard must be \"osc52\", \"system\" or \"both\", got \"{other}\""
            )));
        }
    };

    let default_theme = Theme::default();
    let style =
        |entry: &Option<StyleEntry>, fallback: TextStyle| -> Result<TextStyle, ConfigError> {
            Ok(entry
                .as_ref()
                .map(StyleEntry::to_style)
                .transpose()?
                .unwrap_or(fallback))
        };
    let theme = Theme {
        hint: style(&file.style.hint, default_theme.hint)?,
        highlight: style(&file.style.highlight, default_theme.highlight)?,
        selected_hint: style(&file.style.selected_hint, default_theme.selected_hint)?,
        selected_highlight: style(
            &file.style.selected_highlight,
            default_theme.selected_highlight,
        )?,
        backdrop: file
            .style
            .backdrop
            .as_ref()
            .map(StyleEntry::to_style)
            .transpose()?,
        hint_position,
    };

    let action = |value: &Option<String>, fallback: Action| {
        value.as_deref().map(Action::parse).unwrap_or(fallback)
    };
    let default_actions = Actions::default();
    let actions = Actions {
        main: action(&file.main_action, default_actions.main),
        ctrl: action(&file.ctrl_action, default_actions.ctrl),
        shift: action(&file.shift_action, default_actions.shift),
        alt: action(&file.alt_action, default_actions.alt),
    };

    Ok(Settings {
        alphabet,
        patterns: pattern_set,
        theme,
        actions,
        clipboard,
        clipboard_command: file.clipboard_command,
        notify_on_copy: file
            .show_copied_notification
            .unwrap_or(defaults.notify_on_copy),
        multi_separator: file.multi_separator.unwrap_or(defaults.multi_separator),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::patterns::builtin_names;

    #[test]
    fn the_shipped_default_file_parses_to_the_default_settings() {
        let parsed = parse(DEFAULT_CONFIG).unwrap();
        let defaults = Settings::default();
        assert_eq!(parsed.alphabet, defaults.alphabet);
        assert_eq!(parsed.patterns.names(), defaults.patterns.names());
        assert_eq!(parsed.theme, defaults.theme);
        assert_eq!(parsed.actions, defaults.actions);
        assert_eq!(parsed.clipboard, defaults.clipboard);
        assert_eq!(parsed.clipboard_command, defaults.clipboard_command);
        assert_eq!(parsed.notify_on_copy, defaults.notify_on_copy);
        assert_eq!(parsed.multi_separator, defaults.multi_separator);
    }

    #[test]
    fn an_empty_file_is_the_defaults() {
        let parsed = parse("").unwrap();
        assert_eq!(parsed.patterns.names(), builtin_names());
        assert_eq!(parsed.actions, Actions::default());
    }

    #[test]
    fn custom_patterns_come_before_the_enabled_builtins() {
        let parsed = parse(
            r#"
enabled_builtin_patterns = ["url", "ip"]
[[patterns]]
name = "ticket"
regex = "PROJ-[0-9]+"
"#,
        )
        .unwrap();
        assert_eq!(parsed.patterns.names(), vec!["ticket", "ip", "url"]);
    }

    #[test]
    fn styles_actions_and_modes_are_read() {
        let parsed = parse(
            r##"
alphabet = "jkl"
hint_position = "right"
clipboard = "both"
clipboard_command = ["wl-copy"]
main_action = ":paste:"
alt_action = "xargs open"
show_copied_notification = true
multi_separator = "\n"
[style]
hint = { fg = "#ff0000", bg = "0", bold = true, underline = true }
backdrop = { dim = true }
"##,
        )
        .unwrap();
        assert_eq!(parsed.alphabet.keys(), &['j', 'k', 'l']);
        assert_eq!(parsed.theme.hint_position, HintPosition::Right);
        assert_eq!(parsed.clipboard, ClipboardMode::Both);
        assert_eq!(parsed.clipboard_command, vec!["wl-copy"]);
        assert_eq!(parsed.actions.main, Action::Paste);
        assert_eq!(parsed.actions.alt, Action::Shell("xargs open".into()));
        assert!(parsed.notify_on_copy);
        assert_eq!(parsed.multi_separator, "\n");
        assert_eq!(parsed.theme.hint.fg, Some(Color::Rgb(255, 0, 0)));
        assert!(parsed.theme.hint.underline);
        assert_eq!(parsed.theme.backdrop, Some(TextStyle::PLAIN.dim()));
    }

    #[test]
    fn mistakes_are_reported_precisely() {
        assert!(matches!(
            parse("hint_position = \"middle\""),
            Err(ConfigError::Invalid(_))
        ));
        assert!(matches!(
            parse("clipboard = \"pbcopy\""),
            Err(ConfigError::Invalid(_))
        ));
        assert!(matches!(
            parse("keyboard_layout = \"bepo\""),
            Err(ConfigError::Alphabet(_))
        ));
        assert!(matches!(
            parse("enabled_builtin_patterns = [\"nope\"]"),
            Err(ConfigError::Pattern(_))
        ));
        assert!(matches!(
            parse("[style]\nhint = { fg = \"chartreuse\" }"),
            Err(ConfigError::Color(_))
        ));
        assert!(matches!(parse("typo = 1"), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn a_missing_file_is_written_with_the_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "herdr-fingers-config-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let settings = load(Some(&dir)).unwrap();
        assert_eq!(settings.actions, Actions::default());
        assert_eq!(
            std::fs::read_to_string(dir.join(CONFIG_FILE_NAME)).unwrap(),
            DEFAULT_CONFIG
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
