//! Local migration helper for the paired deployment. Credentials never leave
//! the device or become command arguments; only the known root agent is read.
use std::{fs, io::Read, path::Path};

pub fn paired() -> bool {
    std::env::var("ZTE_AGENT_DEPLOYMENT").as_deref() == Ok("paired")
}

fn export_line(name: &str, value: &str) -> Option<String> {
    if !name.starts_with("ZTE_AGENT_")
        || name == "ZTE_AGENT_DEPLOYMENT"
        || !name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        return None;
    }
    Some(format!(
        "export {name}='{}'\n",
        value.replace('\'', "'\\''")
    ))
}

fn export_environment() -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no arguments or side effects.
    if unsafe { libc::geteuid() } != 0 {
        return Err("Run this migration on the router as root".into());
    }
    let providers = ["/data/zte-agent", "/data/local/tmp/zte-agent"]
        .into_iter()
        .filter_map(|p| fs::canonicalize(p).ok())
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    let mut credential = String::new();
    for process in fs::read_dir("/proc")
        .map_err(|_| "Cannot inspect processes")?
        .flatten()
    {
        if !process
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            continue;
        }
        let path = process.path();
        if !fs::read_link(path.join("exe")).is_ok_and(|exe| providers.contains(&exe))
            || !fs::metadata(&path).is_ok_and(|m| m.uid() == 0)
        {
            continue;
        }
        // Exclude diagnostics, cleanup helpers and update subprocesses.
        let cmd = fs::read(path.join("cmdline")).unwrap_or_default();
        if cmd.split(|b| *b == 0).filter(|v| !v.is_empty()).count() != 1 {
            continue;
        }
        let mut raw = Vec::new();
        fs::File::open(path.join("environ"))
            .map_err(|_| "Cannot read local agent environment")?
            .take(256 * 1024)
            .read_to_end(&mut raw)
            .map_err(|_| "Cannot read local agent environment")?;
        let mut output = String::from("# Private on-device agent environment. Do not publish.\n");
        let mut password = false;
        for entry in raw.split(|b| *b == 0) {
            let Ok(entry) = std::str::from_utf8(entry) else {
                continue;
            };
            let Some((name, value)) = entry.split_once('=') else {
                continue;
            };
            if name == "ZTE_AGENT_PASSWORD" {
                password = !value.is_empty();
                credential = value.into();
            }
            if name == "ZTE_AGENT_BIND" && value != "0.0.0.0:9090" && value != "127.0.0.1:9090" {
                return Err("Paired deployment requires the existing agent on port 9090".into());
            }
            if let Some(line) = export_line(name, value) {
                output.push_str(&line);
            }
        }
        if password {
            found.push(output);
        }
    }
    if found.len() != 1 {
        return Err("Exactly one running root agent with a configured password is required".into());
    }
    let root = Path::new("/data/u60-dash");
    if !fs::symlink_metadata(root)
        .is_ok_and(|m| m.is_dir() && !m.is_symlink() && m.uid() == 0 && m.mode() & 0o077 == 0)
    {
        return Err("Deployment directory must be root-owned and private".into());
    }
    let client = local_client()?;
    let token = login(&client, &credential)?;
    let tailscale = read_status(&client, &token, "/api/tailscale/status")?;
    let proxy =
        read_status(&client, &token, "/api/proxy/status").unwrap_or(serde_json::Value::Null);
    let required = serde_json::json!({"tailscale":tailscale["healthy"]==true,"gateway":tailscale["gateway_mode"]==true,"proxy":proxy["running"]==true});
    crate::ota::storage::atomic_write(
        &root.join("health.json"),
        &serde_json::to_vec(&required).unwrap(),
        0o600,
    )
    .map_err(|_| "Cannot save required service health")?;
    crate::ota::storage::atomic_write(&root.join("agent.env"), found[0].as_bytes(), 0o600)
        .map_err(|_| "Cannot preserve agent environment")?;
    Ok(())
}

fn local_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(1))
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|_| "Cannot build local health client".into())
}
fn login(client: &reqwest::blocking::Client, password: &str) -> Result<String, String> {
    let value: serde_json::Value = client
        .post("http://127.0.0.1:9090/api/auth/login")
        .json(&serde_json::json!({"password":password}))
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(|r| r.json())
        .map_err(|_| "Local agent authentication failed")?;
    value["data"]["token"]
        .as_str()
        .filter(|s| s.len() == 64)
        .map(str::to_owned)
        .ok_or_else(|| "Local health token unavailable".into())
}
fn read_status(
    client: &reqwest::blocking::Client,
    token: &str,
    path: &str,
) -> Result<serde_json::Value, String> {
    let mut reply = client
        .get(format!("http://127.0.0.1:9090{path}"))
        .bearer_auth(token)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|_| "Local service health unavailable")?;
    let mut bytes = Vec::new();
    reply
        .by_ref()
        .take(64 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|_| "Invalid service health reply")?;
    let v: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid service health reply")?;
    if v["ok"] != true {
        return Err("Service health check failed".into());
    }
    Ok(v["data"].clone())
}
fn health() -> Result<(), String> {
    let required: serde_json::Value = serde_json::from_slice(
        &fs::read("/data/u60-dash/health.json")
            .map_err(|_| "Deployment health baseline missing")?,
    )
    .map_err(|_| "Invalid deployment baseline")?;
    let client = local_client()?;
    let token = login(
        &client,
        &std::env::var("ZTE_AGENT_PASSWORD").map_err(|_| "Agent environment missing")?,
    )?;
    let ts = read_status(&client, &token, "/api/tailscale/status")?;
    if required["tailscale"] == true
        && (ts["healthy"] != true || required["gateway"] != ts["gateway_mode"])
    {
        return Err("Waiting for existing Tailscale connectivity and mode".into());
    }
    let proxy = read_status(&client, &token, "/api/proxy/status")?;
    if required["proxy"] == true && proxy["running"] != true {
        return Err("Waiting for existing proxy service".into());
    }
    Ok(())
}
pub fn health_cli() -> i32 {
    match health() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

fn wait_ready() -> Result<(), String> {
    let client = local_client()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        match client.get("http://127.0.0.1:9090/api/device").send() {
            Ok(reply) if reply.status().as_u16() == 401 => {
                return login(
                    &client,
                    &std::env::var("ZTE_AGENT_PASSWORD")
                        .map_err(|_| "Agent environment missing")?,
                )
                .map(|_| ());
            }
            Ok(reply) if reply.status().as_u16() != 503 => {
                return Err(format!(
                    "Unexpected agent readiness status: {}",
                    reply.status()
                ));
            }
            _ => std::thread::sleep(std::time::Duration::from_millis(500)),
        }
    }
    Err("Agent HTTP listener did not become ready within 30 seconds".into())
}

pub fn ready_cli() -> i32 {
    match wait_ready() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub fn network_cli() -> i32 {
    match crate::tailscale::prepare_management() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub fn cli() -> i32 {
    match export_environment() {
        Ok(()) => {
            println!("Existing agent credentials and preferences preserved on device.");
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn environment_values_remain_literal_shell_data() {
        let value = "example ' quoted $HOME `false`\nsecond line";
        let line = export_line("ZTE_AGENT_PASSWORD", value).unwrap();
        let result = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("{line}printf %s \"$ZTE_AGENT_PASSWORD\""))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8(result.stdout).unwrap(), value);
        assert!(export_line("ZTE_AGENT_BAD;false", "x").is_none());
        assert!(export_line("ZTE_OTA_SUPERVISOR", "x").is_none());
        assert!(export_line("PATH", "x").is_none());
    }
}
