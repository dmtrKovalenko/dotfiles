//! The kernel under `src/domain` and the use cases under `src/usecases` may
//! not know about the terminal, Herdr, the clipboard, processes or files.
//! This is the Dependency Rule, enforced.

use std::path::Path;

const FORBIDDEN_INSIDE: &[&str] = &[
    "crate::adapters",
    "crate::app",
    "ratatui",
    "crossterm",
    "serde_json",
    "std::process",
    "std::fs",
    "std::io",
    "std::net",
    "std::os",
    "std::env",
    "base64",
    "shell_words",
];

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("readable directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn violations_under(ring: &str, extra_forbidden: &[&str]) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(ring);
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    assert!(!files.is_empty(), "no files found under {ring}");
    let mut violations = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file).expect("readable source");
        for (number, line) in source.lines().enumerate() {
            for forbidden in FORBIDDEN_INSIDE.iter().chain(extra_forbidden) {
                if line.contains(forbidden) {
                    violations.push(format!(
                        "{}:{}: uses `{forbidden}`",
                        file.display(),
                        number + 1
                    ));
                }
            }
        }
    }
    violations
}

#[test]
fn the_domain_depends_on_nothing_outside_itself() {
    let violations = violations_under("src/domain", &["crate::usecases"]);
    assert!(
        violations.is_empty(),
        "the kernel reaches outward:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_use_cases_depend_only_on_the_domain_and_their_ports() {
    let violations = violations_under("src/usecases", &[]);
    assert!(
        violations.is_empty(),
        "a use case reaches outward:\n{}",
        violations.join("\n")
    );
}
