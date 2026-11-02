//! The keys hints are made of, per keyboard layout (ported from tmux-fingers).

use thiserror::Error;

/// Keys that would clash with the overlay's own controls (`q` quits, and
/// `c`, `i`, `m`, `n` collide with Ctrl+C, Tab, Enter and Ctrl+N).
pub const RESERVED_KEYS: &[char] = &['c', 'i', 'm', 'q', 'n'];

/// tmux-fingers' layouts, best keys first.
pub const LAYOUTS: &[(&str, &str)] = &[
    ("qwerty", "asdfqwerzxcvjklmiuopghtybn"),
    ("qwerty-homerow", "asdfjklgh"),
    ("qwerty-left-hand", "asdfqwerzcxv"),
    ("qwerty-right-hand", "jkluiopmyhn"),
    ("azerty", "qsdfazerwxcvjklmuiopghtybn"),
    ("azerty-homerow", "qsdfjkmgh"),
    ("azerty-left-hand", "qsdfazerwxcv"),
    ("azerty-right-hand", "jklmuiophyn"),
    ("qwertz", "asdfqweryxcvjkluiopmghtzbn"),
    ("qwertz-homerow", "asdfghjkl"),
    ("qwertz-left-hand", "asdfqweryxcv"),
    ("qwertz-right-hand", "jkluiopmhzn"),
    ("dvorak", "aoeuqjkxpyhtnsgcrlmwvzfidb"),
    ("dvorak-homerow", "aoeuhtnsid"),
    ("dvorak-left-hand", "aoeupqjkyix"),
    ("dvorak-right-hand", "htnsgcrlmwvz"),
    ("colemak", "arstqwfpzxcvneioluymdhgjbk"),
    ("colemak-homerow", "arstneiodh"),
    ("colemak-left-hand", "arstqwfpzxcv"),
    ("colemak-right-hand", "neioluymjhk"),
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AlphabetError {
    #[error("unknown keyboard layout `{0}` (known: {known})", known = LAYOUTS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "))]
    UnknownLayout(String),
    #[error("an alphabet needs at least two distinct usable keys, got `{0}`")]
    TooShort(String),
    #[error("alphabet keys must be lowercase ASCII letters or digits, got `{0}`")]
    InvalidKey(char),
}

/// The ordered, de-duplicated keys hints are built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alphabet {
    keys: Vec<char>,
}

impl Alphabet {
    /// A tmux-fingers layout by name, minus the reserved keys.
    pub fn layout(name: &str) -> Result<Alphabet, AlphabetError> {
        let keys = LAYOUTS
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name.trim()))
            .map(|(_, keys)| *keys)
            .ok_or_else(|| AlphabetError::UnknownLayout(name.trim().to_string()))?;
        Alphabet::custom(keys)
    }

    /// Any string of lowercase letters/digits; reserved keys and duplicates
    /// are dropped silently, order is kept.
    pub fn custom(keys: &str) -> Result<Alphabet, AlphabetError> {
        let mut unique: Vec<char> = Vec::new();
        for key in keys.chars() {
            if !(key.is_ascii_lowercase() || key.is_ascii_digit()) {
                return Err(AlphabetError::InvalidKey(key));
            }
            if RESERVED_KEYS.contains(&key) || unique.contains(&key) {
                continue;
            }
            unique.push(key);
        }
        if unique.len() < 2 {
            return Err(AlphabetError::TooShort(keys.to_string()));
        }
        Ok(Alphabet { keys: unique })
    }

    pub fn keys(&self) -> &[char] {
        &self.keys
    }

    pub fn contains(&self, key: char) -> bool {
        self.keys.contains(&key)
    }
}

impl Default for Alphabet {
    fn default() -> Self {
        Alphabet::layout("qwerty").expect("the qwerty layout is valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_drop_the_reserved_keys_but_keep_their_order() {
        let qwerty = Alphabet::layout("qwerty").unwrap();
        let keys: String = qwerty.keys().iter().collect();
        assert_eq!(keys, "asdfwerzxvjkluopghtyb");
        assert!(!qwerty.contains('q'));
        assert!(!qwerty.contains('c'));
    }

    #[test]
    fn every_layout_is_usable() {
        for (name, _) in LAYOUTS {
            let alphabet = Alphabet::layout(name).unwrap();
            assert!(alphabet.keys().len() >= 2, "{name}");
        }
    }

    #[test]
    fn layout_names_are_case_insensitive_and_unknown_ones_are_listed() {
        assert!(Alphabet::layout(" AZERTY ").is_ok());
        assert_eq!(
            Alphabet::layout("bepo"),
            Err(AlphabetError::UnknownLayout("bepo".into()))
        );
    }

    #[test]
    fn custom_alphabets_reject_uppercase_and_too_few_keys() {
        assert_eq!(Alphabet::custom("aB"), Err(AlphabetError::InvalidKey('B')));
        assert_eq!(
            Alphabet::custom("aac"),
            Err(AlphabetError::TooShort("aac".into()))
        );
        assert_eq!(
            Alphabet::custom("asdf").unwrap().keys(),
            &['a', 's', 'd', 'f']
        );
    }
}
