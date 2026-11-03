//! Authenticated proxy service with private configuration and a replaceable
//! engine adapter. No engine or network is started by constructing the state.
mod engine;
mod network;
mod supervisor;
pub use supervisor::cli as watchdog_cli;

use crate::handlers::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const CONFIG_LIMIT: usize = 512 * 1024;
const BINARY_LIMIT: u64 = 96 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub adapter: String,
    pub mode: String,
    pub capture: String,
    pub enabled: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            adapter: "compatible-v1".into(),
            mode: "rule".into(),
            capture: "local".into(),
            enabled: false,
        }
    }
}
impl Settings {
    fn validate(&self) -> Result<(), String> {
        engine::adapter(&self.adapter)?;
        if !["rule", "global", "direct"].contains(&self.mode.as_str()) {
            return Err("Choose rule, global or direct mode".into());
        }
        if !["local", "hotspot"].contains(&self.capture.as_str()) {
            return Err("Choose local or hotspot traffic".into());
        }
        Ok(())
    }
}
struct Runtime {
    owner: Option<std::os::unix::net::UnixStream>,
    child: Option<Child>,
    secret: String,
    lock: Option<File>,
    capturing: bool,
    error: Option<String>,
}
pub struct ProxyManager {
    root: PathBuf,
    settings: Mutex<Settings>,
    runtime: Mutex<Runtime>,
    operation: Mutex<()>,
}

pub fn https_url(text: &str) -> Result<reqwest::Url, String> {
    let u = reqwest::Url::parse(text).map_err(|_| "Invalid HTTPS URL")?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || text.len() > 4096
    {
        return Err("Use an HTTPS URL without user information or fragment".into());
    }
    Ok(u)
}
fn download(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let url = https_url(url)?;
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(Duration::from_secs(90))
        .https_only(true)
        .build()
        .map_err(|_| "Cannot create download client")?;
    let mut body = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|_| "Download failed; verify the URL and network")?
        .take(limit + 1);
    let mut bytes = Vec::new();
    body.read_to_end(&mut bytes)
        .map_err(|_| "Download interrupted")?;
    if bytes.len() as u64 > limit {
        return Err("Download exceeds size limit".into());
    }
    Ok(bytes)
}
fn private_write(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    crate::ota::storage::atomic_write(path, bytes, mode)
        .map_err(|_| "Cannot save private proxy file".into())
}
fn run_checked(
    driver: &dyn engine::Engine,
    binary: &Path,
    root: &Path,
    config: &Path,
) -> Result<(), String> {
    let mut child = driver
        .command(binary, root, config, true)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Cannot run installed proxy engine")?;
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                return if s.success() {
                    Ok(())
                } else {
                    Err(
                        "Proxy engine rejected the configuration; check nodes, groups and rules"
                            .into(),
                    )
                }
            }
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Proxy validation timed out".into());
            }
        }
    }
}
fn elf(bytes: &[u8]) -> bool {
    if bytes.len() < 64
        || &bytes[..4] != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || u16::from_le_bytes([bytes[18], bytes[19]]) != 183
    {
        return false;
    }
    let offset = u64::from_le_bytes(bytes[32..40].try_into().unwrap()) as usize;
    let size = u16::from_le_bytes(bytes[54..56].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(bytes[56..58].try_into().unwrap()) as usize;
    size >= 56
        && count > 0
        && count < 256
        && (0..count).all(|i| {
            offset
                .checked_add(i * size)
                .and_then(|p| p.checked_add(4).and_then(|end| bytes.get(p..end)))
                .is_some_and(|b| u32::from_le_bytes(b.try_into().unwrap()) != 3)
        })
}
impl ProxyManager {
    pub fn new() -> Self {
        Self::at(PathBuf::from("/data/u60-proxy"))
    }
    fn at(root: PathBuf) -> Self {
        let settings = fs::read(root.join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .filter(|s| s.validate().is_ok())
            .unwrap_or_default();
        Self {
            root,
            settings: Mutex::new(settings),
            runtime: Mutex::new(Runtime {
                owner: None,
                child: None,
                secret: String::new(),
                lock: None,
                capturing: false,
                error: None,
            }),
            operation: Mutex::new(()),
        }
    }
    fn prepare(&self) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|_| "Cannot create proxy storage")?;
        if fs::symlink_metadata(&self.root)
            .map_err(|_| "Cannot inspect proxy storage")?
            .is_symlink()
        {
            return Err("Proxy storage cannot be a symlink".into());
        }
        fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect proxy storage")?;
        Ok(())
    }
    fn save(&self, s: &Settings) -> Result<(), String> {
        private_write(
            &self.root.join("settings.json"),
            &serde_json::to_vec(s).map_err(|_| "Invalid proxy settings")?,
            0o600,
        )
    }
    fn source(&self) -> Result<Value, String> {
        serde_json::from_slice(
            &fs::read(self.root.join("source.json"))
                .map_err(|_| "Import a proxy configuration first")?,
        )
        .map_err(|_| "Saved proxy configuration is invalid".into())
    }
    pub fn status(&self) -> Value {
        let s = self.settings.lock().unwrap().clone();
        let r = self.runtime.lock().unwrap();
        json!({"adapter":s.adapter,"installed":self.root.join("engine").is_file(),"configured":self.root.join("source.json").is_file(),"enabled":s.enabled,"running":r.child.is_some(),"capture":s.capture,"capturing":r.capturing,"mode":s.mode,"local_endpoint":"127.0.0.1:7890","last_error":r.error,"capabilities":{"configuration":true,"groups":true,"hotspot":true,"install":true},"version":fs::read_to_string(self.root.join("version")).ok()})
    }
    fn controller(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let secret = {
            let r = self.runtime.lock().unwrap();
            if r.child.is_none() {
                return Err("Proxy service is not running".into());
            }
            r.secret.clone()
        };
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|_| "Cannot create local controller client")?;
        let mut request = client
            .request(method, format!("http://127.0.0.1:19092{path}"))
            .bearer_auth(secret);
        if let Some(b) = body {
            request = request.json(&b);
        }
        let response = request
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|_| "Proxy controller request failed")?;
        if response.status() == 204 {
            return Ok(json!({}));
        }
        let mut bytes = Vec::new();
        response
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read controller reply")?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("Controller reply exceeds limit".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "Invalid controller reply".into())
    }
    fn lease(&self) -> Result<(), String> {
        let mut r = self.runtime.lock().unwrap();
        if r.lock.is_some() {
            return Ok(());
        }
        let lock = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(self.root.join("service.lock"))
            .map_err(|_| "Cannot lock proxy service")?;
        // SAFETY: lock is a live descriptor, retained throughout cleanup.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("Proxy service is owned by another agent; wait for cleanup".into());
        }
        supervisor::cleanup(&self.root);
        r.lock = Some(lock);
        Ok(())
    }
    fn stop(&self) {
        let mut r = self.runtime.lock().unwrap();
        drop(r.owner.take());
        if let Some(mut child) = r.child.take() {
            let _ = child.wait();
        }
        if r.lock.is_some() {
            supervisor::cleanup(&self.root);
        }
        r.capturing = false;
        r.secret.clear();
        r.lock = None;
    }
    fn start(self: &Arc<Self>) -> Result<(), String> {
        if self.runtime.lock().unwrap().child.is_some() {
            return Ok(());
        }
        self.prepare()?;
        self.lease()?;
        let result = self.start_inner();
        if result.is_err() {
            self.stop();
        }
        result
    }
    fn start_inner(self: &Arc<Self>) -> Result<(), String> {
        let settings = self.settings.lock().unwrap().clone();
        let source = self.source()?;
        let driver = engine::adapter(&settings.adapter)?;
        let mut random = [0u8; 32];
        getrandom::getrandom(&mut random).map_err(|_| "Cannot create controller credential")?;
        let secret = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let runtime = driver.runtime(&source, &settings, &secret);
        private_write(
            &self.root.join("runtime.json"),
            &serde_json::to_vec(&runtime).unwrap(),
            0o600,
        )?;
        run_checked(
            driver.as_ref(),
            &self.root.join("engine"),
            &self.root,
            &self.root.join("runtime.json"),
        )?;
        if settings.capture == "hotspot" {
            network::preflight()?;
            private_write(&self.root.join("capture-owned"), b"1", 0o600)?;
            network::prepare()?;
        }
        let generation = {
            let mut r = self.runtime.lock().unwrap();
            let (owner, child) =
                supervisor::launch(&self.root, &settings.adapter, r.lock.as_ref().unwrap())?;
            let pid = child.id();
            r.child = Some(child);
            r.owner = Some(owner);
            r.secret = secret;
            r.error = None;
            pid
        };
        let mut ready = false;
        let until = Instant::now() + Duration::from_secs(12);
        while Instant::now() < until {
            if self
                .controller(reqwest::Method::GET, driver.readiness_path(), None)
                .is_ok()
            {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let result = if !ready {
            Err("Proxy controller did not become ready".into())
        } else if settings.capture == "hotspot" {
            network::activate().map(|()| self.runtime.lock().unwrap().capturing = true)
        } else {
            Ok(())
        };
        if let Err(e) = result {
            self.stop();
            return Err(e);
        }
        let weak = Arc::downgrade(self);
        std::thread::Builder::new()
            .name("proxy-child".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(2));
                let Some(m) = weak.upgrade() else { break };
                let Ok(_op) = m.operation.try_lock() else {
                    continue;
                };
                let exited = {
                    let mut r = m.runtime.lock().unwrap();
                    match r.child.as_mut() {
                        Some(c) if c.id() == generation => !matches!(c.try_wait(), Ok(None)),
                        _ => break,
                    }
                };
                if exited {
                    m.stop();
                    m.runtime.lock().unwrap().error =
                        Some("Proxy engine stopped; hotspot routing was restored".into());
                    break;
                }
            })
            .map_err(|_| {
                self.stop();
                "Cannot supervise proxy process".to_string()
            })?;
        Ok(())
    }
    pub fn auto_start(self: &Arc<Self>) {
        if !self.settings.lock().unwrap().enabled && !self.root.join("capture-owned").exists() {
            return;
        }
        let m = Arc::clone(self);
        let _ = std::thread::Builder::new()
            .name("proxy-start".into())
            .spawn(move || {
                let _op = m.operation.lock().unwrap();
                let enabled = m.settings.lock().unwrap().enabled;
                let result = if enabled {
                    m.start()
                } else {
                    m.prepare().and_then(|()| m.lease()).map(|()| m.stop())
                };
                if let Err(e) = result {
                    m.runtime.lock().unwrap().error = Some(e);
                }
            });
    }
    fn change(self: &Arc<Self>, action: &str, body: &Value) -> Result<Value, String> {
        let _op = self
            .operation
            .try_lock()
            .map_err(|_| "Another proxy operation is running")?;
        self.prepare()?;
        self.lease()?;
        let result = self.change_inner(action, body);
        let mut r = self.runtime.lock().unwrap();
        if r.child.is_none() {
            r.lock = None;
        }
        result
    }
    fn change_inner(self: &Arc<Self>, action: &str, body: &Value) -> Result<Value, String> {
        match action {
            "enable" | "disable" | "restart" => {
                let mut s = self.settings.lock().unwrap().clone();
                s.enabled = action != "disable";
                if action != "enable" {
                    self.stop();
                }
                if s.enabled {
                    if let Err(e) = self.start() {
                        self.runtime.lock().unwrap().error = Some(e.clone());
                        return Err(e);
                    }
                }
                if let Err(e) = self.save(&s) {
                    self.stop();
                    return Err(e);
                }
                *self.settings.lock().unwrap() = s;
            }
            "settings" => {
                if self.runtime.lock().unwrap().child.is_some() {
                    return Err("Stop the service before changing its routing settings".into());
                }
                let mut s = self.settings.lock().unwrap().clone();
                for (key, target) in [
                    ("mode", &mut s.mode),
                    ("capture", &mut s.capture),
                    ("adapter", &mut s.adapter),
                ] {
                    if let Some(v) = body.get(key) {
                        *target = v.as_str().ok_or("Invalid proxy setting")?.into();
                    }
                }
                s.validate()?;
                self.save(&s)?;
                *self.settings.lock().unwrap() = s;
            }
            "config" | "subscription" => {
                if self.runtime.lock().unwrap().child.is_some() {
                    return Err("Stop the service before importing a configuration".into());
                }
                let text = if action == "subscription" {
                    String::from_utf8(download(
                        body["url"].as_str().ok_or("HTTPS URL required")?,
                        CONFIG_LIMIT as u64,
                    )?)
                    .map_err(|_| "Configuration must be UTF-8")?
                } else {
                    body["config"]
                        .as_str()
                        .ok_or("Configuration required")?
                        .into()
                };
                let s = self.settings.lock().unwrap().clone();
                let driver = engine::adapter(&s.adapter)?;
                let source = driver.normalize(&text)?;
                if !self.root.join("engine").is_file() {
                    return Err("Install the proxy engine before importing configuration".into());
                }
                let runtime = driver.runtime(&source, &s, "validation-only");
                let candidate = self.root.join("candidate.json");
                private_write(&candidate, &serde_json::to_vec(&runtime).unwrap(), 0o600)?;
                let valid = run_checked(
                    driver.as_ref(),
                    &self.root.join("engine"),
                    &self.root,
                    &candidate,
                );
                let _ = fs::remove_file(candidate);
                valid?;
                private_write(
                    &self.root.join("source.json"),
                    &serde_json::to_vec(&source).unwrap(),
                    0o600,
                )?;
            }
            "install" => {
                if self.runtime.lock().unwrap().child.is_some() {
                    return Err("Stop the service before updating its engine".into());
                }
                let sha = body["sha256"]
                    .as_str()
                    .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                    .ok_or("SHA-256 must contain 64 hexadecimal characters")?;
                let version = body["version"]
                    .as_str()
                    .filter(|s| {
                        !s.is_empty()
                            && s.len() <= 32
                            && s.bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
                    })
                    .ok_or("A version label is required")?;
                let bytes = download(
                    body["url"].as_str().ok_or("Binary HTTPS URL required")?,
                    BINARY_LIMIT,
                )?;
                if format!("{:x}", Sha256::digest(&bytes)) != sha.to_ascii_lowercase() {
                    return Err("Downloaded engine checksum does not match".into());
                }
                let binary = if bytes.starts_with(&[0x1f, 0x8b]) {
                    let mut out = Vec::new();
                    flate2::read::GzDecoder::new(&bytes[..])
                        .take(BINARY_LIMIT + 1)
                        .read_to_end(&mut out)
                        .map_err(|_| "Invalid compressed engine")?;
                    out
                } else {
                    bytes
                };
                if binary.len() as u64 > BINARY_LIMIT || !elf(&binary) {
                    return Err("Engine must be a static Linux ARM64 executable".into());
                }
                let staged = self.root.join("engine.new");
                private_write(&staged, &binary, 0o700)?;
                let validation = self.root.join("install-check.json");
                private_write(&validation,b"{\"mode\":\"direct\",\"mixed-port\":0,\"proxies\":[],\"rules\":[\"MATCH,DIRECT\"]}",0o600)?;
                let result = run_checked(
                    engine::adapter(&self.settings.lock().unwrap().adapter)?.as_ref(),
                    &staged,
                    &self.root,
                    &validation,
                );
                let _ = fs::remove_file(validation);
                if let Err(e) = result {
                    let _ = fs::remove_file(staged);
                    return Err(e);
                }
                fs::rename(staged, self.root.join("engine"))
                    .map_err(|_| "Cannot activate installed engine")?;
                private_write(&self.root.join("version"), version.as_bytes(), 0o600)?;
            }
            "select" => {
                let group = body["group"].as_str().ok_or("Group required")?;
                let choice = body["choice"].as_str().ok_or("Choice required")?;
                let driver = engine::adapter(&self.settings.lock().unwrap().adapter)?;
                let groups = driver.groups(&self.controller(
                    reqwest::Method::GET,
                    driver.groups_path(),
                    None,
                )?);
                if !groups.as_array().unwrap().iter().any(|g| {
                    g["name"] == group
                        && g["selectable"] == true
                        && g["choices"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|v| v == choice))
                }) {
                    return Err("Group or choice changed; refresh before selecting".into());
                }
                let (path, body) = driver.selection(group, choice);
                self.controller(reqwest::Method::PUT, &path, Some(body))?;
            }
            _ => return Err("Unknown proxy operation".into()),
        }
        Ok(self.status())
    }
}
pub fn status(state: &AppState) -> (u16, Value) {
    (200, json!({"ok":true,"data":state.proxy.status()}))
}
pub fn groups(state: &AppState) -> (u16, Value) {
    let m = &state.proxy;
    let result = engine::adapter(&m.settings.lock().unwrap().adapter).and_then(|d| {
        m.controller(reqwest::Method::GET, d.groups_path(), None)
            .map(|v| d.groups(&v))
    });
    reply(result)
}
fn reply(result: Result<Value, String>) -> (u16, Value) {
    match result {
        Ok(v) => (200, json!({"ok":true,"data":v})),
        Err(e) => (400, json!({"ok":false,"error":e})),
    }
}
pub fn change(state: &AppState, action: &str, body: &[u8]) -> (u16, Value) {
    if !state.auth.has_password() {
        return (
            403,
            json!({"ok":false,"error":"Set an agent password before managing the proxy service"}),
        );
    }
    let parsed = match serde_json::from_slice::<Value>(body) {
        Ok(v) if v.is_object() => v,
        _ => return (400, json!({"ok":false,"error":"JSON object required"})),
    };
    reply(state.proxy.change(action, &parsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_urls_and_settings() {
        for u in [
            "http://example.com/x",
            "https://user:secret@example.com/x",
            "https://example.com/#x",
            "file:///etc/passwd",
        ] {
            assert!(https_url(u).is_err());
        }
        assert!(https_url("https://example.com/config?token=example").is_ok());
        let mut s = Settings::default();
        assert!(s.validate().is_ok());
        s.capture = "all-host-traffic".into();
        assert!(s.validate().is_err());
    }
    #[test]
    fn status_never_includes_saved_configuration_or_controller_secret() {
        let m = ProxyManager::at(PathBuf::from("/nonexistent/u60-proxy-test"));
        m.runtime.lock().unwrap().secret = "private-test-credential".into();
        let s = m.status();
        assert_eq!(s["running"], false);
        assert!(!s.to_string().contains("private-test-credential"));
        assert!(s.get("config").is_none());
    }
    #[test]
    fn rejects_dynamic_or_foreign_binaries() {
        assert!(!elf(b"not executable"));
        let mut b = vec![0u8; 120];
        b[..4].copy_from_slice(b"\x7fELF");
        b[4] = 2;
        b[5] = 1;
        b[18..20].copy_from_slice(&183u16.to_le_bytes());
        b[32..40].copy_from_slice(&64u64.to_le_bytes());
        b[54..56].copy_from_slice(&56u16.to_le_bytes());
        b[56..58].copy_from_slice(&1u16.to_le_bytes());
        assert!(elf(&b));
        b[64..68].copy_from_slice(&3u32.to_le_bytes());
        assert!(!elf(&b));
    }
}
