use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Component, Path},
};

pub(crate) fn extract_tailscale(
    package: &Path,
    version: &str,
    destination: &Path,
) -> Result<(), String> {
    let file = File::open(package).map_err(|_| "Cannot open installation package")?;
    let decoder = flate2::read::GzDecoder::new(file).take(256 * 1024 * 1024 + 1);
    let mut archive = tar::Archive::new(decoder);
    let prefix = format!("tailscale_{version}_arm64");
    let mut found = BTreeSet::new();
    let mut total = 0u64;
    fs::create_dir_all(destination).map_err(|_| "Cannot create package staging directory")?;
    for (index, entry) in archive
        .entries()
        .map_err(|_| "Invalid compressed package")?
        .enumerate()
    {
        if index >= 64 {
            return Err("Package contains too many entries".into());
        }
        let mut entry = entry.map_err(|_| "Invalid package entry")?;
        let path = entry
            .path()
            .map_err(|_| "Invalid package path")?
            .into_owned();
        if path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
            || !path.starts_with(&prefix)
        {
            return Err("Package contains an unsafe or unexpected path".into());
        }
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        if !kind.is_file() {
            return Err("Package links and special files are not supported".into());
        }
        let size = entry.header().size().map_err(|_| "Invalid package size")?;
        total = total
            .checked_add(size)
            .ok_or("Package exceeds size limit")?;
        if size > 128 * 1024 * 1024 || total > 240 * 1024 * 1024 {
            return Err("Expanded package exceeds size limit".into());
        }
        let name = if path == Path::new(&prefix).join("tailscale") {
            "tailscale"
        } else if path == Path::new(&prefix).join("tailscaled") {
            "tailscaled"
        } else {
            continue;
        };
        if !found.insert(name) {
            return Err("Package contains duplicate executables".into());
        }
        let mut out = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o700)
            .open(destination.join(name))
            .map_err(|_| "Cannot stage package executable")?;
        std::io::copy(&mut entry, &mut out).map_err(|_| "Package extraction failed")?;
        out.flush().map_err(|_| "Cannot flush package executable")?;
        let mut header = Vec::new();
        File::open(destination.join(name))
            .map_err(|_| "Cannot inspect executable")?
            .take(64 * 1024)
            .read_to_end(&mut header)
            .map_err(|_| "Cannot inspect executable")?;
        if !crate::proxy::elf(&header) {
            return Err("Package must contain static Linux ARM64 executables".into());
        }
    }
    if found.len() != 2 {
        return Err("Package does not contain both Tailscale ARM64 executables".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn elf() -> Vec<u8> {
        let mut b = vec![0u8; 120];
        b[..4].copy_from_slice(b"\x7fELF");
        b[4] = 2;
        b[5] = 1;
        b[18..20].copy_from_slice(&183u16.to_le_bytes());
        b[32..40].copy_from_slice(&64u64.to_le_bytes());
        b[54..56].copy_from_slice(&56u16.to_le_bytes());
        b[56..58].copy_from_slice(&1u16.to_le_bytes());
        b
    }
    #[test]
    fn extraction_checks_architecture_duplicates_missing_files_and_links() {
        for scenario in [
            "valid",
            "foreign",
            "duplicate",
            "missing",
            "symlink",
            "wrong-version",
        ] {
            let root = std::env::temp_dir().join(format!(
                "u60-archive-{}-{}",
                std::process::id(),
                scenario
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            let file = File::create(root.join("package.tgz")).unwrap();
            let gz = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
            let mut tar = tar::Builder::new(gz);
            for name in if scenario == "duplicate" {
                vec!["tailscale", "tailscale", "tailscaled"]
            } else if scenario == "missing" {
                vec!["tailscale"]
            } else {
                vec!["tailscale", "tailscaled"]
            } {
                let mut binary = elf();
                if scenario == "foreign" {
                    binary[18] = 62;
                }
                let mut header = tar::Header::new_gnu();
                header.set_mode(0o755);
                if scenario == "symlink" {
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_link_name("/etc/passwd").unwrap();
                    binary.clear();
                }
                header.set_size(binary.len() as u64);
                header.set_cksum();
                tar.append_data(
                    &mut header,
                    format!("tailscale_1.2.3_arm64/{name}"),
                    &binary[..],
                )
                .unwrap();
            }
            tar.into_inner().unwrap().finish().unwrap();
            let result = extract_tailscale(
                &root.join("package.tgz"),
                if scenario == "wrong-version" {
                    "1.2.4"
                } else {
                    "1.2.3"
                },
                &root.join("out"),
            );
            assert_eq!(
                result.is_ok(),
                scenario == "valid",
                "{scenario}: {result:?}"
            );
            if scenario == "valid" {
                assert_eq!(fs::read(root.join("out/tailscaled")).unwrap(), elf());
            }
            fs::remove_dir_all(root).unwrap();
        }
    }
}
