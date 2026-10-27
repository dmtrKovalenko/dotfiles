use crate::ansi;

/// Plugin configuration from $HERDR_PLUGIN_CONFIG_DIR/config.toml.
/// Parsed with a minimal flat-TOML extractor in the spirit of
/// herdr::json_str_field: one `key = value` per line, `#` comments,
/// scanning stops at the first `[table]` header. No arrays, no escapes.
pub const DEFAULT_EXTRACT_HEIGHT: u8 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Extract popup height as a percentage of the screen, 1-100.
    pub extract_height: u8,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            extract_height: DEFAULT_EXTRACT_HEIGHT,
        }
    }
}

impl Config {
    pub fn from_toml(text: &str) -> Self {
        let height = toml_int_field(text, "extract_height")
            .unwrap_or(DEFAULT_EXTRACT_HEIGHT as i64)
            .clamp(1, 100) as u8;
        Config {
            extract_height: height,
        }
    }
}

/// Foreground/background color override for one themed UI element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorPair {
    pub fg: ansi::Color,
    pub bg: ansi::Color,
}

/// Colors for the themed UI chrome. Defaults reproduce the values that
/// were hardcoded in ui.rs; matching config.toml keys override them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    // copy / search
    pub badge_copy: ColorPair,
    pub badge_search: ColorPair,
    pub match_: ColorPair,
    pub match_current: ColorPair,
    pub cursor: ColorPair,
    pub selection_bg: ansi::Color,
    pub indicator: ColorPair,
    // extract
    pub badge_word: ColorPair,
    pub badge_line: ColorPair,
    pub extract_selected: ColorPair,
}

impl Default for Theme {
    fn default() -> Self {
        // ANSI palette indices matching the crossterm colors ui.rs used:
        // dark_yellow=3, green=10, yellow=11, blue=12, magenta=13,
        // cyan=14, white=15, on a black (0) foreground.
        let on = |bg| ColorPair {
            fg: ansi::Color::Indexed(0),
            bg: ansi::Color::Indexed(bg),
        };
        Theme {
            badge_copy: on(14),
            badge_search: on(13),
            match_: on(3),
            match_current: on(11),
            cursor: on(14),
            selection_bg: ansi::Color::Rgb(60, 64, 71),
            indicator: on(11),
            badge_word: on(12),
            badge_line: on(10),
            extract_selected: on(15),
        }
    }
}

impl Theme {
    /// Start from the defaults, then apply any `<item>_fg`/`<item>_bg`
    /// overrides present in the flat config. Missing or unrecognized
    /// values keep the default (see parse_color).
    pub fn from_toml(toml: &str) -> Self {
        let mut t = Theme::default();
        override_pair(toml, &mut t.badge_copy, "badge_copy");
        override_pair(toml, &mut t.badge_search, "badge_search");
        override_pair(toml, &mut t.match_, "match");
        override_pair(toml, &mut t.match_current, "match_current");
        override_pair(toml, &mut t.cursor, "cursor");
        if let Some(bg) = toml_str_field(toml, "selection_bg").and_then(|v| parse_color(&v)) {
            t.selection_bg = bg;
        }
        override_pair(toml, &mut t.indicator, "indicator");
        override_pair(toml, &mut t.badge_word, "badge_word");
        override_pair(toml, &mut t.badge_line, "badge_line");
        override_pair(toml, &mut t.extract_selected, "extract_selected");
        t
    }
}

/// Apply `<prefix>_fg` / `<prefix>_bg` color overrides onto `pair`.
fn override_pair(toml: &str, pair: &mut ColorPair, prefix: &str) {
    if let Some(c) = toml_str_field(toml, &format!("{prefix}_fg")).and_then(|v| parse_color(&v)) {
        pair.fg = c;
    }
    if let Some(c) = toml_str_field(toml, &format!("{prefix}_bg")).and_then(|v| parse_color(&v)) {
        pair.bg = c;
    }
}

/// Parse a config color value: `#rrggbb`, a 0-255 palette index, or a
/// crossterm-style color name. None on anything unrecognized so the
/// caller keeps its default.
pub fn parse_color(s: &str) -> Option<ansi::Color> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(ansi::Color::Rgb(r, g, b));
    }
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        return s.parse::<u8>().ok().map(ansi::Color::Indexed);
    }
    named_color(s).map(ansi::Color::Indexed)
}

/// Map a crossterm-style color name to its ANSI palette index.
fn named_color(name: &str) -> Option<u8> {
    Some(match name {
        "black" => 0,
        "dark_red" => 1,
        "dark_green" => 2,
        "dark_yellow" => 3,
        "dark_blue" => 4,
        "dark_magenta" => 5,
        "dark_cyan" => 6,
        "grey" | "gray" => 7,
        "dark_grey" | "dark_gray" => 8,
        "red" => 9,
        "green" => 10,
        "yellow" => 11,
        "blue" => 12,
        "magenta" => 13,
        "cyan" => 14,
        "white" => 15,
        _ => return None,
    })
}

/// Raw text of $HERDR_PLUGIN_CONFIG_DIR/config.toml, or "" when the dir
/// or file is missing. Callers derive both Config and Patterns from it.
pub fn read_config_text(env: impl Fn(&str) -> Option<String>) -> String {
    env("HERDR_PLUGIN_CONFIG_DIR")
        .map(|dir| std::path::Path::new(&dir).join("config.toml"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default()
}

/// Missing dir, file, or key all fall back to defaults.
pub fn load(env: impl Fn(&str) -> Option<String>) -> Config {
    Config::from_toml(&read_config_text(env))
}

/// Raw trimmed value for a top-level key, inline comment stripped;
/// quoted values keep their quotes for toml_str_field to remove.
fn toml_value<'a>(toml: &'a str, key: &str) -> Option<&'a str> {
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            return None;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        let v = v.trim();
        if let Some(rest) = v.strip_prefix('"') {
            let end = rest.find('"')?;
            return Some(&v[..end + 2]);
        }
        return Some(v.split('#').next().unwrap_or("").trim());
    }
    None
}

pub fn toml_int_field(toml: &str, key: &str) -> Option<i64> {
    toml_value(toml, key)?.parse().ok()
}

/// All `pattern_<k> = "regex"` overrides in document order, where `<k>`
/// is a single menu-key char. An empty value (`pattern_u = ""`) is kept
/// as an empty string, the caller's signal to disable a default. Scans
/// flat keys only, stopping at the first `[table]` header.
pub fn pattern_overrides(toml: &str) -> Vec<(char, String)> {
    let mut out = Vec::new();
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let Some(suffix) = k.trim().strip_prefix("pattern_") else {
            continue;
        };
        let mut chars = suffix.chars();
        let (Some(key), None) = (chars.next(), chars.next()) else {
            continue;
        };
        let v = v.trim();
        let value = if let Some(rest) = v.strip_prefix('"') {
            // Quoted: take up to the closing quote, ignoring any trailing
            // inline comment, mirroring toml_value.
            let end = rest.find('"').unwrap_or(rest.len());
            rest[..end].to_string()
        } else {
            v.split('#').next().unwrap_or("").trim().to_string()
        };
        out.push((key, value));
    }
    out
}

pub fn toml_str_field(toml: &str, key: &str) -> Option<String> {
    let v = toml_value(toml, key)?;
    let inner = v
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(v);
    Some(inner.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_field_parses_flat_keys_and_comments() {
        let toml = "# heading\nextract_height = 60 # inline\nother = 2\n";
        assert_eq!(toml_int_field(toml, "extract_height"), Some(60));
        assert_eq!(toml_int_field(toml, "other"), Some(2));
        assert_eq!(toml_int_field(toml, "missing"), None);
    }

    #[test]
    fn scan_stops_at_table_header() {
        let toml = "a = 1\n[table]\nb = 2\n";
        assert_eq!(toml_int_field(toml, "a"), Some(1));
        assert_eq!(toml_int_field(toml, "b"), None);
    }

    #[test]
    fn str_field_strips_quotes() {
        let toml = "name = \"hello # not a comment\"\nbare = word\n";
        assert_eq!(
            toml_str_field(toml, "name").as_deref(),
            Some("hello # not a comment")
        );
        assert_eq!(toml_str_field(toml, "bare").as_deref(), Some("word"));
    }

    #[test]
    fn pattern_overrides_collects_single_char_keys() {
        // No escape processing: the value is verbatim between the quotes.
        let toml = "extract_height = 50\n\
                    pattern_j = \"\\bTODO\\b\" # tasks\n\
                    pattern_u = \"\"\n\
                    pattern_multi = \"ignored\"\n\
                    [table]\n\
                    pattern_x = \"nope\"\n";
        assert_eq!(
            pattern_overrides(toml),
            vec![('j', "\\bTODO\\b".to_string()), ('u', String::new())]
        );
    }

    #[test]
    fn from_toml_clamps_and_defaults() {
        assert_eq!(Config::from_toml("").extract_height, 40);
        assert_eq!(Config::from_toml("extract_height = 60").extract_height, 60);
        assert_eq!(Config::from_toml("extract_height = 0").extract_height, 1);
        assert_eq!(
            Config::from_toml("extract_height = 500").extract_height,
            100
        );
    }

    #[test]
    fn load_without_config_dir_defaults() {
        assert_eq!(load(|_| None), Config::default());
    }

    #[test]
    fn parse_color_handles_hex_index_and_names() {
        assert_eq!(
            parse_color("#5fafff"),
            Some(ansi::Color::Rgb(0x5f, 0xaf, 0xff))
        );
        assert_eq!(parse_color("14"), Some(ansi::Color::Indexed(14)));
        assert_eq!(parse_color(" cyan "), Some(ansi::Color::Indexed(14)));
        assert_eq!(parse_color("dark_yellow"), Some(ansi::Color::Indexed(3)));
        assert_eq!(parse_color("gray"), Some(ansi::Color::Indexed(7)));
        assert_eq!(parse_color("300"), None, "index out of u8 range");
        assert_eq!(parse_color("#fff"), None, "hex must be 6 digits");
        assert_eq!(parse_color("bogus"), None);
    }

    #[test]
    fn theme_from_toml_defaults_then_overrides() {
        assert_eq!(Theme::from_toml(""), Theme::default());
        let toml = "badge_copy_bg = \"red\"\n\
                    match_current_fg = \"#000000\"\n\
                    extract_selected_bg = \"12\"\n\
                    selection_bg = \"#112233\"\n";
        let t = Theme::from_toml(toml);
        let d = Theme::default();
        assert_eq!(t.badge_copy.bg, ansi::Color::Indexed(9));
        assert_eq!(t.badge_copy.fg, d.badge_copy.fg, "fg untouched");
        assert_eq!(t.match_current.fg, ansi::Color::Rgb(0, 0, 0));
        assert_eq!(t.extract_selected.bg, ansi::Color::Indexed(12));
        assert_eq!(t.selection_bg, ansi::Color::Rgb(17, 34, 51));
    }

    #[test]
    fn theme_from_toml_keeps_default_on_invalid_and_stops_at_table() {
        let toml = "badge_copy_bg = \"bogus\"\n[colors]\nmatch_bg = \"red\"\n";
        let t = Theme::from_toml(toml);
        let d = Theme::default();
        assert_eq!(
            t.badge_copy.bg, d.badge_copy.bg,
            "invalid value kept default"
        );
        assert_eq!(t.match_.bg, d.match_.bg, "keys after [table] are ignored");
    }
}
