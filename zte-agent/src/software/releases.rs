use serde::Serialize;
use serde_json::Value;
use std::{io::Read, time::Duration};

#[derive(Clone, Debug, Serialize)]
pub struct Release {
    pub version: String,
    pub name: String,
    pub url: String,
    pub sha256: String,
    pub size: Option<u64>,
    pub architecture: &'static str,
}
pub fn version(raw: &str) -> Result<String, String> {
    let raw = raw.strip_prefix('v').unwrap_or(raw);
    let v = semver::Version::parse(raw).map_err(|_| "Use a stable version such as 1.2.3")?;
    if raw.len() > 32 || !v.pre.is_empty() || !v.build.is_empty() {
        return Err("A stable release version is required".into());
    }
    Ok(v.to_string())
}
pub fn digest(raw: &str) -> Result<String, String> {
    let raw = raw.trim().strip_prefix("sha256:").unwrap_or(raw.trim());
    if raw.len() != 64 || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("The official release did not provide a valid SHA-256".into());
    }
    Ok(raw.to_ascii_lowercase())
}
pub fn client(seconds: u64) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .user_agent("u60-pro-dash/software-updates")
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(seconds))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| "Cannot create release client".into())
}
fn get(url: &str) -> Result<Vec<u8>, String> {
    let response = client(40)?
        .get(url)
        .send()
        .map_err(|_| "Cannot reach the official release source")?;
    if matches!(response.status().as_u16(), 403 | 429) {
        return Err("The release source is rate limited; try again later".into());
    }
    let mut out = Vec::new();
    response
        .error_for_status()
        .map_err(|_| "Official release information is unavailable")?
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut out)
        .map_err(|_| "Release response was interrupted")?;
    if out.len() > 2 * 1024 * 1024 {
        return Err("Release response exceeds the size limit".into());
    }
    Ok(out)
}
pub fn proxy_release(value: &Value) -> Result<Release, String> {
    if value["draft"] == true || value["prerelease"] == true {
        return Err("A stable release is required".into());
    }
    let version = version(
        value["tag_name"]
            .as_str()
            .ok_or("Release version missing")?,
    )?;
    let name = format!("mihomo-linux-arm64-v{version}.gz");
    let asset = value["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|a| a["name"] == name))
        .ok_or("No compatible Linux ARM64 package in this release")?;
    let url = format!("https://github.com/MetaCubeX/mihomo/releases/download/v{version}/{name}");
    if asset["browser_download_url"] != url {
        return Err("Unexpected official asset location".into());
    }
    Ok(Release {
        version,
        name,
        url,
        sha256: digest(asset["digest"].as_str().unwrap_or(""))?,
        size: asset["size"].as_u64(),
        architecture: "linux-arm64",
    })
}
pub fn tailscale_file(value: &Value) -> Result<(String, String), String> {
    let version = version(
        value["TarballsVersion"]
            .as_str()
            .ok_or("Stable package version missing")?,
    )?;
    let name = format!("tailscale_{version}_arm64.tgz");
    if value["Tarballs"]["arm64"] != name {
        return Err("No compatible Linux ARM64 package in this release".into());
    }
    Ok((version, name))
}
pub fn latest(service: &str) -> Result<Release, String> {
    match service {
        "metacubexd" => panel_release(
            &serde_json::from_slice(&get(
                "https://api.github.com/repos/MetaCubeX/metacubexd/releases/latest",
            )?)
            .map_err(|_| "Invalid dashboard release response")?,
        ),
        "mihomo" => proxy_release(
            &serde_json::from_slice(&get(
                "https://api.github.com/repos/MetaCubeX/mihomo/releases/latest",
            )?)
            .map_err(|_| "Invalid release response")?,
        ),
        "tailscale" => {
            let value: Value =
                serde_json::from_slice(&get("https://pkgs.tailscale.com/stable/?mode=json")?)
                    .map_err(|_| "Invalid release response")?;
            let (version, name) = tailscale_file(&value)?;
            let url = format!("https://pkgs.tailscale.com/stable/{name}");
            let checksum = get(&format!("{url}.sha256"))?;
            Ok(Release {
                version,
                name,
                url,
                sha256: digest(
                    std::str::from_utf8(&checksum).map_err(|_| "Invalid release checksum")?,
                )?,
                size: None,
                architecture: "linux-arm64",
            })
        }
        _ => Err("Unknown software service".into()),
    }
}

fn panel_release(value: &Value) -> Result<Release, String> {
    if value["draft"] == true || value["prerelease"] == true {
        return Err("A stable dashboard release is required".into());
    }
    let raw = value["tag_name"]
        .as_str()
        .ok_or("Dashboard version missing")?;
    let version = version(raw)?;
    let name = "compressed-dist.tgz";
    let asset = value["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|a| a["name"] == name))
        .ok_or("No static dashboard archive in this release")?;
    let url = format!("https://github.com/MetaCubeX/metacubexd/releases/download/{raw}/{name}");
    if asset["browser_download_url"] != url {
        return Err("Unexpected dashboard asset location".into());
    }
    Ok(Release {
        version,
        name: name.into(),
        url,
        sha256: digest(asset["digest"].as_str().unwrap_or(""))?,
        size: asset["size"].as_u64(),
        architecture: "web",
    })
}
pub fn uploaded_version(
    service: &str,
    name: &str,
    supplied: Option<&str>,
) -> Result<String, String> {
    if let Some(v) = supplied.filter(|v| !v.trim().is_empty()) {
        return version(v.trim());
    }
    let inferred = match service {
        "mihomo" => name
            .strip_prefix("mihomo-linux-arm64-v")
            .map(|v| v.strip_suffix(".gz").unwrap_or(v)),
        "tailscale" => name
            .strip_prefix("tailscale_")
            .and_then(|v| v.strip_suffix("_arm64.tgz")),
        _ => None,
    };
    version(inferred.ok_or(
        "Version could not be read from the filename; enter it for this manual installation",
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn check_live_release(service: &str) {
        let release = latest(service).unwrap();
        assert_eq!(release.architecture, "linux-arm64");
        assert_eq!(release.sha256.len(), 64);
        assert_eq!(
            uploaded_version(service, &release.name, None).unwrap(),
            release.version
        );
        println!("{service}: {} ({})", release.version, release.name);
    }
    #[test]
    #[ignore = "requires Internet access to official metadata; does not install or download binaries"]
    fn official_proxy_metadata_has_compatible_package() {
        check_live_release("mihomo");
    }
    #[test]
    #[ignore = "requires Internet access to official metadata; does not install or download binaries"]
    fn official_tailscale_metadata_has_compatible_package() {
        check_live_release("tailscale");
    }
    #[test]
    fn static_panel_release_requires_the_official_archive_and_digest() {
        let mut release = json!({"tag_name":"v1.200.0","assets":[{"name":"compressed-dist.tgz","digest":format!("sha256:{}","b".repeat(64)),"size":12345,"browser_download_url":"https://github.com/MetaCubeX/metacubexd/releases/download/v1.200.0/compressed-dist.tgz"}]});
        let selected = panel_release(&release).unwrap();
        assert_eq!(selected.version, "1.200.0");
        assert_eq!(selected.architecture, "web");
        release["assets"][0]["browser_download_url"] = json!("https://example.invalid/archive.tgz");
        assert!(panel_release(&release).is_err());
        release["assets"][0]["browser_download_url"] = json!("https://github.com/MetaCubeX/metacubexd/releases/download/v1.200.0/compressed-dist.tgz");
        release["assets"][0]["digest"] = Value::Null;
        assert!(panel_release(&release).is_err());
    }
    #[test]
    fn selects_only_stable_linux_arm64_and_requires_official_digest() {
        let asset = |name: &str| json!({"name":name,"digest":format!("sha256:{}","a".repeat(64)),"size":100,"browser_download_url":format!("https://github.com/MetaCubeX/mihomo/releases/download/v1.2.3/{name}")});
        let mut release = json!({"tag_name":"v1.2.3","assets":[asset("mihomo-android-arm64-v8-v1.2.3.gz"),asset("mihomo-linux-arm64-v1.2.3.gz"),asset("mihomo-linux-arm64-v1.2.3.deb")]});
        assert_eq!(
            proxy_release(&release).unwrap().name,
            "mihomo-linux-arm64-v1.2.3.gz"
        );
        release["assets"][1]["digest"] = Value::Null;
        assert!(proxy_release(&release).is_err());
        release["prerelease"] = json!(true);
        assert!(proxy_release(&release).is_err());
        for invalid in ["../1.2.3", "1.2", "1.2.3-beta.1", "1.2.3+build"] {
            assert!(version(invalid).is_err());
        }
        assert!(tailscale_file(
            &json!({"TarballsVersion":"1.2.3","Tarballs":{"arm64":"tailscale_1.2.3_amd64.tgz"}})
        )
        .is_err());
        assert_eq!(
            uploaded_version("tailscale", "tailscale_1.104.4_arm64.tgz", None).unwrap(),
            "1.104.4"
        );
        assert!(uploaded_version("tailscale", "tailscale_1.104.4_amd64.tgz", None).is_err());
    }
}
