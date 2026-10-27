use crate::config;

/// A predefined search in the spirit of tmux-copycat's defaults, or a
/// user pattern from config. Activated from the pattern menu (`p` then
/// the key) or directly with alt+key.
#[derive(Debug, Clone)]
pub struct PatternDef {
    pub key: char,
    pub name: String,
    pub regex: String,
}

/// The copycat-style defaults, in menu order. Each name begins with its
/// selection key so the menu can render "[u]rl" from "url".
pub fn default_patterns() -> Vec<PatternDef> {
    [
        (
            'u',
            "url",
            r#"(https?|ftp|git|ssh|file)://[^\s"'<>()\[\]]+"#,
        ),
        // Paths need at least one slash; a :line[:col] suffix from
        // compiler or grep output is included in the match.
        ('f', "file", r"[\w.~-]*(/[\w.#$%&+=@~-]+)+(:\d+(:\d+)?)?"),
        ('g', "git-sha", r"\b[0-9a-f]{7,40}\b"),
        // Loose copycat-level IPv4: octets are not range-checked.
        ('i', "ip", r"\b\d{1,3}(\.\d{1,3}){3}\b"),
        ('d', "digits", r"\b\d+\b"),
        ('q', "quoted", r#""[^"]+"|'[^']+'"#),
    ]
    .into_iter()
    .map(|(key, name, regex)| PatternDef {
        key,
        name: name.to_string(),
        regex: regex.to_string(),
    })
    .collect()
}

/// The active pattern set: copycat defaults with user overrides applied.
#[derive(Debug, Clone)]
pub struct Patterns {
    defs: Vec<PatternDef>,
}

impl Default for Patterns {
    fn default() -> Self {
        Patterns {
            defs: default_patterns(),
        }
    }
}

impl Patterns {
    /// Apply flat-TOML `pattern_<k>` overrides to the defaults: a regex
    /// value replaces an existing key's regex (its name is kept) or adds
    /// a new pattern named by its key; an empty value disables that key.
    pub fn from_config(toml: &str) -> Self {
        let mut defs = default_patterns();
        for (key, regex) in config::pattern_overrides(toml) {
            if regex.is_empty() {
                defs.retain(|d| d.key != key);
            } else if let Some(d) = defs.iter_mut().find(|d| d.key == key) {
                d.regex = regex;
            } else {
                defs.push(PatternDef {
                    key,
                    name: key.to_string(),
                    regex,
                });
            }
        }
        Patterns { defs }
    }

    pub fn by_key(&self, key: char) -> Option<&PatternDef> {
        self.defs.iter().find(|d| d.key == key)
    }

    /// One-line menu for the bottom row, e.g. "pattern: [u]rl [f]ile ...".
    /// A default name begins with its key, stripped so it is not shown
    /// twice; user patterns render as just "[k]".
    pub fn menu_line(&self) -> String {
        let mut out = "pattern:".to_string();
        for d in &self.defs {
            let suffix = d.name.strip_prefix(d.key).unwrap_or(&d.name);
            out.push_str(&format!(" [{}]{}", d.key, suffix));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    fn matches(name: &str, hay: &str) -> Vec<String> {
        let defs = default_patterns();
        let def = defs.iter().find(|d| d.name == name).unwrap();
        Regex::new(&def.regex)
            .unwrap()
            .find_iter(hay)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    #[test]
    fn url_matches_schemes_and_stops_at_delimiters() {
        assert_eq!(
            matches("url", "see https://x.dev/a?b=1 and (git://h/r.git)"),
            vec!["https://x.dev/a?b=1", "git://h/r.git"]
        );
        assert!(matches("url", "no urls here").is_empty());
    }

    #[test]
    fn file_matches_paths_with_line_suffixes() {
        assert_eq!(
            matches(
                "file",
                "error at src/app.rs:12:5, see /var/log/x.log and ~/dl/a.txt"
            ),
            vec!["src/app.rs:12:5", "/var/log/x.log", "~/dl/a.txt"]
        );
        assert!(matches("file", "Cargo.toml alone").is_empty());
    }

    #[test]
    fn git_sha_needs_seven_hex_chars() {
        assert_eq!(matches("git-sha", "commit 0a3773dbe"), vec!["0a3773dbe"]);
        assert!(matches("git-sha", "abc123").is_empty());
    }

    #[test]
    fn ip_matches_dotted_quads() {
        assert_eq!(
            matches("ip", "host 192.168.0.1:8080 up"),
            vec!["192.168.0.1"]
        );
        assert!(matches("ip", "1.2.3").is_empty());
    }

    #[test]
    fn digits_and_quoted() {
        assert_eq!(matches("digits", "took 42 ms"), vec!["42"]);
        assert_eq!(
            matches("quoted", r#"say "hi there" or 'bye'"#),
            vec![r#""hi there""#, "'bye'"]
        );
    }

    #[test]
    fn default_menu_lists_every_pattern_by_key() {
        let p = Patterns::default();
        assert_eq!(
            p.menu_line(),
            "pattern: [u]rl [f]ile [g]it-sha [i]p [d]igits [q]uoted"
        );
        for d in &default_patterns() {
            assert!(p.by_key(d.key).is_some());
        }
    }

    #[test]
    fn from_config_adds_a_custom_pattern() {
        let p = Patterns::from_config("pattern_t = \"\\bTODO\\b\"\n");
        let def = p.by_key('t').expect("custom pattern present");
        assert_eq!(def.regex, r"\bTODO\b");
        assert!(p.menu_line().contains("[t]"));
    }

    #[test]
    fn from_config_empty_value_disables_a_default() {
        let p = Patterns::from_config("pattern_u = \"\"\n");
        assert!(p.by_key('u').is_none());
        assert!(!p.menu_line().contains("[u]"));
    }

    #[test]
    fn from_config_override_replaces_regex_keeps_name() {
        let p = Patterns::from_config("pattern_d = \"X+\"\n");
        let def = p.by_key('d').expect("digits key still present");
        assert_eq!(def.regex, "X+");
        assert_eq!(def.name, "digits");
        assert!(p.menu_line().contains("[d]igits"));
    }
}
