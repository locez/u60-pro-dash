pub mod clients;
pub mod connectivity;
mod http;
pub mod model;
pub mod network;
mod sse;
pub use http::Error;

use reqwest::Method;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};
use tokio::{sync::mpsc as channel, task::JoinHandle};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Config {
    pub address: SocketAddr,
    pub session_file: Option<PathBuf>,
    pub local_credentials: bool,
}
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Status {
    #[default]
    Connecting,
    Live,
    Reconnecting,
    Sleeping,
    Login,
    Unsupported,
    Paused,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connecting => "连接中",
            Self::Live => "实时",
            Self::Reconnecting => "连接中断",
            Self::Sleeping => "已休眠",
            Self::Login => "请登录",
            Self::Unsupported => "版本不支持",
            Self::Paused => "已暂停",
        }
    }
}
#[derive(Clone)]
pub struct Sample {
    pub data: Value,
    pub age_ms: u64,
    pub failed: bool,
    seq: u64,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub status: Status,
    pub values: BTreeMap<String, Sample>,
    pub revision: u64,
    pub authenticated: bool,
    packet_at: Option<Instant>,
    instance: Option<u64>,
}
impl Snapshot {
    pub fn get(&self, topic: &str) -> Option<&Value> {
        let sample = self.values.get(topic)?;
        // Unchanged sampled values need not be sent again. A healthy stream and
        // a good initial age establish freshness; elapsed receipt time does not.
        let max_age = match topic {
            "traffic" => 2000,
            "signal" | "health" => 4000,
            "battery" => 10_000,
            "clients" => 20_000,
            _ => 60_000,
        };
        (self.status == Status::Live
            && !sample.failed
            && sample.age_ms <= max_age
            && self
                .packet_at
                .is_some_and(|t| t.elapsed() < Duration::from_secs(45)))
        .then_some(&sample.data)
    }
}
type Shared = Arc<Mutex<Snapshot>>;
fn update(shared: &Shared, f: impl FnOnce(&mut Snapshot)) {
    let mut state = shared.lock().unwrap();
    f(&mut state);
    state.revision += 1;
}
fn status(shared: &Shared, status: Status) {
    update(shared, |s| s.status = status);
}

#[derive(Clone, Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub body: Option<Value>,
}
impl Request {
    pub fn get(path: &str) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            body: None,
        }
    }
    pub fn json(method: Method, path: &str, body: Value) -> Self {
        Self {
            method,
            path: path.into(),
            body: Some(body),
        }
    }
}
pub struct Reply {
    pub id: u64,
    pub result: Result<Value, Error>,
}
enum Command {
    Watch(Vec<String>),
    Login(Zeroizing<String>),
    Forget,
    Retry,
    Call(u64, Request),
    Probe(u64, SocketAddr),
}
pub struct Agent {
    commands: Option<channel::Sender<Command>>,
    replies: mpsc::Receiver<Reply>,
    shared: Shared,
    thread: Option<thread::JoinHandle<()>>,
    topics: Vec<String>,
}
impl Agent {
    pub fn start(config: Config) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !config.local_credentials
                || config.address == "127.0.0.1:9090".parse::<SocketAddr>()?,
            "local credentials are restricted to the on-device agent endpoint"
        );
        let api = http::Api::new(config.address)?;
        let (commands, receiver) = channel::channel(16);
        let (reply_tx, replies) = mpsc::sync_channel(16);
        let shared = Shared::default();
        let worker_state = shared.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let thread = thread::Builder::new()
            .name("screen-agent".into())
            .spawn(move || {
                runtime.block_on(worker(config, api, receiver, reply_tx, worker_state));
            })?;
        Ok(Self {
            commands: Some(commands),
            replies,
            shared,
            thread: Some(thread),
            topics: vec![],
        })
    }
    fn send(&self, command: Command) -> Result<(), Error> {
        self.commands
            .as_ref()
            .ok_or(Error::Unavailable)?
            .try_send(command)
            .map_err(|_| Error::Busy(1))
    }
    pub fn watch(&mut self, mut topics: Vec<String>) {
        topics.sort();
        topics.dedup();
        if topics != self.topics && self.send(Command::Watch(topics.clone())).is_ok() {
            self.topics = topics;
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        self.shared.lock().unwrap().clone()
    }
    pub fn replies(&self) -> Vec<Reply> {
        self.replies.try_iter().collect()
    }
    pub fn call(&self, id: u64, request: Request) -> Result<(), Error> {
        self.send(Command::Call(id, request))
    }
    pub fn login(&self, password: Zeroizing<String>) -> Result<(), Error> {
        self.send(Command::Login(password))
    }
    pub fn forget(&self) -> Result<(), Error> {
        self.send(Command::Forget)
    }
    pub fn retry(&self) -> Result<(), Error> {
        self.send(Command::Retry)
    }
    pub fn probe_target(&self, id: u64, address: SocketAddr) -> Result<(), Error> {
        self.send(Command::Probe(id, address))
    }
}
impl Drop for Agent {
    fn drop(&mut self) {
        // Closing this channel wakes select! even while a socket is awaiting a
        // 45-second keepalive. Aborting tasks closes SSE immediately on sleep/exit.
        self.commands.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

enum Job {
    Login(Result<Zeroizing<String>, Error>),
    Call(u64, Result<Value, Error>),
}
enum End {
    Login,
    Paused,
    Standby,
    Unsupported,
}
async fn finished<T>(job: &mut Option<JoinHandle<T>>) -> Result<T, tokio::task::JoinError> {
    match job {
        Some(job) => job.await,
        None => std::future::pending().await,
    }
}
fn abort<T>(job: &mut Option<JoinHandle<T>>) {
    if let Some(job) = job.take() {
        job.abort();
    }
}
fn login_job(api: http::Api, password: Zeroizing<String>) -> JoinHandle<Job> {
    tokio::spawn(async move { Job::Login(api.login(password).await) })
}

async fn worker(
    config: Config,
    api: http::Api,
    mut commands: channel::Receiver<Command>,
    replies: mpsc::SyncSender<Reply>,
    shared: Shared,
) {
    let mut token = load_token(config.session_file.as_deref()).ok().flatten();
    let mut topics = Vec::<String>::new();
    let mut stream = None;
    let mut job = None;
    let mut probing = false;
    let mut blocked = false;
    let mut resume_on_wake = false;
    let mut login_after = Instant::now();
    update(&shared, |s| s.authenticated = token.is_some());
    loop {
        if !topics.is_empty() && !blocked {
            if let Some(value) = &token {
                if stream.is_none() {
                    stream = Some(tokio::spawn(watch(
                        api.clone(),
                        value.clone(),
                        topics.clone(),
                        shared.clone(),
                    )));
                }
            } else if job.is_none() {
                if config.local_credentials && Instant::now() >= login_after {
                    match local_password() {
                        Ok(password) => {
                            status(&shared, Status::Connecting);
                            job = Some(login_job(api.clone(), password));
                        }
                        Err(_) => {
                            blocked = true;
                            status(&shared, Status::Login);
                        }
                    }
                } else {
                    blocked = true;
                    status(&shared, Status::Login);
                }
            }
        }
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { break; };
                match command {
                    Command::Watch(wanted) => {
                        if probing && !wanted.iter().any(|t| t == "clients") {
                            abort(&mut job); probing = false;
                        }
                        abort(&mut stream); topics = wanted;
                        if topics.is_empty() { status(&shared, Status::Sleeping); }
                        else if resume_on_wake { blocked = false; resume_on_wake = false; }
                    }
                    Command::Retry => { abort(&mut stream); blocked = false; }
                    Command::Forget => {
                        abort(&mut stream); abort(&mut job); probing = false; token = None; blocked = true;
                        if let Some(path) = &config.session_file { let _ = fs::remove_file(path); }
                        update(&shared, |s| { s.authenticated = false; s.values.clear(); s.status = Status::Login; });
                    }
                    Command::Login(password) => {
                        if job.is_none() && Instant::now() >= login_after {
                            abort(&mut stream); token = None;
                            update(&shared, |s| { s.authenticated = false; s.values.clear(); s.status = Status::Connecting; });
                            job = Some(login_job(api.clone(), password));
                        }
                    }
                    Command::Call(id, request) => {
                        if let Some(token) = token.as_ref().filter(|_| job.is_none()) {
                            let api = api.clone(); let token = token.clone();
                            job = Some(tokio::spawn(async move { Job::Call(id, api.call(&token, request.method, &request.path, request.body).await) }));
                        } else {
                            let error = if token.is_none() { Error::Login } else { Error::Busy(1) };
                            let _ = replies.try_send(Reply { id, result: Err(error) });
                        }
                    }
                    Command::Probe(id, address) => {
                        if config.local_credentials && token.is_some() && job.is_none() {
                            probing = true;
                            job = Some(tokio::spawn(async move { Job::Call(id, Ok(connectivity::probe(address).await)) }));
                        } else {
                            let error = if !config.local_credentials { Error::LocalOnly } else if token.is_none() { Error::Login } else { Error::Busy(1) };
                            let _ = replies.try_send(Reply { id, result: Err(error) });
                        }
                    }
                }
            }
            result = finished(&mut job) => {
                job = None;
                probing = false;
                match result {
                    Ok(Job::Login(Ok(value))) => {
                        // Persistence failure must not prevent an in-memory login.
                        // Credentials are never written beside public appearance data.
                        let _ = save_token(config.session_file.as_deref(), &value);
                        token = Some(value); blocked = false;
                        update(&shared, |s| s.authenticated = true);
                    }
                    Ok(Job::Login(Err(error))) => {
                        login_after = Instant::now() + Duration::from_secs(match error { Error::Busy(n) => n, _ => 3 });
                        blocked = true; status(&shared, Status::Login);
                    }
                    Ok(Job::Call(id, result)) => {
                        if result == Err(Error::Login) {
                            token = None; abort(&mut stream); blocked = !config.local_credentials;
                            update(&shared, |s| { s.authenticated = false; s.values.clear(); s.status = Status::Login; });
                        }
                        let _ = replies.try_send(Reply { id, result });
                    }
                    Err(_) => status(&shared, Status::Reconnecting),
                }
            }
            result = finished(&mut stream) => {
                stream = None;
                match result {
                    Ok(End::Login) => {
                        token = None; blocked = !config.local_credentials;
                        if let Some(path) = &config.session_file { let _ = fs::remove_file(path); }
                        update(&shared, |s| { s.authenticated = false; s.values.clear(); s.status = Status::Login; });
                    }
                    Ok(End::Unsupported) => { blocked = true; status(&shared, Status::Unsupported); }
                    Ok(End::Standby) => { blocked = true; resume_on_wake = true; status(&shared, Status::Paused); }
                    _ => { blocked = true; status(&shared, Status::Paused); }
                }
            }
        }
    }
    abort(&mut stream);
    abort(&mut job);
}

async fn watch(
    api: http::Api,
    token: Zeroizing<String>,
    topics: Vec<String>,
    shared: Shared,
) -> End {
    let mut backoff = 3;
    loop {
        status(&shared, Status::Connecting);
        let error = match api.events(&token, &topics).await {
            Ok(mut response) => {
                let mut decoder = sse::Decoder::default();
                let mut hello = false;
                let mut lifetime = false;
                update(&shared, |s| s.values.clear());
                'response: loop {
                    let chunk = match response.chunk().await {
                        Ok(Some(c)) => c,
                        _ => break,
                    };
                    update(&shared, |s| s.packet_at = Some(Instant::now()));
                    let frames = match decoder.feed(&chunk) {
                        Ok(f) => f,
                        Err(_) => break,
                    };
                    for frame in frames {
                        let Ok(data) = serde_json::from_str::<Value>(&frame.data) else {
                            break 'response;
                        };
                        match frame.event.as_str() {
                            "hello" => {
                                if data["v"] != 1 || data["agent_instance"].as_u64().is_none() {
                                    return End::Unsupported;
                                }
                                let instance = data["agent_instance"].as_u64();
                                if shared
                                    .lock()
                                    .unwrap()
                                    .instance
                                    .is_some_and(|old| Some(old) != instance)
                                {
                                    update(&shared, |s| s.instance = instance);
                                    return End::Login;
                                }
                                hello = true;
                                backoff = 3;
                                update(&shared, |s| {
                                    s.instance = instance;
                                    s.status = Status::Live;
                                });
                            }
                            "bye" => match data["reason"].as_str() {
                                Some("lifetime") => {
                                    lifetime = true;
                                    break 'response;
                                }
                                Some("standby") => return End::Standby,
                                Some("evicted") => return End::Paused,
                                _ => return End::Login,
                            },
                            name if hello && topics.iter().any(|t| t == name) => {
                                let Some(seq) = data["seq"].as_u64() else {
                                    break 'response;
                                };
                                let age_ms = data["age_ms"].as_u64().unwrap_or(u64::MAX);
                                update(&shared, |s| {
                                    if s.values.get(name).is_none_or(|old| seq > old.seq) {
                                        s.values.insert(
                                            name.into(),
                                            Sample {
                                                data: data["data"].clone(),
                                                age_ms,
                                                failed: !data["error"].is_null()
                                                    || data["data"].is_null(),
                                                seq,
                                            },
                                        );
                                    }
                                });
                            }
                            _ => {}
                        }
                    }
                }
                if lifetime {
                    continue;
                }
                Error::Unavailable
            }
            Err(error) => error,
        };
        let delay = match error {
            Error::Login => return End::Login,
            Error::Unsupported => return End::Unsupported,
            Error::Busy(n) => n,
            _ => backoff,
        };
        status(&shared, Status::Reconnecting);
        tokio::time::sleep(Duration::from_secs(delay)).await;
        backoff = (backoff * 2).min(60);
    }
}

fn load_token(path: Option<&Path>) -> anyhow::Result<Option<Zeroizing<String>>> {
    let Some(path) = path else { return Ok(None) };
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    anyhow::ensure!(
        meta.is_file() && !meta.is_symlink() && meta.len() <= 256,
        "invalid private session file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        anyhow::ensure!(
            meta.permissions().mode() & 0o077 == 0,
            "session file must be private"
        );
    }
    let text = Zeroizing::new(fs::read_to_string(path)?);
    let token: String = serde_json::from_str(&text)?;
    anyhow::ensure!(
        token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid session token"
    );
    Ok(Some(Zeroizing::new(token)))
}
fn save_token(path: Option<&Path>, token: &str) -> anyhow::Result<()> {
    let Some(path) = path else { return Ok(()) };
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("session directory required"))?;
    fs::create_dir_all(parent)?;
    anyhow::ensure!(
        !fs::symlink_metadata(parent)?.is_symlink(),
        "session directory must not be a symlink"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    crate::config::write_json_atomic(path, &token)
}

// Only the root-owned on-device process is a credential provider. This reads
// its existing configured password without copying it to a second config or
// exposing it through arguments/logs. The HTTP login and token rules still apply.
fn provider_executable(exe: &Path, provider: &Path) -> bool {
    exe == provider || fs::canonicalize(provider).is_ok_and(|resolved| resolved == exe)
}
fn local_password() -> anyhow::Result<Zeroizing<String>> {
    for entry in fs::read_dir("/proc")?.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            continue;
        }
        let path = entry.path();
        let Ok(exe) = fs::read_link(path.join("exe")) else {
            continue;
        };
        if ![
            Path::new("/data/zte-agent"),
            Path::new("/data/local/tmp/zte-agent"),
        ]
        .iter()
        .any(|provider| provider_executable(&exe, provider))
        {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if fs::metadata(&path)?.uid() != 0 {
                continue;
            }
        }
        let mut env = Zeroizing::new(Vec::new());
        fs::File::open(path.join("environ"))?
            .take(256 * 1024)
            .read_to_end(&mut env)?;
        for item in env.split(|b| *b == 0) {
            if let Some(password) = item.strip_prefix(b"ZTE_AGENT_PASSWORD=") {
                anyhow::ensure!(
                    !password.is_empty() && password.len() <= 1024,
                    "invalid local credential"
                );
                return Ok(Zeroizing::new(std::str::from_utf8(password)?.to_owned()));
            }
        }
    }
    anyhow::bail!("local agent credentials unavailable")
}

pub fn probe(mut config: Config) -> anyhow::Result<()> {
    // A deployment replaces the agent instance. Verify fresh authentication
    // rather than mistaking its old cached session's 401 for a bad password.
    if config.local_credentials {
        config.session_file = None;
    }
    let mut agent = Agent::start(config)?;
    agent.watch(["signal", "traffic", "battery"].map(String::from).to_vec());
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(15) {
        let state = agent.snapshot();
        if ["signal", "traffic", "battery"]
            .iter()
            .all(|t| state.get(t).is_some())
        {
            println!(
                "agent: authenticated; signal, traffic and battery received; no personal data printed"
            );
            return Ok(());
        }
        if matches!(
            state.status,
            Status::Login | Status::Unsupported | Status::Paused
        ) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!(
        "agent probe did not receive all live topics ({})",
        agent.snapshot().status.label()
    )
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn ota_launcher_link_identifies_only_its_resolved_executable() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let slot = dir.path().join("slot-a");
        std::fs::create_dir(&slot).unwrap();
        let binary = slot.join("zte-agent");
        std::fs::write(&binary, b"test executable").unwrap();
        let launcher = dir.path().join("zte-agent");
        symlink(&binary, &launcher).unwrap();
        assert!(super::provider_executable(&binary, &launcher));
        assert!(!super::provider_executable(
            &dir.path().join("other/zte-agent"),
            &launcher
        ));
        std::fs::remove_file(&launcher).unwrap();
        assert!(!super::provider_executable(&binary, &launcher));
    }

    use super::*;
    use std::io::{Read, Write};

    fn request(socket: &mut std::net::TcpStream) -> (String, Vec<u8>) {
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut header = vec![];
        let mut byte = [0];
        while !header.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
            assert!(header.len() < 8192);
        }
        let header = String::from_utf8(header).unwrap();
        let length = header
            .lines()
            .find_map(|line| {
                line.to_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|s| s.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        let mut body = vec![0; length];
        socket.read_exact(&mut body).unwrap();
        (header, body)
    }
    fn until(mut check: impl FnMut() -> bool) {
        let start = Instant::now();
        while !check() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "worker did not make progress"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn login_stream_sleep_wake_and_drop_release_the_socket_without_waiting_for_keepalive() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (closed_tx, closed_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut login, _) = listener.accept().unwrap();
            let (head, body) = request(&mut login);
            assert!(head.starts_with("POST /api/auth/login "));
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap()["password"],
                "test-only-password"
            );
            let body =
                serde_json::json!({"ok": true, "data":{"token": "e".repeat(64)}}).to_string();
            write!(
                login,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            drop(login);
            for round in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let (head, _) = request(&mut stream);
                assert!(head.starts_with("GET /api/events?topics=battery&signal_ms=2000 "));
                assert!(
                    head.to_lowercase()
                        .contains(&format!("authorization: bearer {}", "e".repeat(64)))
                );
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap();
                let event = format!(
                    "event: hello\ndata: {{\"v\":1,\"agent_instance\":42}}\n\nevent: battery\ndata: {{\"seq\":1,\"age_ms\":0,\"error\":null,\"data\":{{\"capacity\":{}}}}}\n\n",
                    70 + round
                );
                write!(stream, "{:x}\r\n{event}\r\n", event.len()).unwrap();
                assert_eq!(
                    stream.read(&mut [0]).unwrap(),
                    0,
                    "screen must close its stream"
                );
                closed_tx.send(round).unwrap();
            }
        });
        let private = tempfile::tempdir().unwrap();
        let path = private.path().join(".screen-private/session.json");
        let mut agent = Agent::start(Config {
            address,
            session_file: Some(path.clone()),
            local_credentials: false,
        })
        .unwrap();
        agent.watch(vec!["battery".into()]);
        agent
            .login(Zeroizing::new("test-only-password".into()))
            .unwrap();
        until(|| {
            agent
                .snapshot()
                .get("battery")
                .is_some_and(|d| d["capacity"] == 70)
        });
        assert!(load_token(Some(&path)).unwrap().is_some());
        let sleep = Instant::now();
        agent.watch(vec![]);
        assert_eq!(closed_rx.recv_timeout(Duration::from_secs(2)).unwrap(), 0);
        assert!(sleep.elapsed() < Duration::from_secs(2));
        agent.watch(vec!["battery".into()]);
        until(|| {
            agent
                .snapshot()
                .get("battery")
                .is_some_and(|d| d["capacity"] == 71)
        });
        drop(agent);
        assert_eq!(closed_rx.recv_timeout(Duration::from_secs(2)).unwrap(), 1);
        server.join().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
