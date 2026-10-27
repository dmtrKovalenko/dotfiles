use super::{Result, support::*};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicI32, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_MESSAGE: usize = 16384;
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop_signal(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}
fn signals() {
    // The handler only updates a lock-free atomic; normal control flow cleans up.
    unsafe {
        libc::signal(
            libc::SIGTERM,
            stop_signal as *const () as libc::sighandler_t,
        );
        libc::signal(libc::SIGINT, stop_signal as *const () as libc::sighandler_t);
    }
}
fn terminate(pid: i32) {
    if pid > 0 {
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
}
pub fn configuration() -> Result<Value> {
    let path = config_dir()?.join("herdr/plugins/config/fingers-open/browser.json");
    if path.exists() {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    } else {
        Ok(json!({"hosts":[], "browser":"chrome"}))
    }
}
pub fn web_url(text: &str) -> bool {
    if text.len() > 8192
        || text.chars().any(char::is_control)
        || !(text.to_ascii_lowercase().starts_with("https://")
            || text.to_ascii_lowercase().starts_with("http://"))
    {
        return false;
    }
    url::Url::parse(text)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}
fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
pub fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && !host.starts_with('-')
        && !host.chars().any(char::is_whitespace)
        && !host.chars().any(char::is_control)
}
fn launch_browser(url: &str, browser: &str) -> Result<()> {
    if !web_url(url) {
        return Err("Only complete HTTP and HTTPS URLs can open a browser".into());
    }
    let mut command = if cfg!(target_os = "macos") {
        Command::new("/usr/bin/open")
    } else {
        Command::new("xdg-open")
    };
    if cfg!(target_os = "macos") && browser == "chrome" {
        command.args(["-b", "com.google.Chrome"]);
    }
    let result = output(command.arg(url), Duration::from_secs(10))?;
    if !result.status.success() {
        return Err("The browser could not open this URL on the client".into());
    }
    Ok(())
}
pub fn request_bridge(url: &str) -> Result<()> {
    if !web_url(url) {
        return Err("Only HTTP and HTTPS URLs can open a browser".into());
    }
    let directory = state_dir()?.join("browsers");
    let mut connections = vec![];
    if directory.is_dir() {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "sock")
                && let Ok(connection) = UnixStream::connect(path)
            {
                connections.push(connection);
            }
        }
    }
    if connections.is_empty() {
        return Err(
            "Local browser bridge is disconnected. Start Fingers Open on your Mac and retry."
                .into(),
        );
    }
    if connections.len() != 1 {
        return Err("More than one computer is connected to the browser bridge. Disconnect the extra bridge before opening a URL.".into());
    }
    let mut connection = connections.pop().unwrap();
    connection.set_read_timeout(Some(Duration::from_secs(15)))?;
    connection.set_write_timeout(Some(Duration::from_secs(3)))?;
    let id = identifier()?;
    emit(&mut connection, &json!({"id":id,"url":url}))?;
    let mut reader = BufReader::new(connection);
    let reply: Value =
        serde_json::from_str(&read_line(&mut reader, MAX_MESSAGE)?.ok_or("Browser disconnected")?)?;
    if reply["id"] != id || reply["ok"] != true {
        return Err(reply["error"]
            .as_str()
            .unwrap_or("Local browser did not acknowledge the request")
            .into());
    }
    Ok(())
}
pub fn open_browser(url: &str) -> Result<()> {
    let config = configuration()?;
    if !cfg!(target_os = "macos")
        || config["role"] == "remote"
        || std::env::var("HERDR_FINGERS_BROWSER_REMOTE").as_deref() == Ok("1")
    {
        request_bridge(url)
    } else {
        launch_browser(url, config["browser"].as_str().unwrap_or("chrome"))
    }
}
fn relay_request(
    connection: &mut UnixStream,
    input: &mut impl std::io::BufRead,
    output: &mut impl Write,
) -> Result<()> {
    connection.set_read_timeout(Some(Duration::from_secs(3)))?;
    connection.set_write_timeout(Some(Duration::from_secs(3)))?;
    let raw = read_line(&mut BufReader::new(connection.try_clone()?), MAX_MESSAGE)?;
    let Some(raw) = raw else {
        return Ok(());
    };
    let message: Value = serde_json::from_str(&raw)?;
    let id = field(&message, "id")?;
    let url = field(&message, "url")?;
    if !valid_id(id) || !web_url(url) {
        return Err("Invalid browser request".into());
    }
    emit(output, &json!({"type":"open","id":id,"url":url}))?;
    if !ready(&[0], Duration::from_secs(12))?[0] {
        return Err("The local browser bridge did not reply".into());
    }
    let reply: Value = serde_json::from_str(
        &read_line(input, MAX_MESSAGE)?.ok_or("Browser client disconnected")?,
    )?;
    if reply["id"] != id || !reply["ok"].is_boolean() {
        return Err("The local browser bridge returned an unexpected reply".into());
    }
    emit(connection, &reply)
}
pub fn relay(client: &str) -> Result<()> {
    if !valid_id(client) {
        return Err("Invalid browser client identifier".into());
    }
    signals();
    let directory = state_dir()?.join("browsers");
    private_dir(&directory)?;
    let _lock = Lock::acquire(&directory.join(format!("{client}.lock")))?;
    let path = directory.join(format!("{client}.sock"));
    if path.exists() {
        fs::remove_file(&path)?;
    }
    let _guard = RemoveFile(path.clone());
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    let mut input = BufReader::new(std::io::stdin());
    let mut output = std::io::stdout();
    emit(&mut output, &json!({"type":"ready","client":client}))?;
    while !STOP.load(Ordering::Relaxed) {
        let active = ready(&[listener.as_raw_fd(), 0], Duration::from_millis(500))?;
        if active[1] && read_line(&mut input, MAX_MESSAGE)?.is_none() {
            break;
        }
        if !active[0] {
            continue;
        }
        let (mut connection, _) = listener.accept()?;
        if let Err(error) = relay_request(&mut connection, &mut input, &mut output) {
            let _ = emit(
                &mut connection,
                &json!({"ok":false,"error":error.to_string()}),
            );
        }
    }
    Ok(())
}
struct Worker {
    stopped: Arc<AtomicBool>,
    pid: Arc<AtomicI32>,
    status: Arc<Mutex<String>>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    fn start(host: String, client: String) -> Self {
        let stopped = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(AtomicI32::new(0));
        let status = Arc::new(Mutex::new("connecting".to_string()));
        let stop_copy = stopped.clone();
        let pid_copy = pid.clone();
        let status_copy = status.clone();
        let thread =
            thread::spawn(move || worker_loop(&host, &client, &stop_copy, &pid_copy, &status_copy));
        Self {
            stopped,
            pid,
            status,
            thread: Some(thread),
        }
    }
    fn stop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        terminate(self.pid.load(Ordering::Relaxed));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}
fn worker_connection(
    host: &str,
    client: &str,
    stop: &AtomicBool,
    pid: &AtomicI32,
    status: &Mutex<String>,
) -> Result<()> {
    let remote = format!(
        "exec \"${{XDG_DATA_HOME:-$HOME/.local/share}}/herdr/fingers-open/runtime/herdr-fingers\" browser relay --client {client}"
    );
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(state_dir()?.join("bridge-ssh.log"))?;
    let mut child = Command::new("/usr/bin/ssh")
        .args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ControlMaster=no",
            "-o",
            "ControlPath=none",
            "-o",
            "ControlPersist=no",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=2",
            "--",
            host,
            &remote,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(log)
        .spawn()?;
    pid.store(child.id() as i32, Ordering::Relaxed);
    if stop.load(Ordering::Relaxed) {
        terminate(child.id() as i32);
    }
    let mut input = child.stdin.take().ok_or("Missing SSH stdin")?;
    let mut reader = BufReader::new(child.stdout.take().ok_or("Missing SSH stdout")?);
    let result = worker_messages(&mut reader, &mut input, stop, status);
    terminate(child.id() as i32);
    let deadline = Instant::now() + Duration::from_secs(3);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = child.wait();
    pid.store(0, Ordering::Relaxed);
    result
}
fn worker_messages(
    reader: &mut impl std::io::BufRead,
    input: &mut impl Write,
    stopped: &AtomicBool,
    status: &Mutex<String>,
) -> Result<()> {
    while !stopped.load(Ordering::Relaxed) {
        let Some(line) = read_line(reader, MAX_MESSAGE)? else {
            break;
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match message["type"].as_str() {
            Some("ready") => *status.lock().unwrap() = "connected".into(),
            Some("open") => {
                let config = configuration()?;
                let result = launch_browser(
                    message["url"].as_str().unwrap_or(""),
                    config["browser"].as_str().unwrap_or("chrome"),
                );
                let reply = match result {
                    Ok(()) => json!({"id":message["id"],"ok":true}),
                    Err(_) => {
                        json!({"id":message["id"],"ok":false,"error":"The browser could not open this URL on the client."})
                    }
                };
                emit(input, &reply)?;
            }
            _ => (),
        }
    }
    Ok(())
}
fn worker_loop(
    host: &str,
    client: &str,
    stopped: &AtomicBool,
    pid: &AtomicI32,
    status: &Mutex<String>,
) {
    while !stopped.load(Ordering::Relaxed) {
        *status.lock().unwrap() = "connecting".into();
        let _ = worker_connection(host, client, stopped, pid, status);
        *status.lock().unwrap() = "disconnected".into();
        for _ in 0..50 {
            if stopped.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}
pub fn serve() -> Result<()> {
    signals();
    let root = state_dir()?;
    let _lock = Lock::acquire(&root.join("bridge.lock"))?;
    let client_file = root.join("browser-client-id");
    if !client_file.exists() {
        atomic_write(&client_file, identifier()?.as_bytes(), 0o600)?;
    }
    let client = fs::read_to_string(client_file)?.trim().to_string();
    if !valid_id(&client) {
        return Err("Invalid saved browser client identifier".into());
    }
    let report = root.join("bridge-status.json");
    let _guard = RemoveFile(report.clone());
    let mut workers: BTreeMap<String, Worker> = BTreeMap::new();
    while !STOP.load(Ordering::Relaxed) {
        let config = configuration()?;
        let hosts: Vec<String> = config["hosts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|host| valid_host(host))
            .map(str::to_string)
            .collect();
        workers.retain(|host, _| hosts.contains(host));
        for host in hosts {
            workers
                .entry(host.clone())
                .or_insert_with(|| Worker::start(host, client.clone()));
        }
        let states: BTreeMap<&str, String> = workers
            .iter()
            .map(|(host, worker)| (host.as_str(), worker.status.lock().unwrap().clone()))
            .collect();
        write_json(
            &report,
            &json!({"pid":std::process::id(),"updated":now(),"client":client,"hosts":states,"runtime":"rust"}),
        )?;
        for _ in 0..20 {
            if STOP.load(Ordering::Relaxed) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}
pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("serve") => serve(),
        Some("relay") if args.len() == 3 && args[1] == "--client" => relay(&args[2]),
        Some("status") => {
            let path = state_dir()?.join("bridge-status.json");
            println!(
                "{}",
                fs::read_to_string(path).unwrap_or_else(|_| "{\"status\":\"stopped\"}".into())
            );
            Ok(())
        }
        Some("request") if args.len() == 2 => request_bridge(&args[1]),
        Some("start") => {
            let root = state_dir()?;
            if Lock::acquire(&root.join("bridge.lock")).is_err() {
                return Ok(());
            }
            let log = OpenOptions::new()
                .append(true)
                .create(true)
                .mode(0o600)
                .open(root.join("bridge.log"))?;
            Command::new(std::env::current_exe()?)
                .args(["browser", "serve"])
                .stdin(Stdio::null())
                .stdout(log.try_clone()?)
                .stderr(log)
                .process_group(0)
                .spawn()?;
            Ok(())
        }
        _ => Err("Usage: herdr-fingers browser serve|start|status|relay --client ID".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_complete_http_urls_are_accepted() {
        for url in [
            "https://example.com/a?q=1#x",
            "http://localhost:1234",
            "https://[::1]/",
        ] {
            assert!(web_url(url), "{url}");
        }
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https:/bad",
            "http://x\nabc",
            "https://[bad",
        ] {
            assert!(!web_url(url), "{url}");
        }
        assert!(!valid_host("-oProxyCommand=bad"));
        assert!(!valid_host("host with spaces"));
        assert!(valid_host("user@sloppi"));
    }
}
