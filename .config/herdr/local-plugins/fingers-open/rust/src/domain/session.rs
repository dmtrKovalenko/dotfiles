//! The hint-picking state machine: which hints exist, what has been typed,
//! and what the user ends up choosing.

use super::alphabet::Alphabet;
use super::hints;
use super::matcher::Candidate;
use super::screen::Segment;

/// A candidate with its hint label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub text: String,
    pub hint: String,
    pub segments: Vec<Segment>,
}

/// The key held while typing the hint; each has its own action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Main,
    Shift,
    Ctrl,
    Alt,
}

impl Modifier {
    /// The value tmux-fingers exposes as `MODIFIER` to shell actions.
    pub fn as_str(&self) -> &'static str {
        match self {
            Modifier::Main => "main",
            Modifier::Shift => "shift",
            Modifier::Ctrl => "ctrl",
            Modifier::Alt => "alt",
        }
    }
}

/// A key press, already stripped of terminal details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Hint(char, Modifier),
    Tab,
    Enter,
    Backspace,
    Escape,
    Help,
}

/// What the user picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub texts: Vec<String>,
    pub hint: String,
    pub modifier: Modifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Cancelled,
    Picked(Selection),
}

/// One overlay session, from the first drawn hint to the pick.
#[derive(Debug, Clone)]
pub struct Session {
    targets: Vec<Target>,
    input: String,
    multi: bool,
    selected: Vec<String>,
    last_modifier: Modifier,
    help: bool,
}

impl Session {
    /// Labels the candidates. Identical texts share a hint; the shortest
    /// hints go to the candidates nearest the bottom of the screen, where
    /// the freshest output is. A candidate shorter than its hint gets none.
    pub fn new(candidates: Vec<Candidate>, alphabet: &Alphabet) -> Self {
        let mut unique_texts: Vec<&str> = Vec::new();
        for candidate in &candidates {
            if !unique_texts.contains(&candidate.text.as_str()) {
                unique_texts.push(&candidate.text);
            }
        }
        let mut pool: std::collections::VecDeque<String> =
            hints::generate(alphabet.keys(), unique_texts.len()).into();
        let mut hint_by_text: Vec<(String, String)> = Vec::new();
        let mut targets: Vec<Target> = Vec::new();
        for candidate in candidates.iter().rev() {
            let hint = match hint_by_text
                .iter()
                .find(|(text, _)| *text == candidate.text)
            {
                Some((_, hint)) => hint.clone(),
                None => {
                    let Some(hint) = pool.pop_front() else { break };
                    if hint.chars().count() > candidate.text.chars().count() {
                        pool.push_front(hint);
                        continue;
                    }
                    hint_by_text.push((candidate.text.clone(), hint.clone()));
                    hint
                }
            };
            targets.push(Target {
                text: candidate.text.clone(),
                hint,
                segments: candidate.segments.clone(),
            });
        }
        targets.reverse();
        Session {
            targets,
            input: String::new(),
            multi: false,
            selected: Vec::new(),
            last_modifier: Modifier::Main,
            help: false,
        }
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn is_multi(&self) -> bool {
        self.multi
    }

    pub fn shows_help(&self) -> bool {
        self.help
    }

    pub fn is_selected(&self, hint: &str) -> bool {
        self.selected.iter().any(|s| s == hint)
    }

    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    /// Whether a target's hint is still reachable with what has been typed.
    pub fn is_reachable(&self, target: &Target) -> bool {
        target.hint.starts_with(&self.input)
    }

    pub fn press(&mut self, key: Key) -> Outcome {
        if self.help {
            self.help = false;
            return Outcome::Continue;
        }
        match key {
            Key::Escape => Outcome::Cancelled,
            Key::Help => {
                self.help = true;
                Outcome::Continue
            }
            Key::Backspace => {
                self.input.pop();
                Outcome::Continue
            }
            Key::Tab => {
                self.multi = !self.multi;
                self.input.clear();
                if !self.multi && !self.selected.is_empty() {
                    return self.pick_selected();
                }
                Outcome::Continue
            }
            Key::Enter => {
                if self.multi && !self.selected.is_empty() {
                    self.pick_selected()
                } else {
                    Outcome::Continue
                }
            }
            Key::Hint(key, modifier) => self.type_hint_key(key, modifier),
        }
    }

    fn type_hint_key(&mut self, key: char, modifier: Modifier) -> Outcome {
        let mut attempt = self.input.clone();
        attempt.push(key);
        if !self.targets.iter().any(|t| t.hint.starts_with(&attempt)) {
            self.input.clear();
            return Outcome::Continue;
        }
        self.input = attempt;
        let Some(target) = self.targets.iter().find(|t| t.hint == self.input) else {
            return Outcome::Continue;
        };
        let hint = target.hint.clone();
        let text = target.text.clone();
        self.input.clear();
        if self.multi {
            self.last_modifier = modifier;
            if let Some(index) = self.selected.iter().position(|s| *s == hint) {
                self.selected.remove(index);
            } else {
                self.selected.push(hint);
            }
            return Outcome::Continue;
        }
        Outcome::Picked(Selection {
            texts: vec![text],
            hint,
            modifier,
        })
    }

    fn pick_selected(&mut self) -> Outcome {
        let texts = self
            .selected
            .iter()
            .filter_map(|hint| self.targets.iter().find(|t| t.hint == *hint))
            .map(|t| t.text.clone())
            .collect();
        Outcome::Picked(Selection {
            texts,
            hint: self.selected.join(","),
            modifier: self.last_modifier,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(text: &str, row: usize, col: u16) -> Candidate {
        Candidate {
            text: text.into(),
            pattern: "test".into(),
            segments: vec![Segment {
                row,
                col,
                width: text.len() as u16,
            }],
        }
    }

    fn alphabet() -> Alphabet {
        Alphabet::custom("asdf").unwrap()
    }

    fn session(texts: &[&str]) -> Session {
        let candidates = texts
            .iter()
            .enumerate()
            .map(|(row, text)| candidate(text, row, 0))
            .collect();
        Session::new(candidates, &alphabet())
    }

    fn hints(session: &Session) -> Vec<(&str, &str)> {
        session
            .targets()
            .iter()
            .map(|t| (t.text.as_str(), t.hint.as_str()))
            .collect()
    }

    #[test]
    fn the_bottom_most_candidate_gets_the_best_hint() {
        let session = session(&["/top", "/middle", "/bottom"]);
        assert_eq!(
            hints(&session),
            vec![("/top", "d"), ("/middle", "s"), ("/bottom", "a")]
        );
    }

    #[test]
    fn identical_texts_share_one_hint() {
        let session = session(&["/same", "/other", "/same"]);
        assert_eq!(
            hints(&session),
            vec![("/same", "a"), ("/other", "s"), ("/same", "a")]
        );
    }

    #[test]
    fn a_candidate_shorter_than_its_hint_is_left_unlabelled() {
        let texts = ["1", "2", "3", "4", "5", "6"];
        let session = session(&texts);
        let labelled: Vec<&str> = session.targets().iter().map(|t| t.text.as_str()).collect();
        assert_eq!(labelled, vec!["4", "5", "6"]);
        assert!(
            session
                .targets()
                .iter()
                .all(|t| t.hint.len() <= t.text.len())
        );
    }

    #[test]
    fn typing_a_full_hint_picks_that_target_with_its_modifier() {
        let mut session = session(&["/top", "/bottom"]);
        assert_eq!(
            session.press(Key::Hint('s', Modifier::Shift)),
            Outcome::Picked(Selection {
                texts: vec!["/top".into()],
                hint: "s".into(),
                modifier: Modifier::Shift,
            })
        );
    }

    #[test]
    fn a_partial_hint_narrows_the_reachable_targets_and_backspace_widens_them() {
        let mut session = session(&["/1111", "/2222", "/3333", "/4444", "/5555"]);
        let hints: Vec<String> = session.targets().iter().map(|t| t.hint.clone()).collect();
        assert_eq!(hints, vec!["fs", "fa", "d", "s", "a"]);
        assert_eq!(
            session.press(Key::Hint('f', Modifier::Main)),
            Outcome::Continue
        );
        assert_eq!(session.input(), "f");
        let reachable: Vec<&str> = session
            .targets()
            .iter()
            .filter(|t| session.is_reachable(t))
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(reachable, vec!["/1111", "/2222"]);
        session.press(Key::Backspace);
        assert_eq!(session.input(), "");
        assert_eq!(
            session.press(Key::Hint('f', Modifier::Main)),
            Outcome::Continue
        );
        assert_eq!(
            session.press(Key::Hint('a', Modifier::Main)),
            Outcome::Picked(Selection {
                texts: vec!["/2222".into()],
                hint: "fa".into(),
                modifier: Modifier::Main
            })
        );
    }

    #[test]
    fn a_key_leading_nowhere_resets_the_input() {
        let mut session = session(&["/top", "/bottom"]);
        session.press(Key::Hint('z', Modifier::Main));
        assert_eq!(session.input(), "");
        assert!(matches!(
            session.press(Key::Hint('a', Modifier::Main)),
            Outcome::Picked(_)
        ));
    }

    #[test]
    fn multi_mode_collects_hints_and_tab_confirms_them_in_order() {
        let mut session = session(&["/top", "/middle", "/bottom"]);
        assert_eq!(session.press(Key::Tab), Outcome::Continue);
        assert!(session.is_multi());
        session.press(Key::Hint('a', Modifier::Main));
        session.press(Key::Hint('d', Modifier::Main));
        assert!(session.is_selected("a"));
        assert_eq!(session.selected_count(), 2);
        assert_eq!(
            session.press(Key::Tab),
            Outcome::Picked(Selection {
                texts: vec!["/bottom".into(), "/top".into()],
                hint: "a,d".into(),
                modifier: Modifier::Main,
            })
        );
    }

    #[test]
    fn multi_mode_toggles_a_hint_off_when_typed_twice_and_enter_confirms() {
        let mut session = session(&["/top", "/bottom"]);
        session.press(Key::Tab);
        session.press(Key::Hint('a', Modifier::Main));
        session.press(Key::Hint('a', Modifier::Main));
        assert_eq!(session.selected_count(), 0);
        assert_eq!(session.press(Key::Enter), Outcome::Continue);
        session.press(Key::Hint('s', Modifier::Shift));
        assert_eq!(
            session.press(Key::Enter),
            Outcome::Picked(Selection {
                texts: vec!["/top".into()],
                hint: "s".into(),
                modifier: Modifier::Shift,
            })
        );
    }

    #[test]
    fn leaving_multi_mode_with_nothing_selected_just_continues() {
        let mut session = session(&["/top"]);
        session.press(Key::Tab);
        assert_eq!(session.press(Key::Tab), Outcome::Continue);
        assert!(!session.is_multi());
    }

    #[test]
    fn escape_cancels_and_help_swallows_the_next_key() {
        let mut session = session(&["/top"]);
        assert_eq!(session.press(Key::Help), Outcome::Continue);
        assert!(session.shows_help());
        assert_eq!(
            session.press(Key::Hint('a', Modifier::Main)),
            Outcome::Continue
        );
        assert!(!session.shows_help());
        assert_eq!(session.press(Key::Escape), Outcome::Cancelled);
    }
}
