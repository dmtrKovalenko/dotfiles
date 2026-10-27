//! The Herdr socket API: newline-delimited JSON over a Unix socket, one
//! connection per request.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use thiserror::Error;

use crate::domain::geometry::{Layout, PanePlacement, Rect};
use crate::usecases::ports::{PaneHost, PortError};

#[derive(Debug, Error)]
pub enum HerdrError {
    #[error("HERDR_SOCKET_PATH is not set; run this through a Herdr plugin action")]
    NoSocket,
    #[error("cannot reach Herdr at {path}: {source}")]
    Connect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("I/O error talking to Herdr: {0}")]
    Io(#[from] std::io::Error),
    #[error("Herdr returned invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Herdr refused {method}: {code}: {message}")]
    Api {
        method: String,
        code: String,
        message: String,
    },
    #[error("unexpected Herdr response to {method}: {detail}")]
    Protocol { method: String, detail: String },
}

/// What Herdr tells a plugin process about the situation it was launched in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginContext {
    pub plugin_id: String,
    pub focused_pane_id: Option<String>,
    pub config_dir: Option<PathBuf>,
    pub state_dir: Option<PathBuf>,
}

pub const DEFAULT_PLUGIN_ID: &str = "nathan-poncet.herdr-fingers";

impl PluginContext {
    pub fn from_env() -> Self {
        let context_json: Option<Value> = std::env::var("HERDR_PLUGIN_CONTEXT_JSON")
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok());
        let focused_pane_id = std::env::var("HERDR_PANE_ID").ok().or_else(|| {
            context_json
                .as_ref()
                .and_then(|c| c.get("focused_pane_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        });
        PluginContext {
            plugin_id: std::env::var("HERDR_PLUGIN_ID")
                .unwrap_or_else(|_| DEFAULT_PLUGIN_ID.to_string()),
            focused_pane_id,
            config_dir: std::env::var_os("HERDR_PLUGIN_CONFIG_DIR").map(PathBuf::from),
            state_dir: std::env::var_os("HERDR_PLUGIN_STATE_DIR").map(PathBuf::from),
        }
    }
}

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// A client for one Herdr session.
#[derive(Debug, Clone)]
pub struct HerdrClient {
    socket: PathBuf,
}

impl HerdrClient {
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        HerdrClient {
            socket: socket.into(),
        }
    }

    /// The socket Herdr handed us, or the default session's socket.
    pub fn from_env() -> Result<Self, HerdrError> {
        if let Some(path) = std::env::var_os("HERDR_SOCKET_PATH") {
            return Ok(HerdrClient::new(path));
        }
        let home = std::env::var_os("HOME").ok_or(HerdrError::NoSocket)?;
        let default = Path::new(&home).join(".config/herdr/herdr.sock");
        if default.exists() {
            Ok(HerdrClient::new(default))
        } else {
            Err(HerdrError::NoSocket)
        }
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value, HerdrError> {
        let id = format!(
            "herdr-fingers-{}",
            NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
        );
        let stream = UnixStream::connect(&self.socket).map_err(|source| HerdrError::Connect {
            path: self.socket.clone(),
            source,
        })?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
        let mut reader = BufReader::new(stream);
        let request = json!({ "id": id, "method": method, "params": params }).to_string();
        {
            let stream = reader.get_mut();
            stream.write_all(request.as_bytes())?;
            stream.write_all(b"\n")?;
            stream.flush()?;
        }
        let mut line = String::new();
        if reader.take(4 * 1024 * 1024 + 1).read_line(&mut line)? == 0 {
            return Err(HerdrError::Protocol {
                method: method.to_string(),
                detail: "connection closed before any answer".to_string(),
            });
        }
        if line.len() > 4 * 1024 * 1024 || !line.ends_with('\n') {
            return Err(HerdrError::Protocol {
                method: method.to_string(),
                detail: "response exceeds its limit or is incomplete".to_string(),
            });
        }
        let envelope: Value = serde_json::from_str(&line)?;
        if let Some(error) = envelope.get("error") {
            return Err(HerdrError::Api {
                method: method.to_string(),
                code: error["code"].as_str().unwrap_or("unknown").to_string(),
                message: error["message"].as_str().unwrap_or("").to_string(),
            });
        }
        if envelope["id"].as_str() != Some(id.as_str()) {
            return Err(HerdrError::Protocol {
                method: method.to_string(),
                detail: "response id does not match the request".to_string(),
            });
        }
        envelope
            .get("result")
            .cloned()
            .ok_or_else(|| HerdrError::Protocol {
                method: method.to_string(),
                detail: "neither result nor error".to_string(),
            })
    }

    /// The tab layout around `pane_id`.
    pub fn layout(&self, pane_id: &str) -> Result<Layout, HerdrError> {
        let result = self.call("pane.layout", json!({ "pane_id": pane_id }))?;
        parse_layout(&result["layout"]).ok_or_else(|| HerdrError::Protocol {
            method: "pane.layout".to_string(),
            detail: "layout is missing area, zoomed or panes".to_string(),
        })
    }

    /// The pane's visible screen with its ANSI styling.
    pub fn read_visible(&self, pane_id: &str) -> Result<String, HerdrError> {
        let result = self.call(
            "pane.read",
            json!({
                "pane_id": pane_id,
                "source": "visible",
                "format": "ansi",
                "strip_ansi": false
            }),
        )?;
        result
            .pointer("/read/text")
            .or_else(|| result.get("text"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| HerdrError::Protocol {
                method: "pane.read".to_string(),
                detail: "no text in the response".to_string(),
            })
    }

    fn pane_info(&self, pane_id: &str) -> Result<Value, HerdrError> {
        let result = self.call("pane.get", json!({ "pane_id": pane_id }))?;
        Ok(result.get("pane").cloned().unwrap_or(result))
    }

    /// The label Herdr shows for the pane (a plugin pane carries its title).
    pub fn pane_label(&self, pane_id: &str) -> Result<Option<String>, HerdrError> {
        let info = self.pane_info(pane_id)?;
        Ok(info["label"].as_str().map(str::to_string))
    }

    /// The directory the pane's foreground process runs in.
    pub fn pane_cwd(&self, pane_id: &str) -> Result<Option<PathBuf>, HerdrError> {
        let info = self.pane_info(pane_id)?;
        Ok(info["foreground_cwd"]
            .as_str()
            .or_else(|| info["cwd"].as_str())
            .map(PathBuf::from))
    }

    /// Opens one of this plugin's declared panes; returns the new pane id.
    pub fn open_plugin_pane(
        &self,
        plugin_id: &str,
        entrypoint: &str,
        env: BTreeMap<String, String>,
    ) -> Result<String, HerdrError> {
        let result = self.call(
            "plugin.pane.open",
            json!({
                "plugin_id": plugin_id,
                "entrypoint": entrypoint,
                "focus": true,
                "env": env
            }),
        )?;
        result
            .pointer("/plugin_pane/pane/pane_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| HerdrError::Protocol {
                method: "plugin.pane.open".to_string(),
                detail: "no pane id in the response".to_string(),
            })
    }

    pub fn send_text(&self, pane_id: &str, text: &str) -> Result<(), HerdrError> {
        self.call(
            "pane.send_text",
            json!({ "pane_id": pane_id, "text": text }),
        )?;
        Ok(())
    }

    pub fn notify(&self, title: &str, body: &str) -> Result<(), HerdrError> {
        self.call(
            "notification.show",
            json!({ "title": title, "body": body, "sound": "none" }),
        )?;
        Ok(())
    }
}

impl From<HerdrError> for PortError {
    fn from(error: HerdrError) -> Self {
        PortError::new(error)
    }
}

impl PaneHost for HerdrClient {
    fn layout(&self, pane_id: &str) -> Result<Layout, PortError> {
        Ok(HerdrClient::layout(self, pane_id)?)
    }

    fn read_visible(&self, pane_id: &str) -> Result<String, PortError> {
        Ok(HerdrClient::read_visible(self, pane_id)?)
    }

    fn pane_label(&self, pane_id: &str) -> Result<Option<String>, PortError> {
        Ok(HerdrClient::pane_label(self, pane_id)?)
    }

    fn pane_cwd(&self, pane_id: &str) -> Result<Option<PathBuf>, PortError> {
        Ok(HerdrClient::pane_cwd(self, pane_id)?)
    }

    fn open_overlay(
        &self,
        plugin_id: &str,
        entrypoint: &str,
        env: BTreeMap<String, String>,
    ) -> Result<String, PortError> {
        Ok(self.open_plugin_pane(plugin_id, entrypoint, env)?)
    }

    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), PortError> {
        Ok(HerdrClient::send_text(self, pane_id, text)?)
    }

    fn notify(&self, title: &str, body: &str) -> Result<(), PortError> {
        Ok(HerdrClient::notify(self, title, body)?)
    }
}

fn parse_rect(value: &Value) -> Option<Rect> {
    let field = |name: &str| value[name].as_u64().and_then(|n| u16::try_from(n).ok());
    Some(Rect::new(
        field("x")?,
        field("y")?,
        field("width")?,
        field("height")?,
    ))
}

/// Reads a `pane.layout` result into the kernel's [`Layout`].
pub fn parse_layout(value: &Value) -> Option<Layout> {
    let area = parse_rect(&value["area"])?;
    let zoomed = value["zoomed"].as_bool()?;
    let panes = value["panes"]
        .as_array()?
        .iter()
        .filter_map(|pane| {
            Some(PanePlacement {
                pane_id: pane["pane_id"].as_str()?.to_string(),
                rect: parse_rect(&pane["rect"])?,
            })
        })
        .collect();
    Some(Layout {
        area,
        zoomed,
        panes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    /// A one-shot fake Herdr: answers `responses` in order, records requests.
    fn fake_herdr(responses: Vec<Value>) -> (PathBuf, std::thread::JoinHandle<Vec<Value>>) {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("herdr-fingers-{unique}.sock"));
        let listener = UnixListener::bind(&path).unwrap();
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                let id = request["id"].clone();
                requests.push(request);
                let envelope = if response.get("error").is_some() {
                    json!({ "id": id, "error": response["error"] })
                } else {
                    json!({ "id": id, "result": response })
                };
                writeln!(reader.get_mut(), "{envelope}").unwrap();
            }
            requests
        });
        (path, handle)
    }

    #[test]
    fn layout_is_read_from_the_pane_layout_response() {
        let (path, server) = fake_herdr(vec![json!({
            "type": "pane_layout",
            "layout": {
                "area": {"x": 0, "y": 0, "width": 100, "height": 40},
                "zoomed": false,
                "focused_pane_id": "w1:p2",
                "panes": [
                    {"pane_id": "w1:p1", "focused": false, "rect": {"x": 0, "y": 0, "width": 50, "height": 40}},
                    {"pane_id": "w1:p2", "focused": true, "rect": {"x": 51, "y": 0, "width": 49, "height": 40}}
                ],
                "splits": []
            }
        })]);
        let layout = HerdrClient::new(&path).layout("w1:p2").unwrap();
        let requests = server.join().unwrap();
        assert_eq!(requests[0]["method"], "pane.layout");
        assert_eq!(requests[0]["params"]["pane_id"], "w1:p2");
        assert_eq!(layout.area, Rect::new(0, 0, 100, 40));
        assert_eq!(layout.panes[1].rect, Rect::new(51, 0, 49, 40));
        assert!(!layout.zoomed);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_visible_screen_is_requested_with_ansi_styling() {
        let (path, server) = fake_herdr(vec![json!({
            "type": "pane_read",
            "read": {"pane_id": "w1:p1", "text": "\u{1b}[31mhello\u{1b}[0m\n", "truncated": false}
        })]);
        let text = HerdrClient::new(&path).read_visible("w1:p1").unwrap();
        let requests = server.join().unwrap();
        assert_eq!(text, "\u{1b}[31mhello\u{1b}[0m\n");
        assert_eq!(requests[0]["method"], "pane.read");
        assert_eq!(requests[0]["params"]["source"], "visible");
        assert_eq!(requests[0]["params"]["format"], "ansi");
        assert_eq!(requests[0]["params"]["strip_ansi"], false);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn opening_the_overlay_passes_the_environment_and_returns_the_pane_id() {
        let (path, server) = fake_herdr(vec![json!({
            "type": "plugin_pane_opened",
            "plugin_pane": {"plugin_id": "x", "entrypoint": "overlay", "pane": {"pane_id": "w1:p9"}}
        })]);
        let mut env = BTreeMap::new();
        env.insert("HERDR_FINGERS_GEOMETRY".to_string(), "{}".to_string());
        let pane = HerdrClient::new(&path)
            .open_plugin_pane("nathan-poncet.herdr-fingers", "overlay", env)
            .unwrap();
        let requests = server.join().unwrap();
        assert_eq!(pane, "w1:p9");
        assert_eq!(requests[0]["method"], "plugin.pane.open");
        assert_eq!(
            requests[0]["params"]["plugin_id"],
            "nathan-poncet.herdr-fingers"
        );
        assert_eq!(requests[0]["params"]["entrypoint"], "overlay");
        assert_eq!(requests[0]["params"]["focus"], true);
        assert_eq!(requests[0]["params"]["env"]["HERDR_FINGERS_GEOMETRY"], "{}");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn api_errors_carry_the_code_and_message() {
        let (path, server) = fake_herdr(vec![json!({
            "error": {"code": "pane_not_found", "message": "pane w1:p7 not found"}
        })]);
        let error = HerdrClient::new(&path).send_text("w1:p7", "x").unwrap_err();
        server.join().unwrap();
        assert!(matches!(
            error,
            HerdrError::Api { code, message, .. } if code == "pane_not_found" && message.contains("w1:p7")
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_missing_socket_is_a_clear_error() {
        let error = HerdrClient::new("/nonexistent/herdr.sock")
            .layout("w1:p1")
            .unwrap_err();
        assert!(matches!(error, HerdrError::Connect { .. }));
    }
}
