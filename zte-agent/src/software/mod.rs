//! Official release selection and bounded manual package uploads. Runtime
//! configuration and process ownership stay with each individual service.
mod archive;
mod releases;
pub(crate) use archive::extract_tailscale;

use crate::proxy::geodata::{self, Kind as DataKind};
use base64::Engine as _;
use releases::Release;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PACKAGE_MAX: u64 = 120 * 1024 * 1024;
const CHUNK_MAX: usize = 256 * 1024;
const UPLOAD_TTL: Duration = Duration::from_secs(20 * 60);
pub const UPLOAD_BODY_MAX: usize = 384 * 1024;

struct Package(PathBuf);
impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
struct Upload {
    package: Package,
    file: File,
    id: String,
    version: String,
    size: u64,
    received: u64,
    touched: Instant,
}
#[derive(Default)]
struct State {
    available: Option<Release>,
    checked: Option<Instant>,
    checked_at: Option<u64>,
    checking: bool,
    busy: bool,
    phase: &'static str,
    error: Option<String>,
    version: Option<String>,
    sha256: Option<String>,
    received: u64,
    total: Option<u64>,
    package_limit: u64,
    upload: Option<Upload>,
}
impl State {
    fn expire_upload(&mut self) {
        if self
            .upload
            .as_ref()
            .is_some_and(|u| u.touched.elapsed() > UPLOAD_TTL)
        {
            self.upload = None;
            self.busy = false;
            self.phase = "failed";
            self.error = Some("The incomplete upload expired; choose the file again".into());
        }
    }
    fn snapshot(&mut self) -> Value {
        self.expire_upload();
        json!({"available":self.available,"checked_at":self.checked_at,"checking":self.checking,
            "busy":self.busy,"phase":if self.phase.is_empty(){"idle"}else{self.phase},"error":self.error,
            "version":self.version,"sha256":self.sha256,"bytes_received":self.received,"bytes_total":self.total,
            "architecture":"linux-arm64","upload_limit":self.package_limit,"chunk_size":CHUNK_MAX})
    }
}
pub struct Manager {
    slots: BTreeMap<&'static str, Arc<Mutex<State>>>,
    root: PathBuf,
}
impl Default for Manager {
    fn default() -> Self {
        Self::at(PathBuf::from("/data/u60-software"))
    }
}
enum Source {
    Official(Release),
    Uploaded(Upload),
}
enum Target {
    Proxy(Arc<crate::proxy::MihomoService>),
    ProxyData(Arc<crate::proxy::MihomoService>, DataKind),
    Tailscale(Arc<crate::tailscale::TailscaleManager>),
}
impl Target {
    fn install(&self, package: &Path, version: &str) -> Result<(), String> {
        match self {
            Self::Proxy(service) => service.install_package(package, version),
            Self::ProxyData(service, kind) => service.install_geodata(*kind, package),
            Self::Tailscale(service) => {
                crate::tailscale::install_package(service, package, version)
            }
        }
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random_id() -> Result<String, String> {
    let mut id = [0u8; 16];
    getrandom::getrandom(&mut id).map_err(|_| "Cannot allocate upload identifier")?;
    Ok(id.iter().map(|b| format!("{b:02x}")).collect())
}
impl Manager {
    fn at(root: PathBuf) -> Self {
        Self {
            root,
            slots: [
                "mihomo",
                "tailscale",
                "mihomo-geoip",
                "mihomo-geosite",
                "mihomo-mmdb",
                "mihomo-asn",
            ]
            .into_iter()
            .map(|s| {
                (
                    s,
                    Arc::new(Mutex::new(State {
                        package_limit: if DataKind::from_service(s).is_some() {
                            geodata::LIMIT
                        } else if s == "mihomo" {
                            crate::proxy::BINARY_LIMIT
                        } else {
                            PACKAGE_MAX
                        },
                        ..State::default()
                    })),
                )
            })
            .collect(),
        }
    }
    fn slot(&self, service: &str) -> Result<Arc<Mutex<State>>, String> {
        self.slots
            .get(service)
            .cloned()
            .ok_or_else(|| "Unknown software service".into())
    }
    fn new_package(&self) -> Result<(Package, File), String> {
        fs::create_dir_all(&self.root).map_err(|_| "Cannot create package directory")?;
        let meta =
            fs::symlink_metadata(&self.root).map_err(|_| "Cannot inspect package directory")?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("Invalid package directory".into());
        }
        fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect package directory")?;
        // Remove only abandoned files from this staging namespace after a restart.
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("package-")
                    && name.len() == 40
                    && entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age > Duration::from_secs(7200))
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let path = self.root.join(format!("package-{}", random_id()?));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| "Cannot stage installation package")?;
        Ok((Package(path), file))
    }
    fn check(&self, service: &str) -> Result<Value, String> {
        if DataKind::from_service(service).is_some() {
            return Err(
                "Prepare rule databases from the configuration page or import a local file".into(),
            );
        }
        let slot = self.slot(service)?;
        {
            let mut s = slot.lock().unwrap();
            s.expire_upload();
            if s.checking {
                return Err("A release check is already running".into());
            }
            if s.checked
                .is_some_and(|t| t.elapsed() < Duration::from_secs(60))
                && s.available.is_some()
            {
                return Ok(s.snapshot());
            }
            s.checking = true;
        }
        let result = releases::latest(service);
        let mut s = slot.lock().unwrap();
        s.checking = false;
        match result {
            Ok(release) => {
                s.available = Some(release);
                s.checked = Some(Instant::now());
                s.checked_at = Some(now());
                if !s.busy {
                    s.error = None;
                }
                Ok(s.snapshot())
            }
            Err(error) => {
                s.available = None;
                s.checked = None;
                s.checked_at = None;
                Err(error)
            }
        }
    }
    fn begin_upload(&self, service: &str, body: &Value) -> Result<Value, String> {
        let slot = self.slot(service)?;
        let mut s = slot.lock().unwrap();
        s.expire_upload();
        if s.busy {
            return Err("An installation or upload is already active for this service".into());
        }
        let size = body["size"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= s.package_limit)
            .ok_or("Package is empty or exceeds this service's size limit")?;
        let name = body["name"]
            .as_str()
            .filter(|v| v.len() <= 180 && !v.contains(['/', '\\']))
            .ok_or("Invalid package filename")?;
        let version = if let Some(kind) = DataKind::from_service(service) {
            if !kind.accepts(name) {
                return Err(format!("Choose {} for this database", kind.filename()));
            }
            kind.filename().to_owned()
        } else {
            releases::uploaded_version(service, name, body["version"].as_str())?
        };
        let (package, file) = self.new_package()?;
        let id = random_id()?;
        s.upload = Some(Upload {
            package,
            file,
            id: id.clone(),
            version: version.clone(),
            size,
            received: 0,
            touched: Instant::now(),
        });
        s.phase = "uploading";
        s.busy = true;
        s.error = None;
        s.received = 0;
        s.total = Some(size);
        s.version = Some(version);
        s.sha256 = None;
        Ok(json!({"upload_id":id,"chunk_size":CHUNK_MAX}))
    }
    fn chunk(&self, service: &str, body: &Value) -> Result<Value, String> {
        let slot = self.slot(service)?;
        let mut s = slot.lock().unwrap();
        s.expire_upload();
        let u = s
            .upload
            .as_mut()
            .filter(|u| body["upload_id"] == u.id)
            .ok_or("Upload expired or identifier invalid")?;
        let offset = body["offset"].as_u64().ok_or("Upload offset missing")?;
        let encoded = body["data"]
            .as_str()
            .filter(|v| v.len() <= CHUNK_MAX.div_ceil(3) * 4)
            .ok_or("Upload chunk exceeds limit")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| "Invalid upload data")?;
        if offset != u.received
            || bytes.is_empty()
            || bytes.len() > CHUNK_MAX
            || u.received + bytes.len() as u64 > u.size
        {
            return Err("Upload offset or size does not match; restart the upload".into());
        }
        u.file
            .write_all(&bytes)
            .map_err(|_| "Cannot write installation package")?;
        u.received += bytes.len() as u64;
        u.touched = Instant::now();
        let received = u.received;
        s.received = received;
        Ok(json!({"bytes_received":received}))
    }
    fn cancel_upload(&self, service: &str, body: &Value) -> Result<Value, String> {
        let slot = self.slot(service)?;
        let mut s = slot.lock().unwrap();
        if !s.upload.as_ref().is_some_and(|u| body["upload_id"] == u.id) {
            return Err("Upload identifier invalid".into());
        }
        s.upload = None;
        s.busy = false;
        s.phase = "idle";
        s.error = None;
        Ok(s.snapshot())
    }
    fn launch(
        self: &Arc<Self>,
        app: &crate::handlers::AppState,
        service: &str,
        body: &Value,
        uploaded: bool,
    ) -> Result<Value, String> {
        let slot = self.slot(service)?;
        let target = match service {
            "mihomo" => Target::Proxy(Arc::clone(app.proxy.legacy())),
            "tailscale" => Target::Tailscale(Arc::clone(&app.tailscale)),
            s if uploaded && DataKind::from_service(s).is_some() => Target::ProxyData(
                Arc::clone(app.proxy.legacy()),
                DataKind::from_service(s).unwrap(),
            ),
            _ => return Err("Unknown software service".into()),
        };
        let source;
        {
            let mut s = slot.lock().unwrap();
            s.expire_upload();
            if uploaded {
                let u = s
                    .upload
                    .as_mut()
                    .filter(|u| body["upload_id"] == u.id)
                    .ok_or("Upload identifier invalid")?;
                if u.received != u.size {
                    return Err("Upload is incomplete".into());
                }
                u.file
                    .flush()
                    .and_then(|()| u.file.sync_all())
                    .map_err(|_| "Cannot finish uploaded package")?;
                source = Source::Uploaded(s.upload.take().unwrap());
            } else {
                if s.busy {
                    return Err("An installation is already active for this service".into());
                }
                let release = s
                    .available
                    .as_ref()
                    .filter(|r| {
                        body["version"] == r.version
                            && body["sha256"] == r.sha256
                            && s.checked
                                .is_some_and(|t| t.elapsed() < Duration::from_secs(1800))
                    })
                    .ok_or("Check for updates again before installing this release")?
                    .clone();
                s.total = release.size;
                s.received = 0;
                s.version = Some(release.version.clone());
                source = Source::Official(release);
            }
            s.busy = true;
            s.phase = "preparing";
            s.error = None;
            s.sha256 = None;
        }
        let manager = Arc::clone(self);
        let worker_slot = Arc::clone(&slot);
        let spawned = std::thread::Builder::new()
            .name(format!("install-{service}"))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    manager.install(&worker_slot, target, source)
                }))
                .unwrap_or_else(|_| Err("Installation interrupted by an internal error".into()));
                let mut s = worker_slot.lock().unwrap_or_else(|p| p.into_inner());
                s.busy = false;
                s.phase = if result.is_ok() {
                    "succeeded"
                } else {
                    "failed"
                };
                s.error = result.err();
            });
        if spawned.is_err() {
            let mut s = slot.lock().unwrap();
            s.busy = false;
            s.phase = "failed";
            s.error = Some("Cannot start installer".into());
            return Err("Cannot start installer".into());
        }
        let snapshot = slot.lock().unwrap().snapshot();
        Ok(snapshot)
    }
    fn install(&self, slot: &Mutex<State>, target: Target, source: Source) -> Result<(), String> {
        let limit = slot.lock().unwrap().package_limit;
        let (package, version, expected) = match source {
            Source::Uploaded(u) => (u.package, u.version, None),
            Source::Official(release) => {
                let (package, mut file) = self.new_package()?;
                slot.lock().unwrap().phase = "downloading";
                let mut response = releases::client(600)?
                    .get(&release.url)
                    .send()
                    .and_then(reqwest::blocking::Response::error_for_status)
                    .map_err(|_| "Package download failed; check the router Internet connection")?;
                if response.content_length().is_some_and(|n| n > limit) {
                    return Err("Package exceeds size limit".into());
                }
                let mut received = 0;
                let mut buffer = [0u8; 64 * 1024];
                loop {
                    let n = response
                        .read(&mut buffer)
                        .map_err(|_| "Package download was interrupted")?;
                    if n == 0 {
                        break;
                    }
                    received += n as u64;
                    if received > limit {
                        return Err("Package exceeds size limit".into());
                    }
                    file.write_all(&buffer[..n])
                        .map_err(|_| "Cannot stage package download")?;
                    let mut s = slot.lock().unwrap();
                    s.received = received;
                    s.total = release.size.or(response.content_length());
                }
                file.sync_all()
                    .map_err(|_| "Cannot finish package download")?;
                (package, release.version, Some(release.sha256))
            }
        };
        slot.lock().unwrap().phase = "verifying";
        let mut file = File::open(&package.0).map_err(|_| "Installation package disappeared")?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let n = file
                .read(&mut buffer)
                .map_err(|_| "Cannot verify installation package")?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        let hash = format!("{:x}", hash.finalize());
        if expected.is_some_and(|expected| expected != hash) {
            return Err(
                "Package checksum does not match the official release; installation cancelled"
                    .into(),
            );
        }
        {
            let mut s = slot.lock().unwrap();
            s.sha256 = Some(hash);
            s.phase = "installing";
        }
        target.install(&package.0, &version)
    }
}
pub fn route(
    app: &crate::handlers::AppState,
    method: &crate::server::Method,
    path: &str,
    body: &[u8],
) -> (u16, Value) {
    use crate::server::Method;
    let Some((service, action)) = path
        .strip_prefix("/api/software/")
        .and_then(|p| p.split_once('/'))
    else {
        return (
            404,
            json!({"ok":false,"error":"Unknown installation route"}),
        );
    };
    let manager = &app.software;
    if manager.slot(service).is_err() {
        return (404, json!({"ok":false,"error":"Unknown software service"}));
    }
    if *method != Method::Get && !app.auth.has_password() {
        return (
            403,
            json!({"ok":false,"error":"Set an agent password before installing software"}),
        );
    }
    let parsed = if *method == Method::Post {
        match serde_json::from_slice::<Value>(body) {
            Ok(v) if v.is_object() => v,
            _ => return (400, json!({"ok":false,"error":"JSON object required"})),
        }
    } else {
        Value::Null
    };
    let result = match (method, action) {
        (Method::Get, "status") => manager.slot(service).map(|s| s.lock().unwrap().snapshot()),
        (Method::Get, "release") => manager.check(service),
        (Method::Post, "install") => manager.launch(app, service, &parsed, false),
        (Method::Post, "upload") => manager.begin_upload(service, &parsed),
        (Method::Post, "upload/chunk") => manager.chunk(service, &parsed),
        (Method::Post, "upload/cancel") => manager.cancel_upload(service, &parsed),
        (Method::Post, "upload/install") => manager.launch(app, service, &parsed, true),
        _ => {
            return (
                404,
                json!({"ok":false,"error":"Unknown installation operation"}),
            )
        }
    };
    match result {
        Ok(data) => (200, json!({"ok":true,"data":data})),
        Err(error) => (400, json!({"ok":false,"error":error})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn database_uploads_have_exact_types_and_do_not_require_a_software_version() {
        let root = std::env::temp_dir().join(format!("u60-data-upload-{}", random_id().unwrap()));
        let manager = Manager::at(root.clone());
        for kind in [
            DataKind::GeoIp,
            DataKind::GeoSite,
            DataKind::Mmdb,
            DataKind::Asn,
        ] {
            let s = kind.service();
            assert!(manager
                .begin_upload(s, &json!({"name":"engine.gz","size":4}))
                .is_err());
            assert!(manager
                .begin_upload(s, &json!({"name":kind.filename(),"size":geodata::LIMIT+1}))
                .is_err());
            let upload = manager
                .begin_upload(s, &json!({"name":kind.filename(),"size":4}))
                .unwrap();
            manager.cancel_upload(s, &upload).unwrap();
            assert!(manager.check(s).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn uploads_are_bounded_ordered_private_and_independent_per_service() {
        let root = std::env::temp_dir().join(format!("u60-upload-test-{}", random_id().unwrap()));
        let manager = Manager::at(root.clone());
        let a = manager
            .begin_upload(
                "mihomo",
                &json!({"name":"mihomo-linux-arm64-v1.2.3.gz","size":4}),
            )
            .unwrap();
        let b = manager
            .begin_upload(
                "tailscale",
                &json!({"name":"tailscale_1.2.3_arm64.tgz","size":4}),
            )
            .unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.encode([1, 2]);
        assert!(manager
            .begin_upload(
                "mihomo",
                &json!({"name":"mihomo-linux-arm64-v1.2.3.gz","size":4})
            )
            .is_err());
        assert!(manager
            .chunk(
                "tailscale",
                &json!({"upload_id":a["upload_id"],"offset":0,"data":bytes})
            )
            .is_err());
        assert!(manager
            .chunk(
                "mihomo",
                &json!({"upload_id":a["upload_id"],"offset":2,"data":bytes})
            )
            .is_err());
        assert_eq!(
            manager
                .chunk(
                    "mihomo",
                    &json!({"upload_id":a["upload_id"],"offset":0,"data":bytes})
                )
                .unwrap()["bytes_received"],
            2
        );
        assert!(manager
            .chunk(
                "mihomo",
                &json!({"upload_id":a["upload_id"],"offset":0,"data":bytes})
            )
            .is_err());
        let slot = manager.slot("mihomo").unwrap();
        let path = slot
            .lock()
            .unwrap()
            .upload
            .as_ref()
            .unwrap()
            .package
            .0
            .clone();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read(&path).unwrap(), [1, 2]);
        assert!(!slot
            .lock()
            .unwrap()
            .snapshot()
            .to_string()
            .contains(a["upload_id"].as_str().unwrap()));
        manager.cancel_upload("mihomo", &a).unwrap();
        assert!(!path.exists());
        assert!(manager.slot("tailscale").unwrap().lock().unwrap().busy);
        let tail = manager.slot("tailscale").unwrap();
        let mut state = tail.lock().unwrap();
        state.upload.as_mut().unwrap().touched =
            Instant::now() - UPLOAD_TTL - Duration::from_secs(1);
        assert_eq!(state.snapshot()["phase"], "failed");
        assert!(state.upload.is_none());
        drop(state);
        assert!(manager.cancel_upload("tailscale", &b).is_err());
        assert!(manager
            .begin_upload(
                "mihomo",
                &json!({"name":"mihomo-linux-arm64-v1.2.3.gz","size":PACKAGE_MAX+1})
            )
            .is_err());
        assert!(manager
            .begin_upload(
                "tailscale",
                &json!({"name":"../tailscale_1.2.3_arm64.tgz","size":4})
            )
            .is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
