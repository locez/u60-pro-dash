//! Local screen gate. A root administrator is outside this access boundary.
use anyhow::{Context, Result, ensure};
use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

const MEMORY: u32 = 19 * 1024;
const ITERATIONS: u32 = 2;
const HOLD: Duration = Duration::from_millis(1000);

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u8,
    hash: Option<String>,
    attempts: u8,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Hold,
    Unlock,
    New,
    Confirm,
    Change,
    Disable,
}
enum Work {
    Verify(bool, Mode),
    Hash(String),
}

pub struct ScreenLock {
    pub locked: bool,
    pub mode: Mode,
    pub message: String,
    pub progress: f32,
    pub input: Zeroizing<String>,
    first: Zeroizing<String>,
    path: Option<PathBuf>,
    stored: Stored,
    hold_started: Option<Instant>,
    until: Option<Instant>,
    work: Option<Receiver<Result<Work, String>>>,
}

fn kdf() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(MEMORY, ITERATIONS, 1, Some(32)).expect("fixed Argon2 params"),
    )
}
fn delay(attempts: u8) -> Duration {
    Duration::from_secs(if attempts < 5 {
        0
    } else {
        (30u64 << (attempts - 5).min(4)).min(300)
    })
}
fn validate_hash(value: &str) -> Result<()> {
    let hash =
        PasswordHash::new(value).map_err(|_| anyhow::anyhow!("invalid screen lock verifier"))?;
    ensure!(
        hash.algorithm.as_str() == "argon2id"
            && hash.version == Some(19)
            && hash.params.get_decimal("m") == Some(MEMORY)
            && hash.params.get_decimal("t") == Some(ITERATIONS)
            && hash.params.get_decimal("p") == Some(1)
            && hash.salt.is_some()
            && hash.hash.as_ref().is_some_and(|h| h.len() == 32),
        "unsupported screen lock verifier"
    );
    Ok(())
}

impl ScreenLock {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let stored = if let Some(path) = path.as_ref().filter(|p| p.exists()) {
            let metadata = std::fs::symlink_metadata(path)?;
            ensure!(
                metadata.is_file() && !metadata.is_symlink(),
                "screen lock must be a regular file"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                ensure!(
                    metadata.permissions().mode() & 0o077 == 0,
                    "screen lock file must be private (0600)"
                );
            }
            let bytes = std::fs::read(path)?;
            ensure!(bytes.len() <= 2048, "screen lock file is too large");
            let value: Stored = serde_json::from_slice(&bytes)
                .context("invalid screen lock file; refusing to bypass it")?;
            ensure!(
                value.version == 1 && value.attempts <= 20,
                "invalid screen lock metadata"
            );
            if let Some(hash) = &value.hash {
                validate_hash(hash)?;
            }
            value
        } else {
            Stored {
                version: 1,
                hash: None,
                attempts: 0,
            }
        };
        let until =
            (!delay(stored.attempts).is_zero()).then(|| Instant::now() + delay(stored.attempts));
        Ok(Self {
            locked: true,
            mode: Mode::Hold,
            message: String::new(),
            progress: 0.,
            input: Zeroizing::new(String::new()),
            first: Zeroizing::new(String::new()),
            path,
            stored,
            hold_started: None,
            until,
            work: None,
        })
    }
    pub fn enabled(&self) -> bool {
        self.stored.hash.is_some()
    }
    pub fn busy(&self) -> bool {
        self.work.is_some()
    }
    pub fn remaining(&self) -> u64 {
        self.until.map_or(0, |t| {
            t.saturating_duration_since(Instant::now())
                .as_secs()
                .saturating_add(u64::from(t > Instant::now()))
        })
    }
    fn commit(&mut self, value: Stored) -> Result<()> {
        let path = self
            .path
            .as_ref()
            .context("a persistent screen preference path is required")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            let metadata = std::fs::symlink_metadata(parent)?;
            ensure!(
                metadata.is_dir() && !metadata.is_symlink(),
                "screen lock directory must not be a symlink"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        crate::config::write_json_atomic(path, &value)?;
        self.stored = value;
        Ok(())
    }
    pub fn lock(&mut self) {
        self.locked = true;
        self.cancel();
    }
    pub fn cancel(&mut self) {
        self.mode = Mode::Hold;
        self.message.clear();
        self.input.zeroize();
        self.first.zeroize();
        self.hold_started = None;
        self.progress = 0.;
        self.work = None;
        // A canceled in-flight attempt still counts; sleep/restart cannot give free guesses.
        if self.stored.attempts >= 5 && self.remaining() == 0 {
            self.until = Some(Instant::now() + delay(self.stored.attempts));
        }
    }
    pub fn hold_start(&mut self) {
        if self.locked && self.mode == Mode::Hold {
            self.hold_started = Some(Instant::now());
            self.progress = 0.;
        }
    }
    pub fn hold_tick(&mut self) {
        self.progress = self.hold_started.map_or(0., |t| {
            (t.elapsed().as_secs_f32() / HOLD.as_secs_f32()).min(1.)
        });
    }
    pub fn hold_cancel(&mut self) {
        self.hold_started = None;
        self.progress = 0.;
    }
    pub fn hold_release(&mut self, inside: bool) {
        let complete = inside
            && self
                .hold_started
                .take()
                .is_some_and(|t| t.elapsed() >= HOLD);
        self.progress = 0.;
        if !complete || !self.locked || self.mode != Mode::Hold {
            return;
        }
        if self.enabled() {
            self.mode = Mode::Unlock;
            self.message = "请输入解锁密码".into();
        } else {
            self.locked = false;
        }
    }
    pub fn setup(&mut self, disable: bool) {
        if self.locked {
            return;
        }
        self.cancel();
        if self.path.is_none() {
            self.message = "请通过预览脚本启动，以保存密码设置。".into();
            return;
        }
        self.mode = if !self.enabled() {
            Mode::New
        } else if disable {
            Mode::Disable
        } else {
            Mode::Change
        };
        self.message = if self.mode == Mode::New {
            "设置 6–12 位数字密码"
        } else {
            "先输入当前密码"
        }
        .into();
    }
    pub fn digit(&mut self, n: i32) {
        if self.mode != Mode::Hold
            && !self.busy()
            && self.remaining() == 0
            && (0..=9).contains(&n)
            && self.input.len() < 12
        {
            self.input.push(char::from(b'0' + n as u8));
        }
    }
    pub fn backspace(&mut self) {
        if !self.busy() {
            self.input.pop();
        }
    }
    pub fn submit(&mut self) {
        if self.busy() || self.mode == Mode::Hold || self.remaining() != 0 {
            return;
        }
        if !(6..=12).contains(&self.input.len()) {
            self.message = "请输入 6–12 位数字".into();
            return;
        }
        let pin = std::mem::replace(&mut self.input, Zeroizing::new(String::new()));
        match self.mode {
            Mode::New => {
                self.first = pin;
                self.mode = Mode::Confirm;
                self.message = "再次输入，确认新密码".into();
            }
            Mode::Confirm => {
                if *pin != *self.first {
                    self.first.zeroize();
                    self.mode = Mode::New;
                    self.message = "两次不一致，请重新设置".into();
                    return;
                }
                self.first.zeroize();
                self.spawn(move || {
                    let mut salt = [0u8; 16];
                    getrandom::getrandom(&mut salt).map_err(|_| "无法生成密码盐".to_string())?;
                    let salt =
                        SaltString::encode_b64(&salt).map_err(|_| "密码盐生成失败".to_string())?;
                    let hash = kdf()
                        .hash_password(pin.as_bytes(), &salt)
                        .map_err(|_| "密码处理失败".to_string())?
                        .to_string();
                    Ok(Work::Hash(hash))
                });
            }
            mode @ (Mode::Unlock | Mode::Change | Mode::Disable) => {
                let Some(hash) = self.stored.hash.clone() else {
                    return;
                };
                let mut stored = self.stored.clone();
                stored.attempts = stored.attempts.saturating_add(1).min(20);
                if self.commit(stored).is_err() {
                    self.message = "无法保存验证状态，保持锁定".into();
                    return;
                }
                self.spawn(move || {
                    let hash =
                        PasswordHash::new(&hash).map_err(|_| "密码校验配置损坏".to_string())?;
                    let ok = kdf().verify_password(pin.as_bytes(), &hash).is_ok();
                    Ok(Work::Verify(ok, mode))
                });
            }
            Mode::Hold => {}
        }
    }
    fn spawn(&mut self, job: impl FnOnce() -> Result<Work, String> + Send + 'static) {
        self.message = "正在验证…".into();
        let (tx, rx) = mpsc::channel();
        self.work = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(job());
        });
    }
    pub fn poll(&mut self) {
        let result = match self.work.as_ref().map(|r| r.try_recv()) {
            Some(Ok(value)) => value,
            Some(Err(mpsc::TryRecvError::Disconnected)) => Err("验证任务中断，请重试".into()),
            _ => return,
        };
        self.work = None;
        let result = match result {
            Ok(v) => v,
            Err(message) => {
                self.message = message;
                return;
            }
        };
        match result {
            Work::Hash(hash) => {
                if self
                    .commit(Stored {
                        version: 1,
                        hash: Some(hash),
                        attempts: 0,
                    })
                    .is_err()
                {
                    self.message = "密码未能保存，请重试".into();
                    return;
                }
                self.mode = Mode::Hold;
                self.message = "密码解锁已开启".into();
                self.until = None;
            }
            Work::Verify(false, _) => {
                self.until = Some(Instant::now() + delay(self.stored.attempts));
                self.message = "密码不正确，请重试".into();
            }
            Work::Verify(true, mode) => {
                let mut stored = self.stored.clone();
                stored.attempts = 0;
                if mode == Mode::Disable {
                    stored.hash = None;
                }
                if self.commit(stored).is_err() {
                    self.message = "无法保存验证结果，保持锁定".into();
                    return;
                }
                self.until = None;
                if mode == Mode::Unlock {
                    self.locked = false;
                    self.mode = Mode::Hold;
                    self.message.clear();
                } else if mode == Mode::Change {
                    self.mode = Mode::New;
                    self.message = "设置新的 6–12 位数字密码".into();
                } else {
                    self.mode = Mode::Hold;
                    self.message = "密码解锁已关闭".into();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    };

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "u60-lock-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf {
            self.0.join("private").join("screen-lock.private.json")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn finish(s: &mut ScreenLock) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while s.busy() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            s.poll();
        }
        assert!(!s.busy(), "password worker timed out");
    }
    fn enter(s: &mut ScreenLock, pin: &str) {
        for c in pin.bytes() {
            s.digit((c - b'0') as i32);
        }
        s.submit();
        finish(s);
    }
    fn hold(s: &mut ScreenLock) {
        s.hold_start();
        s.hold_started = Some(Instant::now() - HOLD);
        s.hold_release(true);
    }
    fn seeded(temp: &Temp) -> ScreenLock {
        static HASH: OnceLock<String> = OnceLock::new();
        let hash = HASH
            .get_or_init(|| {
                let salt = SaltString::encode_b64(b"test-salt-16bytes").unwrap();
                kdf().hash_password(b"123456", &salt).unwrap().to_string()
            })
            .clone();
        let mut s = ScreenLock::load(Some(temp.file())).unwrap();
        s.commit(Stored {
            version: 1,
            hash: Some(hash),
            attempts: 0,
        })
        .unwrap();
        s
    }

    #[test]
    fn long_press_requires_duration_and_release_inside() {
        let mut s = ScreenLock::load(None).unwrap();
        s.hold_start();
        s.hold_release(true);
        assert!(s.locked);
        s.hold_start();
        s.hold_started = Some(Instant::now() - HOLD);
        s.hold_release(false);
        assert!(s.locked);
        s.hold_start();
        s.hold_started = Some(Instant::now() - HOLD);
        s.hold_cancel();
        s.hold_release(true);
        assert!(s.locked);
        hold(&mut s);
        assert!(!s.locked);
        s.lock();
        assert!(s.locked);
    }

    #[test]
    fn setup_confirm_restart_change_and_disable_require_the_right_pin() {
        let temp = Temp::new();
        let mut s = ScreenLock::load(Some(temp.file())).unwrap();
        hold(&mut s);
        s.setup(false);
        enter(&mut s, "123456");
        enter(&mut s, "123456");
        assert!(s.enabled());
        let saved = std::fs::read_to_string(temp.file()).unwrap();
        assert!(saved.contains("$argon2id$"));
        assert!(!saved.contains("123456"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(temp.file()).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let mut s = ScreenLock::load(Some(temp.file())).unwrap();
        assert!(s.locked);
        hold(&mut s);
        assert!(s.locked);
        enter(&mut s, "654321");
        assert!(s.locked);
        enter(&mut s, "123456");
        assert!(!s.locked);
        s.setup(false);
        enter(&mut s, "123456");
        assert!(s.mode == Mode::New);
        enter(&mut s, "778899");
        enter(&mut s, "998877");
        assert!(s.mode == Mode::New);
        enter(&mut s, "778899");
        enter(&mut s, "778899");
        assert!(s.enabled());
        s.lock();
        hold(&mut s);
        enter(&mut s, "123456");
        assert!(s.locked);
        enter(&mut s, "778899");
        assert!(!s.locked);
        s.setup(true);
        enter(&mut s, "123456");
        assert!(s.enabled());
        enter(&mut s, "778899");
        assert!(!s.enabled());
        assert!(!ScreenLock::load(Some(temp.file())).unwrap().enabled());
    }

    #[test]
    fn failed_attempts_and_cooldown_survive_process_restart() {
        let temp = Temp::new();
        let mut s = seeded(&temp);
        hold(&mut s);
        for _ in 0..5 {
            enter(&mut s, "654321");
            assert!(s.locked);
        }
        assert!(s.remaining() > 0);
        assert_eq!(s.stored.attempts, 5);
        let mut s = ScreenLock::load(Some(temp.file())).unwrap();
        assert!(s.remaining() >= 29);
        hold(&mut s);
        enter(&mut s, "123456");
        assert!(s.locked);
        assert_eq!(s.stored.attempts, 5, "cooldown must reject new guesses");
        s.until = Some(Instant::now() - Duration::from_secs(1));
        enter(&mut s, "123456");
        assert!(!s.locked);
        assert_eq!(s.stored.attempts, 0);
    }

    #[test]
    fn cancellation_and_storage_errors_never_grant_access() {
        let temp = Temp::new();
        let mut s = seeded(&temp);
        hold(&mut s);
        for c in b"123456" {
            s.digit((c - b'0') as i32);
        }
        s.submit();
        assert!(s.busy());
        s.lock();
        s.poll();
        assert!(s.locked);
        assert!(!s.busy());
        assert!(s.input.is_empty());
        let bad = temp.0.join("not-a-directory");
        std::fs::write(&bad, b"blocked").unwrap();
        s.path = Some(bad.join("lock.json"));
        hold(&mut s);
        enter(&mut s, "123456");
        assert!(s.locked);
        assert!(s.message.contains("无法保存"));
        std::fs::write(temp.file(), b"{broken").unwrap();
        assert!(ScreenLock::load(Some(temp.file())).is_err());
    }
}
