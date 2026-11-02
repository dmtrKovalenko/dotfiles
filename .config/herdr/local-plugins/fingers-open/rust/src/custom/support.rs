use super::Result;
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub fn home() -> Result<PathBuf> {
    Ok(std::env::var_os("HOME").ok_or("HOME is not set")?.into())
}
pub fn config_dir() -> Result<PathBuf> {
    Ok(std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".config")))
}
pub fn data_dir() -> Result<PathBuf> {
    Ok(std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".local/share")))
}
pub fn state_dir() -> Result<PathBuf> {
    let path = std::env::var_os("HERDR_FINGERS_OPEN_STATE")
        .map(PathBuf::from)
        .unwrap_or(
            std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or(home()?.join(".cache"))
                .join("herdr-fingers-open"),
        );
    private_dir(&path)?;
    Ok(path)
}
pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn identifier() -> Result<String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
pub fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let parent = path.parent().ok_or("Missing parent directory")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".fingers-{}", identifier()?));
    let guard = RemoveFile(temporary.clone());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    drop(guard);
    Ok(())
}
pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    atomic_write(path, &bytes, 0o600)
}
pub fn emit(writer: &mut impl Write, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
pub fn read_line(reader: &mut impl BufRead, limit: usize) -> Result<Option<String>> {
    let mut line = Vec::new();
    reader.take(limit as u64 + 1).read_until(b'\n', &mut line)?;
    if line.is_empty() {
        return Ok(None);
    }
    if line.len() > limit || !line.ends_with(b"\n") {
        return Err("Message exceeds its limit or is incomplete".into());
    }
    Ok(Some(String::from_utf8(line)?))
}
pub fn ready(fds: &[RawFd], timeout: Duration) -> Result<Vec<bool>> {
    let mut polls: Vec<libc::pollfd> = fds
        .iter()
        .map(|fd| libc::pollfd {
            fd: *fd,
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    // The poll array is initialized and remains alive for this synchronous call.
    let result = unsafe {
        libc::poll(
            polls.as_mut_ptr(),
            polls.len() as libc::nfds_t,
            timeout.as_millis().min(i32::MAX as u128) as i32,
        )
    };
    if result < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            return Ok(vec![false; fds.len()]);
        }
        return Err(error.into());
    }
    Ok(polls.iter().map(|p| p.revents != 0).collect())
}
pub struct RemoveFile(pub PathBuf);
impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub struct Lock(File);
impl Lock {
    pub fn acquire(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        // flock operates on this owned descriptor; closing it releases the lock.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.sync_all();
    }
}

fn collect_output(mut input: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input
        .by_ref()
        .take(32 * 1024 * 1024)
        .read_to_end(&mut bytes)?;
    std::io::copy(&mut input, &mut std::io::sink())?;
    Ok(bytes)
}
pub fn output(command: &mut Command, timeout: Duration) -> Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().ok_or("Missing stdout")?;
    let stderr = child.stderr.take().ok_or("Missing stderr")?;
    let out = thread::spawn(move || collect_output(stdout));
    let err = thread::spawn(move || collect_output(stderr));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Command timed out".into());
        }
        thread::sleep(Duration::from_millis(10));
    };
    Ok(Output {
        status,
        stdout: out.join().map_err(|_| "stdout reader failed")??,
        stderr: err.join().map_err(|_| "stderr reader failed")??,
    })
}
pub fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value[name]
        .as_str()
        .ok_or_else(|| format!("Missing field: {name}").into())
}
