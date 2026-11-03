//! Keep engine diagnostics bounded and useful without retaining private logs.
use super::*;
use std::sync::mpsc;

const LOG_LIMIT: usize = 16 * 1024;

fn capture(mut reader: impl Read + Send + 'static) -> mpsc::Receiver<Vec<u8>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut tail = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    tail.extend_from_slice(&buffer[..count]);
                    if tail.len() > LOG_LIMIT {
                        tail.drain(..tail.len() - LOG_LIMIT);
                    }
                }
            }
        }
        let _ = send.send(tail);
    });
    receive
}

fn detail(bytes: &[u8], config: &Value) -> String {
    let text = String::from_utf8_lossy(bytes);
    let useful = text
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error") || lower.contains("fatal") || lower.contains("invalid")
        })
        .collect::<Vec<_>>();
    let mut text = if useful.is_empty() {
        text.lines()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        useful.join(" ")
    };
    fn redact(text: &mut String, value: &Value) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    if [
                        "password",
                        "secret",
                        "uuid",
                        "token",
                        "private-key",
                        "preshared-key",
                        "username",
                    ]
                    .contains(&key.as_str())
                    {
                        if let Some(secret) = value.as_str().filter(|s| !s.is_empty()) {
                            *text = text.replace(secret, "[redacted]");
                        }
                    }
                    redact(text, value);
                }
            }
            Value::Array(items) => {
                for item in items {
                    redact(text, item);
                }
            }
            _ => {}
        }
    }
    redact(&mut text, config);
    for scheme in ["https://", "http://"] {
        while let Some(start) = text.find(scheme) {
            let end = text[start..]
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>'))
                .map_or(text.len(), |n| start + n);
            text.replace_range(start..end, "[URL]");
        }
    }
    text.chars()
        .filter(|c| !c.is_control())
        .take(1200)
        .collect()
}

pub(super) fn run_checked(
    driver: &dyn engine::Engine,
    binary: &Path,
    root: &Path,
    config: &Path,
) -> Result<(), String> {
    let mut child = driver
        .command(binary, root, config, true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Cannot run installed proxy engine")?;
    let stdout = capture(child.stdout.take().unwrap());
    let stderr = capture(child.stderr.take().unwrap());
    let end = Instant::now() + Duration::from_secs(20);
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err("Proxy engine rejected the configuration")
                }
            }
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("Proxy validation timed out; initial geodata or provider downloads may still be unavailable");
            }
        }
    };
    let mut bytes = stdout
        .recv_timeout(Duration::from_secs(1))
        .unwrap_or_default();
    bytes.extend(
        stderr
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or_default(),
    );
    outcome.map_err(|reason| {
        let source = fs::read(config)
            .ok()
            .and_then(|v| serde_json::from_slice(&v).ok())
            .unwrap_or(Value::Null);
        let message = detail(&bytes, &source);
        if message.is_empty() {
            reason.to_owned()
        } else {
            format!("{reason}: {message}")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_returns_the_actual_reason_without_credentials_or_urls() {
        let root = std::env::temp_dir().join(format!("u60-validation-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let binary = root.join("engine");
        let config = root.join("config.json");
        private_write(&binary, b"#!/bin/sh\nprintf '%s\\n' 'level=error msg=ProxyGroup_Auto_has_unset_type password=example-private https://example.com/?token=private' >&2\nexit 1\n", 0o700).unwrap();
        private_write(
            &config,
            br#"{"proxies":[{"password":"example-private"}]}"#,
            0o600,
        )
        .unwrap();
        let error = run_checked(&engine::Mihomo, &binary, &root, &config).unwrap_err();
        assert!(error.contains("ProxyGroup_Auto_has_unset_type"));
        assert!(!error.contains("example-private"));
        assert!(!error.contains("https://"));
        assert!(!error.contains("token="));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn capture_drains_large_output_and_keeps_only_the_bounded_tail() {
        let mut bytes = vec![b'x'; LOG_LIMIT * 10];
        bytes.extend_from_slice(b"last error");
        let result = capture(std::io::Cursor::new(bytes))
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert_eq!(result.len(), LOG_LIMIT);
        assert!(result.ends_with(b"last error"));
    }
}
