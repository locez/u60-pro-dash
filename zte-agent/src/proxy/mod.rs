//! Authenticated proxy service with private configuration and a replaceable
//! engine adapter. No engine or network is started by constructing the state.
mod configuration;
mod validation;
use validation::run_checked;
mod engine;
mod services;
pub use engine::MihomoSettings as Settings;
pub use services::ProxyServices;
mod network;
mod supervisor;
pub use supervisor::cli as watchdog_cli;

use crate::handlers::AppState;
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
pub(crate) const BINARY_LIMIT: u64 = 96 * 1024 * 1024;
struct Runtime {
    owner: Option<std::os::unix::net::UnixStream>,
    child: Option<Child>,
    secret: String,
    lock: Option<File>,
    capturing: bool,
    error: Option<String>,
}
pub struct MihomoService {
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
    download_as(url, limit, "u60-pro-dash")
}
fn download_as(url: &str, limit: u64, user_agent: &str) -> Result<Vec<u8>, String> {
    let url = https_url(url)?;
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .user_agent(user_agent)
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(Duration::from_secs(90))
        .https_only(true)
        .build()
        .map_err(|_| "Cannot create download client")?;
    let mut body = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(download_error)?
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
fn download_error(error: reqwest::Error) -> String {
    if let Some(status) = error.status() {
        return format!("Download returned HTTP {status}");
    }
    if error.is_timeout() {
        return "Download timed out".into();
    }
    if error.is_redirect() {
        return "Download was redirected too many times or to an unsupported address".into();
    }
    // Preserve the useful network cause, never the subscription URL/query.
    let mut cause: &dyn std::error::Error = &error;
    let mut details = String::new();
    while let Some(next) = cause.source() {
        details.push_str(&next.to_string().to_ascii_lowercase());
        cause = next;
    }
    if details.contains("certificate") || details.contains("tls") {
        "Download failed TLS verification; check the server certificate and router clock".into()
    } else if details.contains("dns")
        || details.contains("resolve")
        || details.contains("name or service")
    {
        "Download failed because DNS resolution failed".into()
    } else if error.is_connect() {
        "Download could not connect to the server".into()
    } else {
        "Download failed before a complete response was received".into()
    }
}
pub(crate) fn elf(bytes: &[u8]) -> bool {
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
impl MihomoService {
    pub fn new() -> Self {
        Self::at(PathBuf::from("/data/u60-proxy"))
    }
    pub(super) fn at(root: PathBuf) -> Self {
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
        json!({"id":"mihomo","name":"mihomo","adapter":"mihomo","installed":self.root.join("engine").is_file(),"configured":self.root.join("source.json").is_file(),"config_revision":self.configuration_revision().ok().flatten(),"enabled":s.enabled,"running":r.child.is_some(),"capture":s.capture,"capturing":r.capturing,"mode":s.mode,"local_endpoint":format!("127.0.0.1:{}",s.mixed_port),"mixed_port":s.mixed_port,"ipv6":s.ipv6,"tcp_concurrent":s.tcp_concurrent,"last_error":r.error,"capabilities":{"configuration":true,"reload":true,"groups":true,"hotspot":true,"install":true},"version":fs::read_to_string(self.root.join("version")).ok()})
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
            .timeout(Duration::from_secs(if path == "/configs?force=true" {
                60
            } else {
                8
            }))
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
        engine::check_listeners(&settings)?;
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
            network::locked(&self.root, || {
                network::preflight()?;
                private_write(&self.root.join("capture-owned"), b"1", 0o600)?;
                network::prepare()
            })?;
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
            network::locked(&self.root, || {
                if !self.root.join("capture-owned").exists() {
                    return Err("Proxy engine stopped during startup".into());
                }
                network::activate()
            })
            .map(|()| self.runtime.lock().unwrap().capturing = true)
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
    pub(crate) fn install_package(
        self: &Arc<Self>,
        package: &Path,
        version: &str,
    ) -> Result<(), String> {
        let _op = self
            .operation
            .try_lock()
            .map_err(|_| "Another proxy operation is running")?;
        self.prepare()?;
        self.lease()?;
        let result = (|| {
            let mut bytes = Vec::new();
            File::open(package)
                .map_err(|_| "Cannot open engine package")?
                .take(BINARY_LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Cannot read engine package")?;
            if bytes.len() as u64 > BINARY_LIMIT {
                return Err("Engine package exceeds size limit".into());
            }
            self.install_bytes(&bytes, version, true)
        })();
        let mut r = self.runtime.lock().unwrap();
        if r.child.is_none() {
            r.lock = None;
        }
        result
    }
    fn install_bytes(
        self: &Arc<Self>,
        bytes: &[u8],
        version: &str,
        restart: bool,
    ) -> Result<(), String> {
        let was_running = self.runtime.lock().unwrap().child.is_some();
        if was_running && !restart {
            return Err("Stop the service before updating its engine".into());
        }
        let binary = if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(bytes)
                .take(BINARY_LIMIT + 1)
                .read_to_end(&mut out)
                .map_err(|_| "Invalid compressed engine")?;
            out
        } else {
            bytes.to_vec()
        };
        if binary.len() as u64 > BINARY_LIMIT || !elf(&binary) {
            return Err("Engine must be a static Linux ARM64 executable".into());
        }
        let staged = self.root.join("engine.new");
        private_write(&staged, &binary, 0o700)?;
        let validation = self.root.join("install-check.json");
        let result = (|| {
            let settings = self.settings.lock().unwrap().clone();
            let driver = engine::adapter(&settings.adapter)?;
            let config = if self.root.join("source.json").exists() {
                driver.runtime(&self.source()?, &settings, "installation-check")
            } else {
                json!({"mode":"direct","mixed-port":0,"proxies":[],"rules":["MATCH,DIRECT"]})
            };
            private_write(&validation, &serde_json::to_vec(&config).unwrap(), 0o600)?;
            run_checked(driver.as_ref(), &staged, &self.root, &validation)?;
            // Keep the inter-process lease through stop, replacement and rollback.
            let lease = self
                .runtime
                .lock()
                .unwrap()
                .lock
                .as_ref()
                .ok_or("Service lease missing")?
                .try_clone()
                .map_err(|_| "Cannot retain service lease")?;
            let retain = || -> Result<(), String> {
                self.runtime.lock().unwrap().lock = Some(
                    lease
                        .try_clone()
                        .map_err(|_| "Cannot retain service lease")?,
                );
                Ok(())
            };
            let current = self.root.join("engine");
            let backup = self.root.join("engine.prev");
            let had_binary = current.is_file();
            let old_version = fs::read(self.root.join("version")).ok();
            if was_running {
                self.stop();
                retain()?;
            }
            if had_binary && fs::rename(&current, &backup).is_err() {
                if was_running {
                    let _ = self.start();
                }
                return Err("Cannot back up installed engine".into());
            }
            let activation = (|| {
                fs::rename(&staged, &current).map_err(|_| "Cannot activate installed engine")?;
                private_write(&self.root.join("version"), version.as_bytes(), 0o600)?;
                if was_running {
                    self.start()?;
                }
                Ok::<(), String>(())
            })();
            if let Err(error) = activation {
                self.stop();
                retain()?;
                if had_binary {
                    fs::rename(&backup, &current)
                        .map_err(|_| "Engine activation failed and backup could not be restored")?;
                } else {
                    let _ = fs::remove_file(&current);
                }
                if let Some(version) = old_version {
                    private_write(&self.root.join("version"), &version, 0o600)?;
                } else {
                    let _ = fs::remove_file(self.root.join("version"));
                }
                if was_running && self.start().is_err() {
                    return Err(
                        "Update failed; the previous engine was restored but could not restart"
                            .into(),
                    );
                }
                return Err(format!(
                    "Update failed; previous installation restored: {error}"
                ));
            }
            let _ = fs::remove_file(backup);
            Ok(())
        })();
        let _ = fs::remove_file(validation);
        let _ = fs::remove_file(staged);
        result
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
        if matches!(action, "config" | "subscription" | "reload") {
            r.error = result.as_ref().err().cloned();
        }
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
                for (key, target) in [("mode", &mut s.mode), ("capture", &mut s.capture)] {
                    if let Some(v) = body.get(key) {
                        *target = v.as_str().ok_or("Invalid proxy setting")?.into();
                    }
                }
                if let Some(id) = body.get("adapter") {
                    if !matches!(id.as_str(), Some("mihomo" | "compatible-v1")) {
                        return Err("Manage each proxy service separately; an engine cannot be switched here".into());
                    }
                }
                if let Some(v) = body.get("mixed_port") {
                    s.mixed_port = v
                        .as_u64()
                        .and_then(|n| u16::try_from(n).ok())
                        .ok_or("Invalid mixed port")?;
                }
                if let Some(v) = body.get("ipv6") {
                    s.ipv6 = v.as_bool().ok_or("Invalid IPv6 setting")?;
                }
                if let Some(v) = body.get("tcp_concurrent") {
                    s.tcp_concurrent = v.as_bool().ok_or("Invalid TCP setting")?;
                }
                s.adapter = "mihomo".into();
                s.validate()?;
                self.save(&s)?;
                *self.settings.lock().unwrap() = s;
            }
            "config" | "subscription" | "validate" | "reload" => {
                let text = if action == "subscription" {
                    String::from_utf8(download_as(
                        body["url"].as_str().ok_or("HTTPS URL required")?,
                        CONFIG_LIMIT as u64,
                        "mihomo",
                    )?)
                    .map_err(|_| "Configuration must be UTF-8")?
                } else if action == "reload" {
                    if self.runtime.lock().unwrap().child.is_none() {
                        return Err(
                            "Start this service before reloading its saved configuration".into(),
                        );
                    }
                    serde_json::to_string(&self.source()?)
                        .map_err(|_| "Cannot read saved configuration")?
                } else {
                    body["config"]
                        .as_str()
                        .ok_or("Configuration required")?
                        .into()
                };
                self.apply_configuration(&text, body.get("revision"), action == "validate")?;
                if action == "validate" {
                    return Ok(json!({"valid":true,"revision":self.configuration_revision()?}));
                }
                self.runtime.lock().unwrap().error = None;
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
                self.install_bytes(&bytes, version, false)?;
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
    (200, json!({"ok":true,"data":state.proxy.legacy().status()}))
}
pub fn groups(state: &AppState) -> (u16, Value) {
    let m = state.proxy.legacy();
    let result = engine::adapter(&m.settings.lock().unwrap().adapter).and_then(|d| {
        m.controller(reqwest::Method::GET, d.groups_path(), None)
            .map(|v| d.groups(&v))
    });
    reply(result)
}
pub fn service_route(
    state: &AppState,
    method: &crate::server::Method,
    path: &str,
    body: &[u8],
) -> (u16, Value) {
    use crate::server::Method;
    let Some((id, action)) = path
        .strip_prefix("/api/proxy/")
        .and_then(|p| p.split_once('/'))
    else {
        return (404, json!({"ok":false,"error":"Unknown proxy service"}));
    };
    let Some(service) = state.proxy.get(id) else {
        return (404, json!({"ok":false,"error":"Unknown proxy service"}));
    };
    match (method, action) {
        (Method::Get, "status") => reply(Ok(service.status())),
        (Method::Get, "groups") => reply(service.groups()),
        (Method::Get, "config") => {
            if !state.auth.has_password() {
                return (403, json!({"ok":false,"error":"Agent password required"}));
            }
            reply(service.config())
        }
        (Method::Put, "settings")
        | (
            Method::Post,
            "enable" | "disable" | "restart" | "config" | "subscription" | "validate" | "reload"
            | "install" | "select",
        ) => {
            if !state.auth.has_password() {
                return (
                    403,
                    json!({"ok":false,"error":"Set an agent password before managing a proxy"}),
                );
            }
            match serde_json::from_slice::<Value>(body) {
                Ok(v) if v.is_object() => reply(service.change(action, &v)),
                _ => (400, json!({"ok":false,"error":"JSON object required"})),
            }
        }
        _ => (404, json!({"ok":false,"error":"Unknown proxy operation"})),
    }
}
pub fn services(state: &AppState) -> (u16, Value) {
    reply(Ok(state.proxy.status()))
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
    reply(state.proxy.legacy().change(action, &parsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_package_preserves_installation_configuration_and_releases_lease() {
        let root =
            std::env::temp_dir().join(format!("u60-package-rejection-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let service = Arc::new(MihomoService::at(root.clone()));
        service.prepare().unwrap();
        for (name, bytes) in [
            ("engine", "previous executable"),
            ("version", "1.2.3"),
            (
                "source.json",
                "{\"proxies\":[],\"rules\":[\"MATCH,DIRECT\"]}",
            ),
        ] {
            private_write(&root.join(name), bytes.as_bytes(), 0o600).unwrap();
        }
        // Rejection at architecture validation and at the executable smoke test
        // must both happen before touching the previous installation.
        let mut malformed = vec![0u8; 120];
        malformed[..4].copy_from_slice(b"\x7fELF");
        malformed[4] = 2;
        malformed[5] = 1;
        malformed[18..20].copy_from_slice(&183u16.to_le_bytes());
        malformed[32..40].copy_from_slice(&64u64.to_le_bytes());
        malformed[54..56].copy_from_slice(&56u16.to_le_bytes());
        malformed[56..58].copy_from_slice(&1u16.to_le_bytes());
        for bytes in [b"not an executable".as_slice(), &malformed] {
            private_write(&root.join("package"), bytes, 0o600).unwrap();
            assert!(service
                .install_package(&root.join("package"), "1.2.4")
                .is_err());
            assert_eq!(
                fs::read(root.join("engine")).unwrap(),
                b"previous executable"
            );
            assert_eq!(fs::read(root.join("version")).unwrap(), b"1.2.3");
            assert_eq!(service.source().unwrap()["rules"][0], "MATCH,DIRECT");
            assert!(!root.join("engine.new").exists());
            assert!(!root.join("install-check.json").exists());
            assert!(!root.join("engine.prev").exists());
            assert!(service.runtime.lock().unwrap().lock.is_none());
            let next_owner = MihomoService::at(root.clone());
            next_owner.lease().unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }

    /// Optional device/engine contract check. The executable may be a wrapper
    /// that forwards the fixed validation arguments to a connected test device.
    #[test]
    #[ignore = "requires an external engine via U60_PROXY_TEST_ENGINE"]
    fn external_engine_accepts_service_runtime() {
        let binary = PathBuf::from(std::env::var("U60_PROXY_TEST_ENGINE").expect("test engine"));
        let root = std::env::temp_dir().join(format!("u60-proxy-contract-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let driver = engine::adapter("compatible-v1").unwrap();
        let source = driver
            .normalize(
                r#"
proxies: [{name: Example, type: anytls, server: 192.0.2.2, port: 443, password: example-only}]
url-test: &common
  type: url-test
  include-all-proxies: true
  url: https://example.com/generate_204
  interval: 300
proxy-groups:
  - {name: Primary, type: select, proxies: [Auto, Example, DIRECT]}
  - name: Auto
    <<: *common
rules: ['MATCH,DIRECT']
dns: {nameserver: ['https://dns.google/dns-query'], enhanced-mode: fake-ip}
sniffer: {enable: true, sniff: {TLS: {ports: [443]}}}
"#,
            )
            .unwrap();
        for capture in ["local", "hotspot"] {
            let settings = Settings {
                capture: capture.into(),
                ..Settings::default()
            };
            let path = root.join(format!("{capture}.json"));
            private_write(
                &path,
                &serde_json::to_vec(&driver.runtime(
                    &source,
                    &settings,
                    "example-validation-credential",
                ))
                .unwrap(),
                0o600,
            )
            .unwrap();
            run_checked(driver.as_ref(), &binary, &root, &path).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }
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
        let m = MihomoService::at(PathBuf::from("/nonexistent/u60-proxy-test"));
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
