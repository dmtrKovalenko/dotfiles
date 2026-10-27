//! What counts as "something worth copying": the built-in patterns ported
//! from tmux-fingers plus whatever the user adds, compiled once per session.

use regex::Regex;
use thiserror::Error;

/// A named regular expression. A `match` capture group narrows the copied
/// text to that group (the rest of the match is only context).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternSpec {
    pub name: String,
    pub regex: String,
}

impl PatternSpec {
    pub fn new(name: impl Into<String>, regex: impl Into<String>) -> Self {
        PatternSpec {
            name: name.into(),
            regex: regex.into(),
        }
    }
}

const KUBERNETES_KINDS: &str = "deployment.app|binding|componentstatuse|configmap|endpoint|event|limitrange|namespace|node|persistentvolumeclaim|persistentvolume|pod|podtemplate|replicationcontroller|resourcequota|secret|serviceaccount|service|mutatingwebhookconfiguration.admissionregistration.k8s.io|validatingwebhookconfiguration.admissionregistration.k8s.io|customresourcedefinition.apiextension.k8s.io|apiservice.apiregistration.k8s.io|controllerrevision.apps|daemonset.apps|deployment.apps|replicaset.apps|statefulset.apps|tokenreview.authentication.k8s.io|localsubjectaccessreview.authorization.k8s.io|selfsubjectaccessreviews.authorization.k8s.io|selfsubjectrulesreview.authorization.k8s.io|subjectaccessreview.authorization.k8s.io|horizontalpodautoscaler.autoscaling|cronjob.batch|job.batch|certificatesigningrequest.certificates.k8s.io|events.events.k8s.io|daemonset.extensions|deployment.extensions|ingress.extensions|networkpolicies.extensions|podsecuritypolicies.extensions|replicaset.extensions|networkpolicie.networking.k8s.io|poddisruptionbudget.policy|clusterrolebinding.rbac.authorization.k8s.io|clusterrole.rbac.authorization.k8s.io|rolebinding.rbac.authorization.k8s.io|role.rbac.authorization.k8s.io|storageclasse.storage.k8s.io";

/// The tmux-fingers built-ins, in the order they take precedence when two
/// of them start at the same column.
pub const BUILTIN_PATTERNS: &[(&str, &str)] = &[
    ("ip", r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}"),
    (
        "uuid",
        r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
    ),
    ("sha", r"[0-9a-f]{7,128}"),
    ("digit", r"[0-9]{4,}"),
    (
        "url",
        r#"((https?://|git@|git://|ssh://|ftp://|file:///)[^\s()"']+)"#,
    ),
    ("path", r"(([.\w\-~\$@]+)?(/[.\w\-@]+)+/?)"),
    ("hex", r"(0x[0-9a-fA-F]+)"),
    ("kubernetes", "KUBERNETES_KINDS_PLACEHOLDER"),
    // Deployment-managed pod names such as "nginx-deployment-66b6c48dd5-7xb2r":
    // the generated suffixes use a restricted alphabet (no vowels, no 0/1/3).
    (
        "kubernetes-pod",
        r"[a-z][a-z0-9-]*[a-z0-9]-[bcdfghjklmnpqrstvwxz2456789]{5,10}-[bcdfghjklmnpqrstvwxz2456789]{5}",
    ),
    (
        "git-status",
        r"(modified|deleted|deleted by us|new file): +(?P<match>.+)",
    ),
    (
        "git-status-branch",
        r"Your branch is up to date with '(?P<match>.*)'.",
    ),
    ("diff", r"(---|\+\+\+) [ab]/(?P<match>.*)"),
];

/// The names of every built-in pattern, in precedence order.
pub fn builtin_names() -> Vec<&'static str> {
    BUILTIN_PATTERNS.iter().map(|(name, _)| *name).collect()
}

/// The regular expression behind a built-in pattern name.
pub fn builtin_regex(name: &str) -> Option<String> {
    BUILTIN_PATTERNS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, regex)| match *regex {
            "KUBERNETES_KINDS_PLACEHOLDER" => {
                format!("({KUBERNETES_KINDS})[[:alnum:]_#$%&+=/@-]+")
            }
            other => other.to_string(),
        })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PatternError {
    #[error("unknown built-in pattern `{0}` (known: {known})", known = builtin_names().join(", "))]
    UnknownBuiltin(String),
    #[error("pattern `{name}` is not a valid regular expression: {reason}")]
    InvalidRegex { name: String, reason: String },
    #[error("pattern names must be unique; `{0}` appears twice")]
    DuplicateName(String),
}

#[derive(Debug, Clone)]
struct Pattern {
    name: String,
    regex: Regex,
}

/// A match found in a logical line, in byte offsets of that line's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMatch {
    pub pattern: String,
    /// The whole regular expression match; nothing else may overlap it.
    pub start: usize,
    pub end: usize,
    /// The part worth copying: the `match` group when present, else the whole.
    pub capture_start: usize,
    pub capture_end: usize,
}

/// The compiled, ordered patterns of one session.
#[derive(Debug, Clone, Default)]
pub struct PatternSet {
    patterns: Vec<Pattern>,
}

impl PatternSet {
    /// Every built-in pattern, in precedence order.
    pub fn builtin() -> Self {
        Self::compile(&builtin_specs(&builtin_names()).expect("built-ins are valid"))
            .expect("built-ins compile")
    }

    /// Compiles `specs` in order; earlier patterns win ties at a column.
    pub fn compile(specs: &[PatternSpec]) -> Result<Self, PatternError> {
        let mut patterns: Vec<Pattern> = Vec::with_capacity(specs.len());
        for spec in specs {
            if patterns.iter().any(|existing| existing.name == spec.name) {
                return Err(PatternError::DuplicateName(spec.name.clone()));
            }
            let regex = Regex::new(&spec.regex).map_err(|error| PatternError::InvalidRegex {
                name: spec.name.clone(),
                reason: error.to_string(),
            })?;
            patterns.push(Pattern {
                name: spec.name.clone(),
                regex,
            });
        }
        Ok(PatternSet { patterns })
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.patterns.iter().map(|p| p.name.as_str()).collect()
    }

    /// Non-overlapping matches in `text`, leftmost first; at equal starts the
    /// earlier pattern wins, then the longer match. Trailing whitespace is
    /// trimmed from the captured part so a `.+` group never copies padding.
    pub fn find(&self, text: &str) -> Vec<TextMatch> {
        let mut candidates: Vec<(usize, TextMatch)> = Vec::new();
        for (priority, pattern) in self.patterns.iter().enumerate() {
            for captures in pattern.regex.captures_iter(text) {
                let whole = captures.get(0).expect("group 0 always exists");
                let capture = captures.name("match").unwrap_or(whole);
                let (capture_start, capture_end) =
                    trim_trailing_whitespace(text, capture.start(), capture.end());
                if capture_start >= capture_end {
                    continue;
                }
                candidates.push((
                    priority,
                    TextMatch {
                        pattern: pattern.name.clone(),
                        start: whole.start(),
                        end: whole.end(),
                        capture_start,
                        capture_end,
                    },
                ));
            }
        }
        candidates.sort_by(|(priority_a, a), (priority_b, b)| {
            a.start
                .cmp(&b.start)
                .then(priority_a.cmp(priority_b))
                .then(b.end.cmp(&a.end))
        });
        let mut accepted: Vec<TextMatch> = Vec::new();
        let mut cursor = 0;
        for (_, candidate) in candidates {
            if candidate.start < cursor {
                continue;
            }
            cursor = candidate.end;
            accepted.push(candidate);
        }
        accepted
    }
}

fn trim_trailing_whitespace(text: &str, start: usize, end: usize) -> (usize, usize) {
    let trimmed = text[start..end].trim_end();
    (start, start + trimmed.len())
}

/// Resolves built-in names to specs, preserving the built-in precedence order
/// rather than the order the user listed them in.
pub fn builtin_specs(names: &[&str]) -> Result<Vec<PatternSpec>, PatternError> {
    for name in names {
        if builtin_regex(name).is_none() {
            return Err(PatternError::UnknownBuiltin((*name).to_string()));
        }
    }
    Ok(BUILTIN_PATTERNS
        .iter()
        .filter(|(name, _)| names.contains(name))
        .map(|(name, _)| PatternSpec::new(*name, builtin_regex(name).expect("known")))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured<'a>(set: &PatternSet, text: &'a str) -> Vec<(&'static str, &'a str)> {
        set.find(text)
            .into_iter()
            .map(|m| {
                let name = builtin_names()
                    .into_iter()
                    .find(|n| *n == m.pattern)
                    .unwrap_or("custom");
                (name, &text[m.capture_start..m.capture_end])
            })
            .collect()
    }

    #[test]
    fn every_builtin_pattern_compiles() {
        let set = PatternSet::builtin();
        assert_eq!(set.names().len(), BUILTIN_PATTERNS.len());
    }

    #[test]
    fn each_builtin_recognises_its_canonical_example() {
        let set = PatternSet::builtin();
        let cases = [
            ("ip", "host 192.168.0.1 up", "192.168.0.1"),
            (
                "uuid",
                "id 550e8400-e29b-41d4-a716-446655440000 ok",
                "550e8400-e29b-41d4-a716-446655440000",
            ),
            (
                "sha",
                "commit 8b1a9953c4611299a820df69698463c3ca01599d",
                "8b1a9953c4611299a820df69698463c3ca01599d",
            ),
            ("digit", "pid 48213 running", "48213"),
            (
                "url",
                "see https://example.com/a?b=1 now",
                "https://example.com/a?b=1",
            ),
            ("path", "edit src/main.rs please", "src/main.rs"),
            ("hex", "addr 0xDEADBEEF", "0xDEADBEEF"),
            (
                "kubernetes",
                "configmap-app-settings ready",
                "configmap-app-settings",
            ),
            (
                "kubernetes-pod",
                "pod nginx-deployment-66b6c48dd5-7xb2r ready",
                "nginx-deployment-66b6c48dd5-7xb2r",
            ),
            ("git-status", "\tmodified:   src/lib.rs", "src/lib.rs"),
            (
                "git-status-branch",
                "Your branch is up to date with 'origin/main'.",
                "origin/main",
            ),
            ("diff", "--- a/src/lib.rs", "src/lib.rs"),
        ];
        for (name, text, expected) in cases {
            let found = captured(&set, text);
            assert!(
                found.contains(&(name, expected)),
                "{name}: expected {expected:?} in {found:?}"
            );
        }
    }

    #[test]
    fn a_diff_header_copies_the_path_without_the_a_prefix() {
        let set = PatternSet::builtin();
        assert_eq!(
            captured(&set, "+++ b/src/domain/x.rs"),
            vec![("diff", "src/domain/x.rs")]
        );
    }

    #[test]
    fn a_git_status_line_does_not_copy_trailing_padding() {
        let set = PatternSet::builtin();
        assert_eq!(
            captured(&set, "\tnew file:   docs/README.md        "),
            vec![("git-status", "docs/README.md")]
        );
    }

    #[test]
    fn matches_never_overlap_and_the_leftmost_wins() {
        let set = PatternSet::builtin();
        let found = captured(&set, "https://x.io/abcdef0 1234 /etc/hosts");
        assert_eq!(
            found,
            vec![
                ("url", "https://x.io/abcdef0"),
                ("digit", "1234"),
                ("path", "/etc/hosts")
            ]
        );
    }

    #[test]
    fn a_uuid_is_not_split_into_shas() {
        let set = PatternSet::builtin();
        let found = captured(&set, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "uuid");
    }

    #[test]
    fn custom_patterns_join_the_builtins_and_may_narrow_with_a_match_group() {
        let mut specs = builtin_specs(&["url"]).unwrap();
        specs.push(PatternSpec::new("ticket", r"PROJ-(?P<match>[0-9]+)"));
        let set = PatternSet::compile(&specs).unwrap();
        let text = "fixes PROJ-42 via https://j.ira/PROJ-42";
        let found: Vec<&str> = set
            .find(text)
            .iter()
            .map(|m| &text[m.capture_start..m.capture_end])
            .collect();
        assert_eq!(found, vec!["42", "https://j.ira/PROJ-42"]);
    }

    #[test]
    fn unknown_builtin_names_and_bad_regexes_are_reported() {
        assert_eq!(
            builtin_specs(&["nope"]),
            Err(PatternError::UnknownBuiltin("nope".into()))
        );
        let error = PatternSet::compile(&[PatternSpec::new("bad", "(")]).unwrap_err();
        assert!(matches!(error, PatternError::InvalidRegex { name, .. } if name == "bad"));
        let error =
            PatternSet::compile(&[PatternSpec::new("dup", "a"), PatternSpec::new("dup", "b")])
                .unwrap_err();
        assert_eq!(error, PatternError::DuplicateName("dup".into()));
    }

    #[test]
    fn enabling_a_subset_keeps_the_builtin_precedence_order() {
        let specs = builtin_specs(&["path", "url", "ip"]).unwrap();
        let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["ip", "url", "path"]);
    }
}
