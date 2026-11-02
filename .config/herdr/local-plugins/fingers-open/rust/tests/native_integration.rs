use herdr_fingers::custom::{navigation::nvim_call, support::identifier};
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Fixture {
    root: PathBuf,
    children: Vec<Child>,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!("hf-{}", &identifier().unwrap()[..12]));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        Self {
            root,
            children: vec![],
        }
    }
    fn relay(&mut self, client: &str) -> BufReader<std::process::ChildStdout> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-fingers"))
            .args(["browser", "relay", "--client", client])
            .env("HERDR_FINGERS_OPEN_STATE", &self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&line).unwrap()["type"],
            "ready"
        );
        self.children.push(child);
        reader
    }
    fn request(&self, url: &str) -> Child {
        Command::new(env!("CARGO_BIN_EXE_herdr-fingers"))
            .args(["browser", "request", url])
            .env("HERDR_FINGERS_OPEN_STATE", &self.root)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn browser_transport_requires_matching_ack_and_cleans_up_on_disconnect() {
    let mut fixture = Fixture::new();
    let client = "0123456789abcdef0123456789abcdef";
    let mut reader = fixture.relay(client);
    let socket = fixture.root.join(format!("browsers/{client}.sock"));
    assert_eq!(
        fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let mut request = fixture.request("https://example.com/a?q=';$HOME");
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let message: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(message["url"], "https://example.com/a?q=';$HOME");
    assert!(request.try_wait().unwrap().is_none());
    writeln!(
        fixture.children[0].stdin.as_mut().unwrap(),
        "{}",
        json!({"id":message["id"],"ok":true})
    )
    .unwrap();
    assert!(request.wait().unwrap().success());

    let request = fixture.request("https://example.com/reject");
    line.clear();
    reader.read_line(&mut line).unwrap();
    writeln!(
        fixture.children[0].stdin.as_mut().unwrap(),
        "{}",
        json!({"id":"wrong","ok":true})
    )
    .unwrap();
    let result = request.wait_with_output().unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("unexpected reply"));

    let mut invalid = UnixStream::connect(&socket).unwrap();
    writeln!(
        invalid,
        "{}",
        json!({"id":client,"url":"file:///etc/passwd"})
    )
    .unwrap();
    invalid
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    line.clear();
    BufReader::new(invalid).read_line(&mut line).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["ok"], false);
    drop(fixture.children[0].stdin.take());
    assert!(fixture.children[0].wait().unwrap().success());
    assert!(!socket.exists());
    let result = fixture
        .request("https://example.com")
        .wait_with_output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("disconnected"));
}

#[test]
fn two_browser_clients_are_refused_and_duplicate_relay_cannot_remove_active_socket() {
    let mut fixture = Fixture::new();
    let first = "0123456789abcdef0123456789abcdef";
    let _reader = fixture.relay(first);
    let duplicate = Command::new(env!("CARGO_BIN_EXE_herdr-fingers"))
        .args(["browser", "relay", "--client", first])
        .env("HERDR_FINGERS_OPEN_STATE", &fixture.root)
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    assert!(fixture.root.join(format!("browsers/{first}.sock")).exists());
    let _reader = fixture.relay("abcdef0123456789abcdef0123456789");
    let result = fixture
        .request("https://example.com")
        .wait_with_output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("More than one"));
}

fn remote(executable: &str, socket: &str, expression: &str) -> Value {
    let output = Command::new(executable)
        .args(["--server", socket, "--remote-expr", expression])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn native_rpc_preserves_unsaved_buffers_and_does_not_execute_filename_text() {
    let executable = std::env::var("NVIM_BIN").unwrap_or_else(|_| "nvim".into());
    if Command::new(&executable).arg("--version").output().is_err() {
        panic!("Install Neovim or set NVIM_BIN to run the native RPC integration test");
    }
    let mut fixture = Fixture::new();
    let filename = fixture.root.join("file's name | let g:hacked=1.lua");
    fs::write(&filename, "first\nsecond line\nthird\n").unwrap();
    let unsaved = fixture.root.join("unsaved.txt");
    let module = fixture.root.join("module.lua");
    fs::write(&module, include_str!("../../herdr_fingers.lua")).unwrap();
    let init = fixture.root.join("init.lua");
    fs::write(&init, format!("package.loaded.herdr_fingers = dofile({})\nrequire('herdr_fingers').setup()\nvim.api.nvim_buf_set_name(0, {})\nvim.api.nvim_buf_set_lines(0, 0, -1, false, {{'keep this unsaved text'}})\nvim.o.hidden=false\n", json!(module), json!(unsaved))).unwrap();
    let socket = fixture
        .root
        .join("nvim.sock")
        .to_string_lossy()
        .into_owned();
    fixture.children.push(
        Command::new(&executable)
            .args(["--headless", "-n", "-u"])
            .arg(init)
            .args(["-i", "NONE", "--listen", &socket])
            .env("HERDR_FINGERS_OPEN_STATE", fixture.root.join("state"))
            .env("HERDR_SOCKET_PATH", fixture.root.join("herdr.sock"))
            .env("HERDR_PANE_ID", "w1:p2")
            .env("XDG_STATE_HOME", fixture.root.join("nvim-state"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let editor = json!({"nvim_bin":executable,"socket":socket});
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if nvim_call(&editor, "status", Value::Null).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "Neovim did not start");
        std::thread::sleep(Duration::from_millis(25));
    }
    nvim_call(
        &editor,
        "open",
        json!({"path":filename,"line":2,"column":4}),
    )
    .unwrap();
    let lua = format!(
        "vim.json.encode({{file=vim.api.nvim_buf_get_name(0),cursor=vim.api.nvim_win_get_cursor(0),modified=vim.fn.getbufvar(vim.fn.bufnr({}), '&modified'),text=vim.fn.getbufline({},1),hacked=vim.g.hacked or false,hidden=vim.o.hidden}})",
        json!(unsaved),
        json!(unsaved)
    );
    let expression = format!("luaeval('{}')", lua.replace('\'', "''"));
    let result = remote(&executable, &socket, &expression);
    assert_eq!(result["file"], json!(filename));
    assert_eq!(result["cursor"], json!([2, 3]));
    assert_eq!(result["modified"], 1);
    assert_eq!(result["text"], json!(["keep this unsaved text"]));
    assert_eq!(result["hacked"], false);
    assert_eq!(result["hidden"], false);
}
