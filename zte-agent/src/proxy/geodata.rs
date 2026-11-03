//! Prepare rule databases before starting the engine. Validation runs in a
//! private workspace, so an interrupted native download cannot poison caches.
use super::*;
use std::io::Write;

pub(crate) const LIMIT: u64 = 64 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    GeoIp,
    GeoSite,
    Mmdb,
    Asn,
}
impl Kind {
    pub(crate) fn service(self) -> &'static str {
        match self {
            Self::GeoIp => "mihomo-geoip",
            Self::GeoSite => "mihomo-geosite",
            Self::Mmdb => "mihomo-mmdb",
            Self::Asn => "mihomo-asn",
        }
    }
    pub(crate) fn from_service(s: &str) -> Option<Self> {
        Self::all().into_iter().find(|k| k.service() == s)
    }
    fn all() -> [Self; 4] {
        [Self::GeoIp, Self::GeoSite, Self::Mmdb, Self::Asn]
    }
    pub(crate) fn filename(self) -> &'static str {
        match self {
            Self::GeoIp => "GeoIP.dat",
            Self::GeoSite => "GeoSite.dat",
            Self::Mmdb => "geoip.metadb",
            Self::Asn => "ASN.mmdb",
        }
    }
    pub(crate) fn accepts(self, name: &str) -> bool {
        name.eq_ignore_ascii_case(self.filename())
            || (self == Self::Mmdb
                && ["Country.mmdb", "geoip.db"]
                    .iter()
                    .any(|n| name.eq_ignore_ascii_case(n)))
            || (self == Self::Asn && name.eq_ignore_ascii_case("GeoLite2-ASN.mmdb"))
    }
    fn existing(self, root: &Path) -> PathBuf {
        let mut names = fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| {
                self.accepts(n)
                    && !(self == Self::Asn && n.eq_ignore_ascii_case("GeoLite2-ASN.mmdb"))
            })
            .collect::<Vec<_>>();
        names.sort();
        root.join(names.first().map(String::as_str).unwrap_or(self.filename()))
    }
    fn url(self, config: &Value) -> Result<String, String> {
        let (key, default) = match self {
            Self::GeoIp => ("geoip", "geoip.dat"),
            Self::GeoSite => ("geosite", "geosite.dat"),
            Self::Mmdb => ("mmdb", "geoip.metadb"),
            Self::Asn => ("asn", "GeoLite2-ASN.mmdb"),
        };
        let url = match config.get("geox-url").and_then(|v| v.get(key)) {
            None => format!(
                "https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/{default}"
            ),
            Some(Value::String(s)) => s.clone(),
            _ => return Err(format!("Invalid {key} rule database URL")),
        };
        https_url(&url)
            .map_err(|_| format!("{} requires an HTTPS download URL", self.filename()))?;
        Ok(url)
    }
}

fn required(config: &Value) -> Vec<Kind> {
    fn scan(v: &Value, found: &mut [bool; 3]) {
        match v {
            Value::String(s) => {
                let s = s.to_ascii_uppercase();
                found[0] |= s.contains("GEOIP,") || s.starts_with("GEOIP:");
                found[1] |= s.contains("GEOSITE,") || s.starts_with("GEOSITE:");
                found[2] |= s.contains("IP-ASN,");
            }
            Value::Array(a) => {
                for v in a {
                    scan(v, found);
                }
            }
            Value::Object(o) => {
                for (k, v) in o {
                    scan(&Value::String(k.clone()), found);
                    scan(v, found);
                }
            }
            _ => {}
        }
    }
    let mut found = [false; 3];
    scan(config, &mut found);
    let mut kinds = Vec::new();
    if found[0] {
        kinds.push(if config["geodata-mode"] == true {
            Kind::GeoIp
        } else {
            Kind::Mmdb
        });
    }
    // DNS's geoip fallback uses the MMDB reader independently of rule mode.
    if (config["dns"]["fallback-filter"]["geoip"] == true
        || (config["dns"]["fallback"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
            && config["dns"]["fallback-filter"]["geoip"] != false))
        && !kinds.contains(&Kind::Mmdb)
    {
        kinds.push(Kind::Mmdb);
    }
    if found[1] {
        kinds.push(Kind::GeoSite);
    }
    if found[2] {
        kinds.push(Kind::Asn);
    }
    kinds
}

fn varint(bytes: &[u8], pos: &mut usize) -> Result<u64, ()> {
    let mut value = 0;
    for shift in (0..70).step_by(7) {
        let b = *bytes.get(*pos).ok_or(())?;
        *pos += 1;
        if shift == 63 && b > 1 {
            return Err(());
        }
        value |= u64::from(b & 127) << shift;
        if b & 128 == 0 {
            return Ok(value);
        }
    }
    Err(())
}
// Check every length, including entries after the category the engine uses.
// Looking up one category alone can accept a partially downloaded protobuf.
fn fields(bytes: &[u8], mut visit: impl FnMut(u64, u8, &[u8]) -> Result<(), ()>) -> Result<(), ()> {
    let mut pos = 0;
    while pos < bytes.len() {
        let tag = varint(bytes, &mut pos)?;
        if tag >> 3 == 0 {
            return Err(());
        }
        let wire = (tag & 7) as u8;
        let length = match wire {
            0 => {
                let start = pos;
                varint(bytes, &mut pos)?;
                visit(tag >> 3, wire, &bytes[start..pos])?;
                continue;
            }
            1 => 8,
            2 => usize::try_from(varint(bytes, &mut pos)?).map_err(|_| ())?,
            5 => 4,
            _ => return Err(()),
        };
        let end = pos
            .checked_add(length)
            .filter(|n| *n <= bytes.len())
            .ok_or(())?;
        visit(tag >> 3, wire, &bytes[pos..end])?;
        pos = end;
    }
    Ok(())
}
fn category(kind: Kind, bytes: &[u8]) -> Result<String, ()> {
    let mut first = None;
    fields(bytes, |tag, wire, entry| {
        if tag != 1 || wire != 2 {
            return Err(());
        }
        let mut name = None;
        fields(entry, |tag, wire, value| {
            match (tag, wire) {
                (1, 2) => {
                    let text = std::str::from_utf8(value).map_err(|_| ())?;
                    if text.is_empty() || text.len() > 256 {
                        return Err(());
                    }
                    name = Some(text.to_owned());
                }
                (2, 2) => fields(value, |tag, wire, value| {
                    if tag == 1
                        && kind == Kind::GeoIp
                        && (wire != 2 || !matches!(value.len(), 4 | 16))
                    {
                        return Err(());
                    }
                    if tag == 1 && kind == Kind::GeoSite && wire != 0 {
                        return Err(());
                    }
                    if tag == 2 && kind == Kind::GeoIp && wire != 0 {
                        return Err(());
                    }
                    if tag == 2
                        && kind == Kind::GeoSite
                        && (wire != 2 || std::str::from_utf8(value).is_err())
                    {
                        return Err(());
                    }
                    Ok(())
                })?,
                _ => {}
            }
            Ok(())
        })?;
        let name = name.ok_or(())?;
        if first.is_none() {
            first = Some(name);
        }
        Ok(())
    })?;
    first.ok_or(())
}
fn inspect(kind: Kind, bytes: &[u8]) -> Result<(), String> {
    let valid = !bytes.is_empty()
        && bytes.len() as u64 <= LIMIT
        && match kind {
            Kind::GeoIp | Kind::GeoSite => category(kind, bytes).is_ok(),
            // The native reader performs full MMDB validation before promotion.
            Kind::Mmdb | Kind::Asn => {
                bytes.len() >= 64
                    && bytes
                        .windows(14)
                        .rposition(|v| v == b"\xab\xcd\xefMaxMind.com")
                        .is_some_and(|n| bytes.len() - n <= 128 * 1024 && bytes.len() - n > 32)
            }
        };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{} is incomplete or invalid; prepare rule databases or import a complete file",
            kind.filename()
        ))
    }
}
fn read(kind: Kind, path: &Path) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "Rule database is missing")?;
    if !meta.is_file() || meta.len() > LIMIT {
        return Err("Invalid rule database file".into());
    }
    let bytes = fs::read(path).map_err(|_| "Cannot read rule database")?;
    inspect(kind, &bytes)?;
    Ok(bytes)
}

pub(super) struct Workspace {
    pub path: PathBuf,
    changed: Vec<(Kind, PathBuf)>,
}
impl Workspace {
    pub fn new(root: &Path) -> Result<Self, String> {
        for entry in fs::read_dir(root).into_iter().flatten().flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name
                .strip_prefix(".geodata-")
                .is_some_and(|s| s.len() == 24 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                && entry.file_type().is_ok_and(|t| t.is_dir())
                && entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > Duration::from_secs(7200))
            {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
        let mut id = [0; 12];
        getrandom::getrandom(&mut id).map_err(|_| "Cannot create validation workspace")?;
        let path = root.join(format!(
            ".geodata-{}",
            id.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        fs::create_dir(&path).map_err(|_| "Cannot create validation workspace")?;
        let work = Self {
            path,
            changed: Vec::new(),
        };
        fs::set_permissions(&work.path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect validation workspace")?;
        Ok(work)
    }
    pub fn prepare(
        &mut self,
        root: &Path,
        config: &Value,
        force: bool,
        allow_updates: bool,
    ) -> Result<(), String> {
        self.prepare_with(root, config, force, allow_updates, fetch)
    }
    fn prepare_with(
        &mut self,
        root: &Path,
        config: &Value,
        force: bool,
        allow_updates: bool,
        mut download: impl FnMut(Kind, &str, &Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let needed = required(config);
        for kind in Kind::all() {
            let existing = kind.existing(root);
            let cached = read(kind, &existing);
            let replace = needed.contains(&kind) && (force || cached.is_err());
            if replace {
                if !allow_updates {
                    return Err(
                        "Stop this service before preparing or repairing its rule databases".into(),
                    );
                }
                download(kind, &kind.url(config)?, &self.path.join(kind.filename()))?;
                read(kind, &self.path.join(kind.filename()))?;
                self.changed.push((kind, existing));
            } else if let Ok(bytes) = cached {
                private_write(&self.path.join(kind.filename()), &bytes, 0o600)?;
            }
        }
        Ok(())
    }
    pub fn include_native_downloads(
        &mut self,
        root: &Path,
        allow_updates: bool,
    ) -> Result<(), String> {
        for kind in Kind::all() {
            let target = kind.existing(root);
            let staged = kind.existing(&self.path);
            if !self.changed.iter().any(|(k, _)| *k == kind)
                && staged.is_file()
                && read(kind, &target).is_err()
            {
                if !allow_updates {
                    return Err("Stop this service before preparing new rule databases".into());
                }
                read(kind, &staged)?;
                if staged != self.path.join(kind.filename()) {
                    fs::rename(&staged, self.path.join(kind.filename()))
                        .map_err(|_| "Cannot stage native rule database")?;
                }
                self.changed.push((kind, target));
            }
        }
        Ok(())
    }
    pub fn promote(&self) -> Result<(), String> {
        for (kind, _) in &self.changed {
            read(*kind, &self.path.join(kind.filename()))?;
        }
        // Keep backups until all replacements succeed. Renames are atomic on
        // this filesystem; no existing database is truncated in place.
        let mut replaced: Vec<(&PathBuf, PathBuf, bool)> = Vec::new();
        let result = (|| {
            for (kind, target) in &self.changed {
                let backup = self.path.join(format!("{}.previous", kind.filename()));
                let existed = fs::symlink_metadata(target).is_ok();
                if existed {
                    fs::hard_link(target, &backup)
                        .map_err(|_| "Cannot preserve previous rule database")?;
                }
                fs::rename(self.path.join(kind.filename()), target)
                    .map_err(|_| "Cannot replace rule database")?;
                replaced.push((target, backup, existed));
            }
            Ok(())
        })();
        if result.is_err() {
            let mut restored = true;
            for (target, backup, existed) in replaced.into_iter().rev() {
                restored &= if existed {
                    fs::rename(backup, target).is_ok()
                } else {
                    fs::remove_file(target).is_ok()
                };
            }
            if !restored {
                return Err(
                    "Rule database replacement failed; restoration could not be confirmed".into(),
                );
            }
        }
        result
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn fetch(kind: Kind, url: &str, path: &Path) -> Result<(), String> {
    let result = (|| {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .https_only(true)
            .user_agent("u60-pro-dash")
            .redirect(reqwest::redirect::Policy::limited(5))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(90))
            .build()
            .map_err(|_| "Cannot create download client")?;
        let response = client
            .get(url)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(download_error)?;
        let expected = response.content_length();
        if expected.is_some_and(|n| n > LIMIT) {
            return Err("Rule database exceeds 64 MiB".into());
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|_| "Cannot stage rule database")?;
        let size = std::io::copy(&mut response.take(LIMIT + 1), &mut file)
            .map_err(|_| "Rule database download interrupted")?;
        if size == 0 || size > LIMIT || expected.is_some_and(|n| n != size) {
            return Err("Rule database download was incomplete or exceeded 64 MiB".into());
        }
        file.flush()
            .and_then(|()| file.sync_all())
            .map_err(|_| "Cannot finish rule database download")?;
        Ok(())
    })();
    result.map_err(|e: String| {
        format!(
            "{}: {e}. Retry preparation or import this database from your computer",
            kind.filename()
        )
    })
}

impl MihomoService {
    pub(crate) fn install_geodata(&self, kind: Kind, package: &Path) -> Result<(), String> {
        let _op = self
            .operation
            .try_lock()
            .map_err(|_| "Another proxy operation is running")?;
        self.prepare()?;
        self.lease()?;
        let result = (|| {
            if self.runtime.lock().unwrap().child.is_some() {
                return Err("Stop this service before importing rule databases".into());
            }
            let bytes = read(kind, package)?;
            let mut work = Workspace::new(&self.root)?;
            private_write(&work.path.join(kind.filename()), &bytes, 0o600)?;
            let rule = match kind {
                Kind::GeoIp => format!(
                    "GEOIP,{},DIRECT,no-resolve",
                    category(kind, &bytes).map_err(|_| "Invalid GeoIP database")?
                ),
                Kind::GeoSite => format!(
                    "GEOSITE,{},DIRECT",
                    category(kind, &bytes).map_err(|_| "Invalid GeoSite database")?
                ),
                Kind::Mmdb => "GEOIP,CN,DIRECT,no-resolve".into(),
                Kind::Asn => "IP-ASN,1,DIRECT,no-resolve".into(),
            };
            let config = work.path.join("check.json");
            private_write(
                &config,
                &serde_json::to_vec(
                    &json!({"geodata-mode":kind == Kind::GeoIp,"rules":[rule,"MATCH,DIRECT"]}),
                )
                .unwrap(),
                0o600,
            )?;
            validation::run_raw(
                &engine::Mihomo,
                &self.root.join("engine"),
                &work.path,
                &config,
            )?;
            work.changed.push((kind, kind.existing(&self.root)));
            work.promote()
        })();
        let mut r = self.runtime.lock().unwrap();
        if r.child.is_none() {
            r.lock = None;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dataset(name: &str) -> Vec<u8> {
        let mut v = vec![10, (name.len() + 2) as u8, 10, name.len() as u8];
        v.extend(name.as_bytes());
        v
    }
    fn root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("u60-geodata-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn rejects_truncated_later_entries_and_wrong_dat_type() {
        let mut bytes = dataset("CN");
        bytes.extend([10, 127, 10, 7]);
        assert!(inspect(Kind::GeoIp, &bytes).is_err());
        assert!(inspect(Kind::GeoIp, b"<html>error</html>").is_err());
        assert!(fields(
            &[10, 255, 255, 255, 255, 255, 255, 255, 255, 255, 2],
            |_, _, _| Ok(())
        )
        .is_err());
        let site = b"\x0a\x0f\x0a\x02CN\x12\x09\x08\x02\x12\x05a.com";
        inspect(Kind::GeoSite, site).unwrap();
        assert!(inspect(Kind::GeoIp, site).is_err());
    }
    #[test]
    fn derives_dat_mmdb_dns_and_nested_rule_dependencies() {
        assert_eq!(
            required(
                &json!({"geodata-mode":true,"rules":["AND,((GEOIP,private),(IP-ASN,1)),DIRECT"],"dns":{"nameserver-policy":{"geosite:cn":"https://example.com"},"fallback-filter":{"geoip":true}}})
            ),
            vec![Kind::GeoIp, Kind::Mmdb, Kind::GeoSite, Kind::Asn]
        );
        assert_eq!(
            required(&json!({"rules":["GEOIP,CN,DIRECT"]})),
            vec![Kind::Mmdb]
        );
        assert!(required(&json!({"rules":["MATCH,DIRECT"]})).is_empty());
    }
    #[test]
    fn partial_download_and_failed_promotion_preserve_existing_files() {
        let root = root("preserve");
        let original = dataset("CN");
        fs::write(root.join("GeoIP.dat"), &original).unwrap();
        let config = json!({"geodata-mode":true,"rules":["GEOIP,CN,DIRECT","GEOSITE,CN,DIRECT"]});
        let mut work = Workspace::new(&root).unwrap();
        assert!(work
            .prepare_with(&root, &config, true, true, |_, _, path| {
                fs::write(path, [10, 127]).unwrap();
                Ok(())
            })
            .is_err());
        assert_eq!(fs::read(root.join("GeoIP.dat")).unwrap(), original);
        drop(work);
        let mut work = Workspace::new(&root).unwrap();
        work.prepare_with(&root, &config, true, true, |_, _, path| {
            fs::write(path, dataset("US")).unwrap();
            Ok(())
        })
        .unwrap();
        fs::create_dir(root.join("GeoSite.dat")).unwrap();
        assert!(work.promote().is_err());
        assert_eq!(fs::read(root.join("GeoIP.dat")).unwrap(), original);
        drop(work);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn offline_complete_cache_is_reused_and_live_repair_is_refused() {
        let root = root("offline");
        fs::write(root.join("GeoIP.dat"), dataset("CN")).unwrap();
        let config = json!({"geodata-mode":true,"rules":["GEOIP,CN,DIRECT"],"geox-url":{"geoip":"https://example.com/unavailable"}});
        let mut work = Workspace::new(&root).unwrap();
        work.prepare_with(&root, &config, false, false, |_, _, _| {
            panic!("offline cache must not be downloaded again")
        })
        .unwrap();
        assert!(work
            .prepare_with(&root, &config, true, false, |_, _, _| panic!())
            .unwrap_err()
            .contains("Stop this service"));
        drop(work);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn offline_import_validates_before_replacement_and_releases_the_lease() {
        let root = root("import");
        let service = MihomoService::at(root.clone());
        private_write(&root.join("engine"), b"#!/bin/sh\nexit 1\n", 0o700).unwrap();
        let original = dataset("CN");
        fs::write(root.join("GeoIP.dat"), &original).unwrap();
        let package = root.join("upload");
        fs::write(&package, dataset("US")).unwrap();
        assert!(service.install_geodata(Kind::GeoIp, &package).is_err());
        assert_eq!(fs::read(root.join("GeoIP.dat")).unwrap(), original);
        assert!(service.runtime.lock().unwrap().lock.is_none());
        private_write(&root.join("engine"), b"#!/bin/sh\nexit 0\n", 0o700).unwrap();
        service.install_geodata(Kind::GeoIp, &package).unwrap();
        assert_eq!(fs::read(root.join("GeoIP.dat")).unwrap(), dataset("US"));
        assert!(!root.join("source.json").exists());
        assert_eq!(
            fs::metadata(root.join("GeoIP.dat"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(service.runtime.lock().unwrap().lock.is_none());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires public database samples via U60_GEOIP_TEST_FILE and U60_GEOSITE_TEST_FILE"]
    fn external_complete_databases_and_truncated_copy() {
        for (kind, env) in [
            (Kind::GeoIp, "U60_GEOIP_TEST_FILE"),
            (Kind::GeoSite, "U60_GEOSITE_TEST_FILE"),
        ] {
            let bytes = fs::read(std::env::var(env).unwrap()).unwrap();
            inspect(kind, &bytes).unwrap();
            assert!(inspect(kind, &bytes[..bytes.len() - 1]).is_err());
        }
    }
    #[test]
    #[ignore = "requires public database downloads and U60_PROXY_TEST_ENGINE"]
    fn external_bootstrap_repairs_truncation_before_real_engine_validation() {
        let root = root("external-bootstrap");
        fs::write(root.join("GeoIP.dat"), [10, 255]).unwrap();
        let binary = PathBuf::from(std::env::var("U60_PROXY_TEST_ENGINE").unwrap());
        let config = root.join("check.json");
        let source = json!({"geodata-mode":true,"geodata-loader":"memconservative",
            "geox-url":{"geoip":"https://cdn.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geoip.dat","geosite":"https://cdn.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geosite.dat"},
            "rules":["GEOSITE,private,DIRECT","GEOIP,private,DIRECT,no-resolve","GEOSITE,cn,DIRECT","GEOIP,CN,DIRECT,no-resolve","MATCH,DIRECT"]});
        private_write(&config, &serde_json::to_vec(&source).unwrap(), 0o600).unwrap();
        run_checked(&engine::Mihomo, &binary, &root, &config).unwrap();
        let complete = read(Kind::GeoIp, &root.join("GeoIP.dat")).unwrap();
        read(Kind::GeoSite, &root.join("GeoSite.dat")).unwrap();
        assert!(complete.len() > 1_000_000);
        assert!(!root.join("source.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
