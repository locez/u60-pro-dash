use super::*;
use std::{io::Write, os::unix::fs::symlink, path::Component};

const EXPANDED_LIMIT: u64 = 160 * 1024 * 1024;
struct Staging(PathBuf, bool);
impl Drop for Staging {
    fn drop(&mut self) {
        if !self.1 {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn unpack(package: &Path, destination: &Path) -> Result<(), String> {
    let mut archive = tar::Archive::new(
        flate2::read::GzDecoder::new(
            File::open(package).map_err(|_| "Cannot open dashboard package")?,
        )
        .take(EXPANDED_LIMIT + 1024 * 1024),
    );
    let mut total = 0u64;
    for (n, entry) in archive
        .entries()
        .map_err(|_| "Invalid dashboard archive")?
        .enumerate()
    {
        if n > 12000 {
            return Err("Dashboard archive contains too many entries".into());
        }
        let mut entry = entry.map_err(|_| "Invalid dashboard archive entry")?;
        let raw = entry
            .path()
            .map_err(|_| "Invalid dashboard path")?
            .into_owned();
        if raw
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err("Unsafe dashboard archive path".into());
        }
        let path = raw
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect::<PathBuf>();
        if path.as_os_str().is_empty() {
            continue;
        }
        let kind = entry.header().entry_type();
        if !kind.is_dir() && !kind.is_file() {
            return Err("Dashboard links and special files are not supported".into());
        }
        let target = destination.join(path);
        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|_| "Cannot stage dashboard directory")?;
            continue;
        }
        let size = entry
            .header()
            .size()
            .map_err(|_| "Invalid dashboard file size")?;
        total = total
            .checked_add(size)
            .ok_or("Expanded dashboard exceeds size limit")?;
        if size > 32 * 1024 * 1024 || total > EXPANDED_LIMIT {
            return Err("Expanded dashboard exceeds size limit".into());
        }
        fs::create_dir_all(target.parent().unwrap())
            .map_err(|_| "Cannot stage dashboard directory")?;
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(target)
            .map_err(|_| "Duplicate or unwritable dashboard file")?;
        std::io::copy(&mut entry, &mut out).map_err(|_| "Dashboard extraction was interrupted")?;
        out.flush().map_err(|_| "Cannot finish dashboard file")?;
    }
    // Consume the gzip trailer too; a truncated archive must not replace a
    // working dashboard even if all advertised tar entries were extracted.
    let mut decoder = archive.into_inner();
    std::io::copy(&mut decoder, &mut std::io::sink())
        .map_err(|_| "Dashboard archive checksum failed")?;
    if decoder.limit() == 0 {
        return Err("Expanded dashboard stream exceeds size limit".into());
    }
    let index = fs::read(destination.join("index.html"))
        .map_err(|_| "Dashboard package has no index.html")?;
    if index.len() > 1024 * 1024
        || !String::from_utf8_lossy(&index)
            .to_ascii_lowercase()
            .contains("<html")
    {
        return Err("Invalid dashboard index.html".into());
    }
    Ok(())
}

impl MihomoService {
    pub(crate) fn panel_status(&self) -> Value {
        json!({"installed":self.root.join("panel/index.html").is_file(),"version":fs::read_to_string(self.root.join("panel/.u60-version")).ok(),"running":self.runtime.lock().unwrap().child.is_some()})
    }
    pub(crate) fn install_panel(&self, package: &Path, version: &str) -> Result<(), String> {
        let _op = self
            .operation
            .try_lock()
            .map_err(|_| "Another proxy operation is running")?;
        self.prepare()?;
        let mut random = [0; 12];
        getrandom::getrandom(&mut random).map_err(|_| "Cannot stage dashboard")?;
        let name = format!(
            "panel-{}",
            random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let mut stage = Staging(self.root.join(&name), false);
        fs::create_dir(&stage.0).map_err(|_| "Cannot stage dashboard")?;
        fs::set_permissions(&stage.0, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect dashboard staging")?;
        unpack(package, &stage.0)?;
        private_write(&stage.0.join(".u60-version"), version.as_bytes(), 0o600)?;
        let previous = fs::read_link(self.root.join("panel")).ok();
        let next = self.root.join("panel.next");
        let _ = fs::remove_file(&next);
        symlink(&name, &next).map_err(|_| "Cannot stage dashboard link")?;
        fs::rename(next, self.root.join("panel")).map_err(|_| "Cannot activate dashboard")?;
        stage.1 = true;
        for entry in fs::read_dir(&self.root).into_iter().flatten().flatten() {
            let file = entry.file_name();
            let file = file.to_string_lossy();
            if file != name
                && previous.as_deref() != Some(Path::new(file.as_ref()))
                && file
                    .strip_prefix("panel-")
                    .is_some_and(|s| s.len() == 24 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                && entry.file_type().is_ok_and(|t| t.is_dir())
            {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
        Ok(())
    }
    pub(crate) fn panel_file(&self, name: &str) -> Result<(String, Vec<u8>), String> {
        if name.len() > 1024
            || name
                .split('/')
                .any(|s| s.is_empty() || s.starts_with('.') || s.contains(['\\', '%']))
        {
            return Err("Invalid dashboard asset path".into());
        }
        let mime = match name.rsplit('.').next().unwrap_or("") {
            "html" => "text/html; charset=utf-8",
            "js" | "mjs" => "text/javascript",
            "css" => "text/css",
            "json" | "webmanifest" => "application/json",
            "svg" => "image/svg+xml",
            "png" => "image/png",
            "ico" => "image/x-icon",
            "woff2" => "font/woff2",
            "woff" => "font/woff",
            "ttf" => "font/ttf",
            "otf" => "font/otf",
            "webp" => "image/webp",
            "wasm" => "application/wasm",
            "txt" => "text/plain",
            _ => return Err("Unsupported dashboard asset".into()),
        };
        let root = self
            .root
            .join("panel")
            .canonicalize()
            .map_err(|_| "Install the dashboard first")?;
        let path = root
            .join(name)
            .canonicalize()
            .map_err(|_| "Dashboard asset not found")?;
        if !path.starts_with(&root) {
            return Err("Invalid dashboard asset path".into());
        }
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|_| "Dashboard asset not found")?
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read dashboard asset")?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("Dashboard asset exceeds limit".into());
        }
        Ok((mime.into(), bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_or_linked_dashboard_archive_preserves_the_installed_panel() {
        let root = std::env::temp_dir().join(format!("u60-panel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let service = MihomoService::at(root.clone());
        for bad in [false, true] {
            let package = root.join("panel.tgz");
            let file = File::create(&package).unwrap();
            let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
                file,
                flate2::Compression::fast(),
            ));
            let mut header = tar::Header::new_gnu();
            let bytes = b"<html>dashboard</html>";
            header.set_size(if bad { 0 } else { bytes.len() as u64 });
            header.set_mode(0o600);
            if bad {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name("/etc/passwd").unwrap();
            }
            header.set_cksum();
            tar.append_data(
                &mut header,
                "index.html",
                if bad { &b""[..] } else { &bytes[..] },
            )
            .unwrap();
            tar.into_inner().unwrap().finish().unwrap();
            assert_eq!(service.install_panel(&package, "1.0.0").is_ok(), !bad);
            assert_eq!(service.panel_file("index.html").unwrap().1, bytes);
            assert!(service.panel_file("../settings.json").is_err());
            assert!(service.panel_file(".u60-version").is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
