//! Everything the user can tune, already validated: the kernel never sees a
//! raw config file.

use super::alphabet::Alphabet;
use super::patterns::PatternSet;
use super::session::Modifier;
use super::style::{Color, TextStyle};

/// What happens to the picked text. `Shell` runs a command with the text on
/// its stdin and `MODIFIER`/`HINT` in its environment, like tmux-fingers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Copy,
    Paste,
    Open,
    Shell(String),
    Nothing,
}

impl Action {
    /// `:copy:`, `:paste:`, `:open:`, an empty string for nothing, or a
    /// shell command line.
    pub fn parse(value: &str) -> Action {
        match value.trim() {
            "" => Action::Nothing,
            ":copy:" => Action::Copy,
            ":paste:" => Action::Paste,
            ":open:" => Action::Open,
            command => Action::Shell(command.to_string()),
        }
    }
}

/// One action per modifier held while typing the hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actions {
    pub main: Action,
    pub ctrl: Action,
    pub shift: Action,
    pub alt: Action,
}

impl Actions {
    pub fn for_modifier(&self, modifier: Modifier) -> &Action {
        match modifier {
            Modifier::Main => &self.main,
            Modifier::Ctrl => &self.ctrl,
            Modifier::Shift => &self.shift,
            Modifier::Alt => &self.alt,
        }
    }
}

impl Default for Actions {
    fn default() -> Self {
        Actions {
            main: Action::Copy,
            ctrl: Action::Open,
            shift: Action::Paste,
            alt: Action::Nothing,
        }
    }
}

/// How `:copy:` reaches the clipboard. OSC 52 travels through Herdr to the
/// terminal you are looking at, even over SSH; `System` runs a local command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClipboardMode {
    #[default]
    Osc52,
    System,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HintPosition {
    #[default]
    Left,
    Right,
}

/// Colors of the overlay. `backdrop` restyles everything that is not a
/// match (for instance dimming it); `None` keeps the pane's own colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub hint: TextStyle,
    pub highlight: TextStyle,
    pub selected_hint: TextStyle,
    pub selected_highlight: TextStyle,
    pub backdrop: Option<TextStyle>,
    pub hint_position: HintPosition,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            hint: TextStyle::PLAIN
                .fg(Color::Indexed(0))
                .bg(Color::Indexed(3))
                .bold(),
            highlight: TextStyle::PLAIN.fg(Color::Indexed(3)),
            selected_hint: TextStyle::PLAIN
                .fg(Color::Indexed(0))
                .bg(Color::Indexed(12))
                .bold(),
            selected_highlight: TextStyle::PLAIN.fg(Color::Indexed(12)),
            backdrop: None,
            hint_position: HintPosition::Left,
        }
    }
}

/// The validated configuration of one run.
#[derive(Debug, Clone)]
pub struct Settings {
    pub alphabet: Alphabet,
    pub patterns: PatternSet,
    pub theme: Theme,
    pub actions: Actions,
    pub clipboard: ClipboardMode,
    pub clipboard_command: Vec<String>,
    pub notify_on_copy: bool,
    pub multi_separator: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            alphabet: Alphabet::default(),
            patterns: PatternSet::builtin(),
            theme: Theme::default(),
            actions: Actions::default(),
            clipboard: ClipboardMode::default(),
            clipboard_command: Vec::new(),
            notify_on_copy: false,
            multi_separator: " ".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_keywords_and_shell_commands_are_told_apart() {
        assert_eq!(Action::parse(":copy:"), Action::Copy);
        assert_eq!(Action::parse(" :open: "), Action::Open);
        assert_eq!(Action::parse(""), Action::Nothing);
        assert_eq!(
            Action::parse("xargs nvim"),
            Action::Shell("xargs nvim".into())
        );
    }

    #[test]
    fn default_actions_mirror_tmux_fingers() {
        let actions = Actions::default();
        assert_eq!(actions.for_modifier(Modifier::Main), &Action::Copy);
        assert_eq!(actions.for_modifier(Modifier::Ctrl), &Action::Open);
        assert_eq!(actions.for_modifier(Modifier::Shift), &Action::Paste);
        assert_eq!(actions.for_modifier(Modifier::Alt), &Action::Nothing);
    }
}
