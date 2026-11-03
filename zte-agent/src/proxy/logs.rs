//! Bounded private logs survive a failed start without retaining credentials.
use super::*;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::sync::mpsc;

const FILE_LIMIT: usize = 256 * 1024;
const REPLY_LIMIT: usize = 96 * 1024;
const LINE_LIMIT: usize = 4096;

struct Guard(File);
fn lock(root: &Path) -> Result<Guard, String> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(root.join("logs.lock"))
        .map_err(|_| "Cannot open log lock")?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err("Cannot lock proxy logs".into());
    }
    Ok(Guard(file))
}
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

fn append(root: &Path, text: &str) -> Result<(), String> {
    let _lock = lock(root)?;
    let path = root.join("engine.log");
    if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file()) {
        return Err("Invalid proxy log file".into());
    }
    if fs::metadata(&path).is_ok_and(|m| m.len() as usize + text.len() + 1 > FILE_LIMIT) {
        fs::rename(&path, root.join("engine.log.1")).map_err(|_| "Cannot rotate proxy log")?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "Cannot open proxy log")?;
    writeln!(file, "{text}").map_err(|_| "Cannot write proxy log".into())
}
pub(super) fn record(root: &Path, text: &str) {
    let _ = append(root, text);
}

struct Redactor(Vec<String>);
impl Redactor {
    fn new(config: &Value) -> Self {
        fn collect(v: &Value, out: &mut Vec<String>) {
            match v {
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
                            if let Some(s) = value.as_str().filter(|s| !s.is_empty()) {
                                out.push(s.to_owned());
                            }
                        }
                        collect(value, out);
                    }
                }
                Value::Array(values) => {
                    for v in values {
                        collect(v, out);
                    }
                }
                _ => {}
            }
        }
        let mut secrets = Vec::new();
        collect(config, &mut secrets);
        secrets.sort();
        secrets.dedup();
        secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        Self(secrets)
    }
    fn clean(&self, bytes: &[u8]) -> String {
        let mut text = String::from_utf8_lossy(bytes).into_owned();
        for secret in &self.0 {
            text = text.replace(secret, "[redacted]");
        }
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
            .take(LINE_LIMIT)
            .collect()
    }
}

pub(super) fn capture(root: &Path, reader: impl Read + Send + 'static) -> mpsc::Receiver<()> {
    let root = root.to_owned();
    let config = fs::read(root.join("runtime.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let mut redactor = Redactor::new(&config);
    let stamp = |path: &Path| {
        fs::metadata(path)
            .ok()
            .map(|m| (m.ino(), m.len(), m.mtime(), m.mtime_nsec()))
    };
    let mut revision = stamp(&root.join("runtime.json"));
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut chunk = [0; 4096];
        let mut line = Vec::new();
        let mut oversized = false;
        let mut redaction_full = false;
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            // Reload keeps the process alive. Remember credentials from each
            // applied configuration, including lines buffered before a reload.
            let current = stamp(&root.join("runtime.json"));
            if current != revision && !redaction_full {
                if let Ok(config) = fs::read(root.join("runtime.json")).and_then(|b| {
                    serde_json::from_slice::<Value>(&b).map_err(std::io::Error::other)
                }) {
                    redactor.0.extend(Redactor::new(&config).0);
                    redactor.0.sort();
                    redactor.0.dedup();
                    redaction_full =
                        redactor.0.iter().map(String::len).sum::<usize>() > CONFIG_LIMIT * 2;
                    redactor.0.sort_by_key(|s| std::cmp::Reverse(s.len()));
                    revision = current;
                }
            }
            for &byte in &chunk[..n] {
                if byte == b'\n' {
                    let _ = append(
                        &root,
                        &if redaction_full {
                            "[log line omitted: credential cache limit]".into()
                        } else if oversized {
                            "[log line omitted: size limit]".into()
                        } else {
                            redactor.clean(&line)
                        },
                    );
                    line.clear();
                    oversized = false;
                } else if line.len() < LINE_LIMIT {
                    line.push(byte);
                } else {
                    oversized = true;
                }
            }
        }
        if !line.is_empty() {
            let _ = append(
                &root,
                &if redaction_full {
                    "[log line omitted: credential cache limit]".into()
                } else if oversized {
                    "[log line omitted: size limit]".into()
                } else {
                    redactor.clean(&line)
                },
            );
        }
        let _ = send.send(());
    });
    receive
}

impl MihomoService {
    pub(super) fn logs(&self) -> Result<Value, String> {
        if !self.root.is_dir() {
            return Ok(json!({"text":"","truncated":false,"limit_bytes":REPLY_LIMIT}));
        }
        let _lock = lock(&self.root)?;
        let mut bytes = Vec::new();
        for name in ["engine.log.1", "engine.log"] {
            let path = self.root.join(name);
            if fs::symlink_metadata(&path)
                .is_ok_and(|m| m.is_file() && m.len() <= (FILE_LIMIT + LINE_LIMIT) as u64)
            {
                bytes.extend(fs::read(path).map_err(|_| "Cannot read proxy log")?);
            }
        }
        let truncated = bytes.len() > REPLY_LIMIT;
        if truncated {
            bytes.drain(..bytes.len() - REPLY_LIMIT);
            if let Some(n) = bytes.iter().position(|b| *b == b'\n') {
                bytes.drain(..=n);
            }
        }
        Ok(
            json!({"text":String::from_utf8_lossy(&bytes),"truncated":truncated,"limit_bytes":REPLY_LIMIT}),
        )
    }
    pub(super) fn clear_logs(&self) -> Result<(), String> {
        let _lock = lock(&self.root)?;
        private_write(&self.root.join("engine.log"), b"", 0o600)?;
        match fs::remove_file(self.root.join("engine.log.1")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("Cannot clear proxy log".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_redacts_credentials_and_rotates_without_unbounded_lines() {
        let root = std::env::temp_dir().join(format!("u60-logs-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        private_write(
            &root.join("runtime.json"),
            br#"{"secret":"controller-example","proxies":[{"password":"node-example"}]}"#,
            0o600,
        )
        .unwrap();
        let mut bytes =
            b"level=error controller-example node-example https://example.com/sub?token=private\n"
                .to_vec();
        bytes.extend(vec![b'x'; LINE_LIMIT * 4]);
        bytes.extend(b"\nlast line\n");
        capture(&root, std::io::Cursor::new(bytes))
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        let service = MihomoService::at(root.clone());
        let text = service.logs().unwrap()["text"].as_str().unwrap().to_owned();
        assert!(text.contains("last line"));
        assert!(!text.contains("controller-example"));
        assert!(!text.contains("node-example"));
        assert!(!text.contains("token=private"));
        let (reader, mut writer) = std::os::unix::net::UnixStream::pair().unwrap();
        let finished = capture(&root, reader);
        private_write(
            &root.join("runtime.json"),
            br#"{"password":"after-reload-example"}"#,
            0o600,
        )
        .unwrap();
        writer
            .write_all(b"node-example after-reload-example\n")
            .unwrap();
        drop(writer);
        finished.recv_timeout(Duration::from_secs(2)).unwrap();
        let text = service.logs().unwrap()["text"].as_str().unwrap().to_owned();
        assert!(!text.contains("node-example"));
        assert!(!text.contains("after-reload-example"));
        for _ in 0..150 {
            append(&root, &"a".repeat(LINE_LIMIT)).unwrap();
        }
        assert!(fs::metadata(root.join("engine.log")).unwrap().len() <= FILE_LIMIT as u64);
        assert!(fs::metadata(root.join("engine.log.1")).unwrap().len() <= FILE_LIMIT as u64);
        assert_eq!(service.logs().unwrap()["truncated"], true);
        service.clear_logs().unwrap();
        assert_eq!(service.logs().unwrap()["text"], "");
        append(&root, "after clear").unwrap();
        assert!(service.logs().unwrap()["text"]
            .as_str()
            .unwrap()
            .contains("after clear"));
        fs::remove_dir_all(root).unwrap();
    }
}
