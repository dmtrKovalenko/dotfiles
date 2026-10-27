use super::{Result, bridge, support::*};
use crate::adapters::herdr::HerdrClient;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const LUA: &str = include_str!("../../../herdr_fingers.lua");
const LABEL: &str = "com.dkovalenko.herdr-fingers-browser";
const PATTERNS: &[(&str, &str)] = &[
    (
        "fingers-open-home",
        r"(?:\$HOME|\$\{HOME\})(?:/[\w@.+-]+)+/?(?::[0-9]+(?:-[0-9]+|:[0-9]+)?|#L[0-9]+(?:-L?[0-9]+)?)?",
    ),
    (
        "fingers-open-location",
        r"(?:~?/|\.{1,2}/)?[\w@.+-]+(?:/[\w@.+-]+)*(?::[0-9]+(?:-[0-9]+|:[0-9]+)?|#L[0-9]+(?:-L?[0-9]+)?)",
    ),
    (
        "fingers-open-quoted-file",
        r#"["`'](?P<match>(?:\.{0,2}/|~/|/)?[^"`'\r\n]*[\w-]+\.[\w+-]+(?::[0-9]+(?::[0-9]+)?|#L[0-9]+(?:-L?[0-9]+)?)?)["`']"#,
    ),
    (
        "fingers-open-filename",
        r"\b[\w.-]+\.[A-Za-z][A-Za-z0-9_+-]{0,15}\b",
    ),
    (
        "fingers-open-known-file",
        r"\b(?:Makefile|Dockerfile|README|LICENSE|Justfile|justfile)\b",
    ),
];

pub fn runtime() -> Result<PathBuf> {
    Ok(data_dir()?.join("herdr/fingers-open/runtime"))
}

pub fn configure_fingers(original: &str) -> Result<String> {
    let mut text = original.to_string();
    for (key, value) in [
        ("main_action", ":fingers-open:"),
        ("ctrl_action", ":copy:"),
        ("multi_separator", "\n"),
    ] {
        let expression = Regex::new(&format!(r"(?m)^{key}\s*=.*$"))?;
        let replacement = format!("{key} = {}", serde_json::to_string(value)?);
        if expression.is_match(&text) {
            text = expression
                .replace(&text, regex::NoExpand(&replacement))
                .into_owned();
        } else {
            text = format!("{replacement}\n{text}");
        }
    }
    let parsed: toml::Value = toml::from_str(&text)?;
    let existing: BTreeSet<&str> = parsed
        .get("patterns")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.get("name").and_then(toml::Value::as_str))
        .collect();
    for (name, pattern) in PATTERNS {
        if !existing.contains(name) {
            text.push_str(&format!(
                "\n[[patterns]]\nname = {}\nregex = {}\n",
                serde_json::to_string(name)?,
                serde_json::to_string(pattern)?
            ));
        }
    }
    let _: toml::Value = toml::from_str(&text)?;
    Ok(text)
}

fn backup_root() -> Result<PathBuf> {
    let root = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".cache"))
        .join("herdr-fingers-open-backups")
        .join(format!("{}-{}", now() as u64, identifier()?));
    private_dir(&root)?;
    Ok(root)
}

fn backup(path: &Path, destination: &Path) -> Result<()> {
    if path.is_file() {
        fs::create_dir_all(destination.parent().ok_or("Missing backup parent")?)?;
        fs::copy(path, destination)?;
    }
    Ok(())
}

fn checked(command: &mut Command) -> Result<()> {
    let result = output(command, Duration::from_secs(30))?;
    if !result.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<()> {
    let mut role = None;
    let mut hosts = Vec::new();
    let default_herdr = home()?.join(".local/bin/herdr");
    let mut herdr = if default_herdr.is_file() {
        default_herdr
    } else {
        PathBuf::from("herdr")
    };
    let mut arguments = args.iter();
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or("Every install option requires a value")?;
        match flag.as_str() {
            "--role" if matches!(value.as_str(), "local" | "remote") => role = Some(value.clone()),
            "--host" if bridge::valid_host(value) => hosts.push(value.clone()),
            "--herdr" => herdr = value.into(),
            _ => return Err(format!("Invalid install option: {flag}").into()),
        }
    }
    let role =
        role.ok_or("Specify --role local on the browser Mac or --role remote on a devbox")?;
    if role == "local" && !cfg!(target_os = "macos") {
        return Err("The local browser role requires macOS".into());
    }
    if role != "local" && !hosts.is_empty() {
        return Err("--host requires --role local".into());
    }
    let config = config_dir()?;
    let init = config.join("nvim/init.lua");
    let mut init_text = fs::read_to_string(&init)
        .map_err(|_| "Set up Neovim's init.lua before installing Fingers")?;
    let registration = "require(\"herdr_fingers\").setup()";
    if !init_text.contains(registration) {
        init_text.push_str(&format!(
            "\n-- Register this editor for Herdr file hints.\n{registration}\n"
        ));
    }
    let fingers_path = config.join("herdr/plugins/config/nathan-poncet.herdr-fingers/config.toml");
    let original = if fingers_path.exists() {
        fs::read_to_string(&fingers_path)?
    } else {
        "clipboard = \"osc52\"\n".into()
    };
    let fingers = configure_fingers(&original)?;
    let browser_path = config.join("herdr/plugins/config/fingers-open/browser.json");
    let mut browser: Value = if browser_path.exists() {
        serde_json::from_slice(&fs::read(&browser_path)?)?
    } else {
        json!({})
    };
    let object = browser
        .as_object_mut()
        .ok_or("Browser configuration must be an object")?;
    object.insert("role".into(), json!(role));
    object.entry("browser").or_insert(json!("chrome"));
    let entries = object
        .entry("hosts")
        .or_insert(json!([]))
        .as_array_mut()
        .ok_or("Browser hosts must be an array")?;
    for host in hosts {
        if !entries.contains(&json!(host)) {
            entries.push(json!(host));
        }
    }
    let root = runtime()?;
    let executable = root.join("herdr-fingers");
    let binary = fs::read(std::env::current_exe()?)?;
    let backups = backup_root()?;
    for relative in [
        "nvim/init.lua",
        "nvim/lua/herdr_fingers.lua",
        "herdr/plugins/config/nathan-poncet.herdr-fingers/config.toml",
        "herdr/plugins/config/fingers-open/browser.json",
        "herdr/plugins.json",
    ] {
        backup(&config.join(relative), &backups.join(relative))?;
    }
    backup(&executable, &backups.join("runtime/herdr-fingers"))?;
    backup(
        &root.join("herdr-plugin.toml"),
        &backups.join("runtime/herdr-plugin.toml"),
    )?;
    atomic_write(&executable, &binary, 0o755)?;
    let manifest = include_str!("../../herdr-plugin.toml")
        .replace(
            "[[build]]\ncommand = [\"cargo\", \"build\", \"--release\", \"--locked\"]\n",
            "",
        )
        .replace("./target/release/herdr-fingers", "./herdr-fingers");
    atomic_write(&root.join("herdr-plugin.toml"), manifest.as_bytes(), 0o644)?;
    // Keep the existing companion plugin ID and its key bindings; both manifests use one executable.
    let companion = root.join("companion");
    let program = serde_json::to_string(&executable)?;
    let companion_manifest = format!(
        "id = \"fingers-open\"\nname = \"Fingers Open\"\nversion = \"0.2.0\"\nmin_herdr_version = \"0.9.1\"\nplatforms = [\"macos\", \"linux\"]\n\n[[actions]]\nid = \"browser-start\"\ntitle = \"Start local browser bridge\"\nplatforms = [\"macos\"]\ncommand = [{program}, \"browser\", \"start\"]\n\n[[actions]]\nid = \"browser-status\"\ntitle = \"Browser bridge status\"\ncommand = [{program}, \"browser\", \"status\"]\n"
    );
    atomic_write(
        &companion.join("herdr-plugin.toml"),
        companion_manifest.as_bytes(),
        0o644,
    )?;
    atomic_write(
        &config.join("nvim/lua/herdr_fingers.lua"),
        LUA.as_bytes(),
        0o644,
    )?;
    atomic_write(&init, init_text.as_bytes(), 0o644)?;
    atomic_write(&fingers_path, fingers.as_bytes(), 0o600)?;
    write_json(&browser_path, &browser)?;
    checked(Command::new(&herdr).args(["plugin", "link"]).arg(&root))?;
    checked(
        Command::new(&herdr)
            .args(["plugin", "link"])
            .arg(&companion),
    )?;
    println!(
        "Installed: {}\nBackup: {}",
        executable.display(),
        backups.display()
    );
    Ok(())
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn browser_service() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err("Install the browser service on the Mac".into());
    }
    let executable = runtime()?.join("herdr-fingers");
    if !executable.is_file() {
        return Err("Run install --role local first".into());
    }
    let root = state_dir()?;
    let destination = home()?.join(format!("Library/LaunchAgents/{LABEL}.plist"));
    let path = format!(
        "/opt/homebrew/bin:{}/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        home()?.display()
    );
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{LABEL}</string>
<key>ProgramArguments</key><array><string>{}</string><string>browser</string><string>serve</string></array>
<key>RunAtLoad</key><true/><key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
<key>ThrottleInterval</key><integer>30</integer><key>ProcessType</key><string>Background</string>
<key>EnvironmentVariables</key><dict><key>PATH</key><string>{}</string></dict>
<key>StandardOutPath</key><string>{}</string><key>StandardErrorPath</key><string>{}</string>
</dict></plist>
"#,
        xml(&executable.to_string_lossy()),
        xml(&path),
        xml(&root.join("bridge-service.log").to_string_lossy()),
        xml(&root.join("bridge-service.log").to_string_lossy())
    );
    backup(&destination, &backup_root()?.join("browser-service.plist"))?;
    atomic_write(&destination, plist.as_bytes(), 0o644)?;
    checked(
        Command::new("/usr/bin/plutil")
            .arg("-lint")
            .arg(&destination),
    )?;
    // getuid has no preconditions and does not mutate process state.
    let domain = format!("gui/{}", unsafe { libc::getuid() });
    let target = format!("{domain}/{LABEL}");
    if output(
        Command::new("/bin/launchctl").args(["print", &target]),
        Duration::from_secs(5),
    )?
    .status
    .success()
    {
        checked(Command::new("/bin/launchctl").args(["bootout", &target]))?;
    }
    // bootout can return before launchd has finished removing the old job.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let result = output(
            Command::new("/bin/launchctl")
                .args(["bootstrap", &domain])
                .arg(&destination),
            Duration::from_secs(5),
        )?;
        if result.status.success() {
            break;
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Could not start browser service: {}",
                String::from_utf8_lossy(&result.stderr)
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("Installed and started: {}", destination.display());
    Ok(())
}

fn editor_processes(pid: u64) -> Result<BTreeSet<u64>> {
    let result = output(
        Command::new("/bin/ps").args(["-axo", "pid=,ppid=,comm="]),
        Duration::from_secs(5),
    )?;
    let mut children: BTreeMap<u64, Vec<(u64, bool)>> = BTreeMap::new();
    for line in String::from_utf8_lossy(&result.stdout).lines() {
        let mut words = line.split_whitespace();
        if let (Some(child), Some(parent), Some(name)) = (words.next(), words.next(), words.next())
            && let (Ok(child), Ok(parent)) = (child.parse(), parent.parse())
        {
            children.entry(parent).or_default().push((
                child,
                Path::new(name).file_name().is_some_and(|n| n == "nvim"),
            ));
        }
    }
    let mut found = BTreeSet::from([pid]);
    let mut pending = vec![pid];
    while let Some(parent) = pending.pop() {
        for &(child, nvim) in children.get(&parent).into_iter().flatten() {
            pending.push(child);
            if nvim {
                found.insert(child);
            }
        }
    }
    Ok(found)
}

fn sockets_for_pid(pid: u64) -> Result<Vec<PathBuf>> {
    if cfg!(target_os = "linux") {
        let inodes: BTreeSet<String> = fs::read_dir(format!("/proc/{pid}/fd"))?
            .flatten()
            .filter_map(|entry| fs::read_link(entry.path()).ok())
            .filter_map(|path| {
                path.to_str()?
                    .strip_prefix("socket:[")?
                    .strip_suffix(']')
                    .map(str::to_string)
            })
            .collect();
        return Ok(fs::read_to_string("/proc/net/unix")?
            .lines()
            .filter_map(|line| {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() == 8 && inodes.contains(fields[6]) && fields[7].starts_with('/') {
                    Some(fields[7].into())
                } else {
                    None
                }
            })
            .collect());
    }
    let result = output(
        Command::new("/usr/sbin/lsof").args(["-a", "-p", &pid.to_string(), "-U", "-Fn"]),
        Duration::from_secs(5),
    )?;
    Ok(String::from_utf8_lossy(&result.stdout)
        .lines()
        .filter_map(|line| {
            line.strip_prefix('n')
                .filter(|s| s.starts_with('/'))
                .map(PathBuf::from)
        })
        .collect())
}

fn reload_editor(
    executable: &Path,
    socket: &Path,
    pids: &BTreeSet<u64>,
    pane: &str,
    host_path: &Path,
) -> Result<bool> {
    if !socket.metadata()?.file_type().is_socket() {
        return Ok(false);
    }
    let query = "luaeval('vim.json.encode({pid=vim.fn.getpid(),pane=vim.env.HERDR_PANE_ID,host=vim.env.HERDR_SOCKET_PATH})')";
    let result = output(
        Command::new(executable)
            .arg("--server")
            .arg(socket)
            .args(["--remote-expr", query]),
        Duration::from_secs(2),
    )?;
    if !result.status.success() {
        return Ok(false);
    }
    let current: Value = serde_json::from_slice(&result.stdout)?;
    if !current["pid"]
        .as_u64()
        .is_some_and(|pid| pids.contains(&pid))
        || current["pane"].as_str() != Some(pane)
    {
        return Ok(false);
    }
    if fs::canonicalize(field(&current, "host")?)? != fs::canonicalize(host_path)? {
        return Ok(false);
    }
    let module = serde_json::to_string(&config_dir()?.join("nvim/lua/herdr_fingers.lua"))?;
    let lua =
        format!("lua local m = dofile({module}); package.loaded.herdr_fingers = m; m.setup()");
    let expression = format!("execute('{}')", lua.replace('\'', "''"));
    checked(
        Command::new(executable)
            .arg("--server")
            .arg(socket)
            .args(["--remote-expr", &expression]),
    )?;
    Ok(true)
}

pub fn register_existing() -> Result<()> {
    let host_path = std::env::var_os("HERDR_SOCKET_PATH")
        .map(PathBuf::from)
        .unwrap_or(config_dir()?.join("herdr/herdr.sock"));
    let host = HerdrClient::new(&host_path);
    let panes = host.call("pane.list", json!({}))?;
    let mut updated = BTreeSet::new();
    for pane in panes["panes"].as_array().ok_or("Missing panes")? {
        let pane_id = field(pane, "pane_id")?;
        let info = host.call("pane.process_info", json!({"pane_id":pane_id}))?;
        for process in info["process_info"]["foreground_processes"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if !process["name"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .contains("nvim")
            {
                continue;
            }
            let mut executables: Vec<PathBuf> = process["argv0"]
                .as_str()
                .or_else(|| process["argv"][0].as_str())
                .map(PathBuf::from)
                .into_iter()
                .collect();
            executables.extend([
                home()?.join(".local/bin/nvim"),
                PathBuf::from("/opt/homebrew/bin/nvim"),
            ]);
            let Some(executable) = executables.into_iter().find(|p| p.is_file()) else {
                continue;
            };
            let pids = editor_processes(process["pid"].as_u64().ok_or("Missing editor PID")?)?;
            let sockets: Vec<PathBuf> = pids
                .iter()
                .filter_map(|pid| sockets_for_pid(*pid).ok())
                .flatten()
                .collect();
            for socket in sockets {
                if reload_editor(&executable, &socket, &pids, pane_id, &host_path).unwrap_or(false)
                {
                    updated.insert(pane_id.to_string());
                    break;
                }
            }
        }
    }
    println!(
        "Registered existing Neovim panes: {}",
        updated.into_iter().collect::<Vec<_>>().join(", ")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_is_idempotent_and_preserves_custom_settings() {
        let original = "# custom\nclipboard = \"osc52\"\nshift_action = \":paste:\"\nmain_action = \"python3 old.py\"\n\n[[patterns]]\nname = \"mine\"\nregex = \"hello\"\n";
        let configured = configure_fingers(original).unwrap();
        assert_eq!(configured, configure_fingers(&configured).unwrap());
        assert!(configured.contains("# custom"));
        let value: toml::Value = toml::from_str(&configured).unwrap();
        assert_eq!(value["main_action"].as_str(), Some(":fingers-open:"));
        assert_eq!(value["shift_action"].as_str(), Some(":paste:"));
        assert_eq!(value["patterns"].as_array().unwrap().len(), 6);
        for (_, pattern) in PATTERNS {
            Regex::new(pattern).unwrap();
        }
    }
}
