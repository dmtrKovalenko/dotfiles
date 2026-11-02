use super::{Result, bridge, support::*};
use crate::adapters::herdr::{HerdrClient, HerdrError};
use base64::Engine;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::thread;
use std::time::{Duration, Instant};

static LOCATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.*?)(?::(\d+)(?:-(\d+)|:(\d+))?|#L(\d+)(?:-L?(\d+))?)$").unwrap()
});
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reference {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub end_line: Option<usize>,
    pub kind: String,
}

fn expand_home(text: &str, home: &Path) -> String {
    for prefix in ["${HOME}", "$HOME", "~"] {
        if text == prefix {
            return home.to_string_lossy().into_owned();
        }
        if let Some(suffix) = text.strip_prefix(&format!("{prefix}/")) {
            return home.join(suffix).to_string_lossy().into_owned();
        }
    }
    text.to_string()
}
pub fn parse_reference(text: &str, home: &Path) -> Option<Reference> {
    let mut text = text.trim().trim_matches(['`', '\'', '"']).to_string();
    if text.starts_with("file://") {
        let mut url = url::Url::parse(&text).ok()?;
        let hostname = std::env::var("HOSTNAME").unwrap_or_default();
        if url
            .host_str()
            .is_some_and(|host| host != "localhost" && host != hostname)
        {
            return None;
        }
        let fragment = url.fragment().map(str::to_owned);
        url.set_host(None).ok()?;
        text = url.to_file_path().ok()?.to_string_lossy().into_owned();
        if let Some(fragment) = fragment {
            text.push('#');
            text.push_str(&fragment);
        }
    }
    if text.is_empty() || text.contains(['\0', '\n', '\r']) {
        return None;
    }
    text = expand_home(&text, home);
    let mut reference = Reference {
        path: text.clone(),
        line: 1,
        column: 1,
        end_line: None,
        kind: "file".into(),
    };
    if let Some(parts) = LOCATION.captures(&text) {
        let number = |index| {
            parts
                .get(index)
                .and_then(|part| part.as_str().parse::<usize>().ok())
        };
        reference.path = parts[1].into();
        reference.line = number(2).or_else(|| number(5)).unwrap_or(1).max(1);
        reference.column = number(4).unwrap_or(1).max(1);
        reference.end_line = number(3).or_else(|| number(6));
    }
    Some(reference)
}
pub fn project_root(path: &Path) -> PathBuf {
    match output(
        Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "--show-toplevel"]),
        Duration::from_secs(3),
    ) {
        Ok(result) if result.status.success() => {
            PathBuf::from(String::from_utf8_lossy(&result.stdout).trim())
        }
        _ => path.to_owned(),
    }
}
pub fn resolve_reference(
    text: &str,
    directory: &Path,
    workspace: Option<&Path>,
    home: &Path,
) -> Option<Reference> {
    let reference = parse_reference(text, home)?;
    let mut roots = vec![directory.to_owned(), project_root(directory)];
    if let Some(workspace) = workspace {
        roots.push(workspace.to_owned());
    }
    let raw = expand_home(text.trim().trim_matches(['`', '\'', '"']), home);
    for (spelling, located) in [(raw.as_str(), false), (reference.path.as_str(), true)] {
        let mut variants = vec![spelling];
        if spelling.starts_with("a/") || spelling.starts_with("b/") {
            variants.push(&spelling[2..]);
        }
        variants.push(spelling.trim_end_matches(['.', ',', ';']));
        for variant in variants {
            let path = Path::new(variant);
            let candidates: Vec<PathBuf> = if path.is_absolute() {
                vec![path.to_owned()]
            } else {
                roots.iter().map(|root| root.join(path)).collect()
            };
            for candidate in candidates {
                if !(candidate.is_file() || candidate.is_dir()) {
                    continue;
                }
                if let Ok(path) = candidate.canonicalize() {
                    let mut found = reference.clone();
                    found.path = path.to_string_lossy().into_owned();
                    found.kind = if path.is_dir() { "directory" } else { "file" }.into();
                    if !located {
                        found.line = 1;
                        found.column = 1;
                        found.end_line = None;
                    }
                    return Some(found);
                }
            }
        }
    }
    None
}
pub fn reference_candidates(
    text: &str,
    directory: &Path,
    workspace: Option<&Path>,
    editors: &[Value],
    home: &Path,
) -> Vec<Reference> {
    if let Some(found) = resolve_reference(text, directory, workspace, home) {
        return vec![found];
    }
    let Some(reference) = parse_reference(text, home) else {
        return vec![];
    };
    let mut relative = reference.path.as_str();
    if Path::new(relative).is_absolute() || relative.starts_with('~') || relative.contains("://") {
        return vec![];
    }
    if !(relative.contains(['/', '.'])
        || [
            "Makefile",
            "Dockerfile",
            "README",
            "LICENSE",
            "Justfile",
            "justfile",
        ]
        .contains(&relative))
    {
        return vec![];
    }
    if relative.starts_with("a/") || relative.starts_with("b/") || relative.starts_with("./") {
        relative = &relative[2..];
    }
    let mut matches = BTreeSet::new();
    for editor in editors {
        if let Some(buffers) = editor["buffers"].as_array() {
            for name in buffers.iter().filter_map(Value::as_str) {
                if name.ends_with(&format!("/{relative}"))
                    && Path::new(name).is_file()
                    && let Ok(path) = fs::canonicalize(name)
                {
                    matches.insert(path);
                }
            }
        }
    }
    let mut roots = BTreeSet::from([project_root(directory)]);
    if let Some(workspace) = workspace {
        roots.insert(project_root(workspace));
    }
    let escaped = relative
        .chars()
        .map(|c| match c {
            '[' => "[[]".into(),
            '*' => "[*]".into(),
            '?' => "[?]".into(),
            _ => c.to_string(),
        })
        .collect::<String>();
    for root in roots {
        let result = output(
            Command::new("git")
                .arg("-C")
                .arg(&root)
                .args([
                    "ls-files",
                    "-z",
                    "--cached",
                    "--others",
                    "--exclude-standard",
                    "--",
                ])
                .arg(format!(":(glob)**/{escaped}")),
            Duration::from_secs(3),
        );
        if let Ok(result) = result {
            if !result.status.success() {
                continue;
            }
            for raw in result
                .stdout
                .split(|byte| *byte == 0)
                .filter(|raw| !raw.is_empty())
            {
                let name = String::from_utf8_lossy(raw);
                if name != relative && !name.ends_with(&format!("/{relative}")) {
                    continue;
                }
                let path = root.join(name.as_ref());
                if path.is_file()
                    && let Ok(path) = path.canonicalize()
                {
                    matches.insert(path);
                }
            }
        }
    }
    matches
        .into_iter()
        .map(|path| Reference {
            path: path.to_string_lossy().into_owned(),
            ..reference.clone()
        })
        .collect()
}

pub fn nvim_call(editor: &Value, method: &str, data: Value) -> Result<Value> {
    if !matches!(method, "open" | "status") {
        return Err("Unknown Neovim operation".into());
    }
    let argument = serde_json::to_string(&data)?.replace('\'', "''");
    let expression =
        format!("luaeval('require(\"herdr_fingers\").{method}(_A)', json_decode('{argument}'))");
    let result = output(
        Command::new(field(editor, "nvim_bin")?).args([
            "--server",
            field(editor, "socket")?,
            "--remote-expr",
            &expression,
        ]),
        Duration::from_secs(6),
    )?;
    if !result.status.success() {
        return Err(format!(
            "Neovim did not accept the request: {}",
            String::from_utf8_lossy(&result.stderr)
                .chars()
                .take(300)
                .collect::<String>()
        )
        .into());
    }
    let response: Value = serde_json::from_slice(&result.stdout)?;
    if response["ok"] != true {
        return Err(response["error"]
            .as_str()
            .unwrap_or("Neovim request failed")
            .into());
    }
    Ok(response)
}
fn pane(host: &HerdrClient, id: &str) -> Result<Value> {
    Ok(host.call("pane.get", json!({"pane_id": id}))?["pane"].clone())
}
fn socket_path() -> Result<PathBuf> {
    Ok(std::env::var_os("HERDR_SOCKET_PATH")
        .map(PathBuf::from)
        .unwrap_or(config_dir()?.join("herdr/herdr.sock")))
}
fn read_editor(path: &Path, host: &HerdrClient, origin: &Value, socket: &Path) -> Result<Value> {
    let mut editor: Value = serde_json::from_slice(&fs::read(path)?)?;
    if fs::canonicalize(field(&editor, "herdr_socket")?)? != fs::canonicalize(socket)? {
        return Err("Different Herdr session".into());
    }
    let info = pane(host, field(&editor, "pane_id")?)?;
    if info["workspace_id"] != origin["workspace_id"] {
        return Err("Different workspace".into());
    }
    let current = nvim_call(&editor, "status", Value::Null)?;
    if current["pid"] != editor["pid"] || current["pane_id"] != editor["pane_id"] {
        return Err("Stale editor registration".into());
    }
    let object = editor.as_object_mut().ok_or("Invalid registration")?;
    object.extend(current.as_object().ok_or("Invalid editor status")?.clone());
    editor["tab_id"] = info["tab_id"].clone();
    Ok(editor)
}
fn find_editors(host: &HerdrClient, origin: &Value) -> Result<Vec<Value>> {
    let directory = state_dir()?.join("nvim");
    let mut editors = vec![];
    if !directory.is_dir() {
        return Ok(editors);
    }
    let socket = socket_path()?;
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json")
            && let Ok(editor) = read_editor(&path, host, origin, &socket)
        {
            editors.push(editor);
        }
    }
    Ok(editors)
}
pub fn editor_pool(editors: &[Value], origin: &Value, filename: &str) -> Vec<usize> {
    let open: Vec<usize> = editors
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e["buffers"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v == filename))
        })
        .map(|(i, _)| i)
        .collect();
    if !open.is_empty() {
        return open;
    }
    let same: Vec<usize> = editors
        .iter()
        .enumerate()
        .filter(|(_, e)| e["tab_id"] == origin["tab_id"])
        .map(|(i, _)| i)
        .collect();
    if !same.is_empty() {
        same
    } else {
        (0..editors.len()).collect()
    }
}
struct RawTerminal;
impl Drop for RawTerminal {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}
fn choose(labels: &[String], title: &str) -> Result<Option<usize>> {
    if labels.is_empty() {
        return Ok(None);
    }
    let mut tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    crossterm::terminal::enable_raw_mode()?;
    let _raw = RawTerminal;
    let mut selected: usize = 0;
    loop {
        write!(
            tty,
            "\x1b[2J\x1b[H{title} ({}/{}; j/k, arrows, Enter, Esc)\r\n\r\n",
            selected + 1,
            labels.len()
        )?;
        let height = crossterm::terminal::size()?.1.saturating_sub(3).max(1) as usize;
        let start = selected
            .saturating_sub(height / 2)
            .min(labels.len().saturating_sub(height));
        for (index, label) in labels.iter().enumerate().skip(start).take(height) {
            let label: String = label
                .chars()
                .filter(|c| !c.is_control())
                .take(160)
                .collect();
            write!(
                tty,
                "{}{label}\r\n",
                if index == selected { " > " } else { "   " }
            )?;
        }
        tty.flush()?;
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => selected = (selected + 1) % labels.len(),
                KeyCode::Char('k') | KeyCode::Up => {
                    selected = (selected + labels.len() - 1) % labels.len()
                }
                KeyCode::Enter => return Ok(Some(selected)),
                KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                _ => (),
            }
        }
    }
}
fn choose_editor(editors: &[Value], origin: &Value, filename: &str) -> Result<Option<Value>> {
    if editors.is_empty() {
        return Err("No Neovim registered in this workspace. Run :HerdrFingersRegister.".into());
    }
    let pool = editor_pool(editors, origin, filename);
    if pool.len() == 1 {
        return Ok(Some(editors[pool[0]].clone()));
    }
    let labels = pool
        .iter()
        .map(|i| {
            format!(
                "{}  {}  {}",
                editors[*i]["pane_id"].as_str().unwrap_or(""),
                editors[*i]["cwd"].as_str().unwrap_or(""),
                editors[*i]["current_file"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>();
    Ok(choose(&labels, "Open in Neovim")?.map(|i| editors[pool[i]].clone()))
}
fn copy_text(text: &str) -> Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    OpenOptions::new()
        .write(true)
        .open("/dev/tty")?
        .write_all(format!("\x1b]52;c;{encoded}\x07").as_bytes())?;
    Ok(())
}
pub fn focus_after_close(socket: &str, overlay: &str, source: &str, target: &str) -> Result<()> {
    let host = HerdrClient::new(socket);
    let deadline = Instant::now() + Duration::from_secs(8);
    while !overlay.is_empty() && overlay != source {
        match host.call("pane.get", json!({"pane_id": overlay})) {
            Err(HerdrError::Api { code, .. }) if code == "pane_not_found" => break,
            Err(error) => return Err(error.into()),
            Ok(_) => (),
        }
        if Instant::now() >= deadline {
            return Err("Hint overlay did not close; editor focus was left unchanged".into());
        }
        thread::sleep(Duration::from_millis(30));
    }
    let current = host.call("pane.current", json!({}))?;
    if [source, target, overlay].contains(&current["pane"]["pane_id"].as_str().unwrap_or("")) {
        host.call("pane.focus", json!({"pane_id": target}))?;
    }
    Ok(())
}
fn focus_editor(host: &HerdrClient, source: &str, target: &str) -> Result<()> {
    let overlay = std::env::var("HERDR_PANE_ID").unwrap_or(source.into());
    if pane(host, source)?["tab_id"] == pane(host, target)?["tab_id"] {
        Command::new(std::env::current_exe()?)
            .arg("focus")
            .arg(socket_path()?)
            .args([&overlay, source, target])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()?;
    } else {
        let current = host.call("pane.current", json!({}))?;
        if [source, target, &overlay].contains(&current["pane"]["pane_id"].as_str().unwrap_or("")) {
            host.call("pane.focus", json!({"pane_id": target}))?;
        }
    }
    Ok(())
}
fn open_reference(
    editors: &[Value],
    origin: &Value,
    reference: &Reference,
) -> Result<Option<String>> {
    let Some(editor) = choose_editor(editors, origin, &reference.path)? else {
        return Ok(None);
    };
    nvim_call(&editor, "open", serde_json::to_value(reference)?)?;
    Ok(Some(field(&editor, "pane_id")?.into()))
}
pub fn open_selections(text: &str) -> Result<()> {
    let host = HerdrClient::from_env()?;
    let source = std::env::var("HERDR_FINGERS_GEOMETRY")
        .ok()
        .and_then(|g| g.split('|').next().map(str::to_string))
        .or_else(|| std::env::var("HERDR_PANE_ID").ok())
        .ok_or("Fingers did not provide its source pane")?;
    let origin = pane(&host, &source)?;
    let directory = PathBuf::from(
        origin["foreground_cwd"]
            .as_str()
            .or(origin["cwd"].as_str())
            .unwrap_or("."),
    );
    let workspace = host.call(
        "workspace.get",
        json!({"workspace_id": origin["workspace_id"]}),
    )?;
    let workspace = workspace["workspace"]["cwd"].as_str().map(Path::new);
    let selections = text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if selections.len() > 32 {
        return Err("Select at most 32 items at once".into());
    }
    let mut copied = vec![];
    let mut target = None;
    let report = state_dir()?.join("last-action.json");
    for selected in selections {
        let mut activity = json!({"time": now(), "source_pane": source, "workspace": origin["workspace_id"], "cwd": directory});
        if bridge::web_url(selected) {
            match bridge::open_browser(selected) {
                Ok(()) => activity["route"] = json!("browser"),
                Err(error) => {
                    copied.push(selected);
                    copy_text(&copied.join("\n"))?;
                    activity["route"] = json!("copied-browser-failed");
                    activity["error"] = json!(error.to_string());
                    let _ = host.notify(
                        "Fingers Open",
                        &format!("Could not open the link on your Mac; copied instead. {error}"),
                    );
                }
            }
            write_json(&report, &activity)?;
            continue;
        }
        activity["selection"] = json!(selected);
        activity["route"] = json!("resolving");
        write_json(&report, &activity)?;
        let editors = find_editors(&host, &origin)?;
        let references = reference_candidates(selected, &directory, workspace, &editors, &home()?);
        if references.is_empty() {
            copied.push(selected);
            copy_text(&copied.join("\n"))?;
            activity["route"] = json!("copied-unresolved");
            write_json(&report, &activity)?;
            if selected.contains(['/', '.']) {
                let _ = host.notify(
                    "Fingers Open",
                    &format!("Could not resolve {selected}; copied instead."),
                );
            }
            continue;
        }
        let index = if references.len() == 1 {
            Some(0)
        } else {
            choose(
                &references
                    .iter()
                    .map(|r| r.path.clone())
                    .collect::<Vec<_>>(),
                "Choose file",
            )?
        };
        let Some(index) = index else {
            continue;
        };
        let reference = &references[index];
        match open_reference(&editors, &origin, reference) {
            Ok(Some(editor)) => {
                activity["route"] = json!("opened");
                activity["path"] = json!(reference.path);
                activity["editor_pane"] = json!(editor);
                target = Some(editor);
            }
            Ok(None) => continue,
            Err(error) => {
                copied.push(selected);
                copy_text(&copied.join("\n"))?;
                activity["route"] = json!("copied-open-failed");
                activity["path"] = json!(reference.path);
                activity["error"] = json!(error.to_string());
                let _ = host.notify(
                    "Fingers Open",
                    &format!(
                        "Could not open {} in Neovim; copied instead. {error}",
                        reference.path
                    ),
                );
            }
        }
        write_json(&report, &activity)?;
    }
    if let Some(target) = target {
        focus_editor(&host, &source, &target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locations_home_and_file_uris_are_data() {
        for (input, line, column, end) in [
            ("src/main.rs:2", 2, 1, None),
            ("src/main.rs:2:3", 2, 3, None),
            ("src/main.rs:2-4", 2, 1, Some(4)),
            ("src/main.rs#L2-L4", 2, 1, Some(4)),
        ] {
            let r = parse_reference(input, Path::new("/home/user")).unwrap();
            assert_eq!((r.line, r.column, r.end_line), (line, column, end));
        }
        assert_eq!(
            parse_reference("${HOME}/a.rs:2", Path::new("/home/user"))
                .unwrap()
                .path,
            "/home/user/a.rs"
        );
        assert_eq!(
            parse_reference("file:///tmp/a%20file%27s.rs#L4", Path::new("/home/user"))
                .unwrap()
                .path,
            "/tmp/a file's.rs"
        );
        assert!(
            parse_reference("file://foreign-host/etc/passwd", Path::new("/home/user")).is_none()
        );
        assert!(parse_reference("a\nb", Path::new("/home/user")).is_none());
    }
    #[test]
    fn editor_choice_prefers_open_buffer_then_same_tab() {
        let editors = vec![
            json!({"tab_id":"t1","buffers":[]}),
            json!({"tab_id":"t2","buffers":["/file"]}),
        ];
        assert_eq!(
            editor_pool(&editors, &json!({"tab_id":"t1"}), "/file"),
            vec![1]
        );
        assert_eq!(
            editor_pool(&editors, &json!({"tab_id":"t1"}), "/other"),
            vec![0]
        );
        assert_eq!(
            editor_pool(&editors, &json!({"tab_id":"t3"}), "/other"),
            vec![0, 1]
        );
        assert!(
            choose_editor(&[], &json!({"tab_id":"t1"}), "/file")
                .unwrap_err()
                .to_string()
                .contains("No Neovim")
        );
    }
    #[test]
    fn resolution_preserves_colon_filenames_and_ambiguity() {
        let root = std::env::temp_dir().join(format!("fingers-resolve-{}", identifier().unwrap()));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("other")).unwrap();
        fs::write(root.join("src/main.rs"), "test").unwrap();
        for (spelling, line, column) in [
            ("a/src/main.rs", 1, 1),
            ("src/main.rs:2:3", 2, 3),
            ("$HOME/src/main.rs:2:3", 2, 3),
            ("${HOME}/src/main.rs:2:3", 2, 3),
            ("~/src/main.rs#L2-L4", 2, 1),
        ] {
            let reference = resolve_reference(spelling, &root, None, &root).unwrap();
            assert_eq!((reference.line, reference.column), (line, column));
            assert_eq!(
                reference.path,
                root.join("src/main.rs")
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
            );
        }
        assert_eq!(
            resolve_reference("src/", &root, None, &root).unwrap().kind,
            "directory"
        );
        let quoted = root.join("a file's name.txt");
        fs::write(&quoted, "hello").unwrap();
        for spelling in [
            format!("\"{}\"", quoted.display()),
            format!("`{}`", quoted.display()),
            format!("{}#L1", url::Url::from_file_path(&quoted).unwrap()),
        ] {
            assert_eq!(
                resolve_reference(&spelling, &root, None, &root)
                    .unwrap()
                    .path,
                quoted.canonicalize().unwrap().to_string_lossy()
            );
        }
        let name = root.join("src/main.rs").canonicalize().unwrap();
        assert_eq!(
            reference_candidates(
                "main.rs:3",
                &root,
                None,
                &[json!({"buffers":[name]})],
                &root
            )[0]
            .line,
            3
        );
        fs::write(root.join("src/main.rs:2"), "literal").unwrap();
        let found = resolve_reference("src/main.rs:2", &root, None, &root).unwrap();
        assert!(found.path.ends_with("main.rs:2"));
        assert_eq!(found.line, 1);
        Command::new("git")
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .unwrap();
        assert!(resolve_reference("src/main.rs:2:3", &root.join("src"), None, &root).is_some());
        assert_eq!(
            reference_candidates("main.rs:2:3", &root, None, &[], &root).len(),
            1
        );
        fs::write(root.join("other/main.rs"), "test").unwrap();
        assert_eq!(
            reference_candidates("main.rs:2:3", &root, None, &[], &root).len(),
            2
        );
        assert!(resolve_reference("missing.rs:2", &root, None, &root).is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
