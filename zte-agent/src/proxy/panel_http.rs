//! The optional dashboard uses the existing management origin and a scoped
//! session. Its HTTP/WebSocket traffic only reaches the owned loopback API.
use super::*;
use crate::{
    auth::{Kind, SessionCheck, SessionId},
    httpd::{api, guard, Ctx, Peer},
    server,
};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode},
    response::Response,
    routing::{any, post},
    Router,
};
use hyper_util::rt::TokioIo;
use std::collections::HashMap;

const PREFIX: &str = "/proxy-panel/mihomo/";
const API_PREFIX: &str = "/proxy-panel/mihomo/api/";
const MAX_REPLY: usize = 8 * 1024 * 1024;
struct Ticket {
    owner: SessionId,
    until: Instant,
    generation: String,
}
pub(super) struct PanelState {
    tickets: Mutex<HashMap<String, Ticket>>,
    streams: Arc<tokio::sync::Semaphore>,
}
impl Default for PanelState {
    fn default() -> Self {
        Self {
            tickets: Mutex::new(HashMap::new()),
            streams: Arc::new(tokio::sync::Semaphore::new(16)),
        }
    }
}

pub(crate) fn router() -> Router<Ctx> {
    Router::new()
        .route("/api/proxy/mihomo/panel-session", post(session))
        .route("/proxy-panel/mihomo/", any(serve))
        .route("/proxy-panel/mihomo/{*rest}", any(serve))
}
fn json_reply(status: u16, value: Value) -> Response {
    let mut reply = Response::new(Body::from(value.to_string()));
    *reply.status_mut() = StatusCode::from_u16(status).unwrap();
    reply
        .headers_mut()
        .insert("content-type", HeaderValue::from_static("application/json"));
    reply
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    reply
}
fn error(status: u16, message: &str) -> Response {
    json_reply(status, json!({"message":message}))
}
fn browser_allowed(ctx: &Ctx, req: &Request) -> bool {
    let h = req.headers();
    let loopback = req
        .extensions()
        .get::<Peer>()
        .is_some_and(|p| p.0.ip().to_canonical().is_loopback());
    let secure = req
        .extensions()
        .get::<crate::httpd::tls::Secure>()
        .is_some();
    api::header(h, "host").is_some_and(|host| {
        ctx.hosts
            .allows_via(host, loopback, secure, &|| ctx.ts.get())
    }) && !guard::api_mutation_refused(
        server::Method::Post,
        &guard::ReqHeaders {
            host: api::header(h, "host"),
            origin: api::header(h, "origin"),
            referer: api::header(h, "referer"),
            sec_fetch_site: api::header(h, "sec-fetch-site"),
        },
    )
}
fn generation(service: &MihomoService) -> Option<String> {
    let r = service.runtime.lock().unwrap();
    r.child.as_ref().map(|_| r.secret.clone())
}
fn authorize(ctx: &Ctx, token: &str) -> Option<String> {
    let service = ctx.app.proxy.legacy();
    let secret = generation(service)?;
    let mut tickets = service.panel.tickets.lock().unwrap();
    tickets.retain(|_, t| t.until > Instant::now() && t.generation == secret);
    let ticket = tickets.get(token)?;
    matches!(
        ctx.app.auth.touch_session(ticket.owner),
        SessionCheck::Live { .. }
    )
    .then_some(secret)
}
async fn session(State(ctx): State<Ctx>, req: Request) -> Response {
    if !browser_allowed(&ctx, &req) {
        return json_reply(403, json!({"ok":false,"error":"Request origin refused"}));
    }
    let principal = api::header(req.headers(), "authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|t| ctx.app.auth.authenticate(Kind::Bearer, t));
    let Some(principal) = principal else {
        return json_reply(
            401,
            json!({"ok":false,"error":"Agent authentication required"}),
        );
    };
    if !matches!(
        tokio::time::timeout(
            Duration::from_secs(15),
            axum::body::to_bytes(req.into_body(), 1024)
        )
        .await,
        Ok(Ok(_))
    ) {
        return json_reply(413, json!({"ok":false,"error":"Request too large"}));
    }
    let service = ctx.app.proxy.legacy();
    if service.panel_status()["installed"] != true {
        return json_reply(
            400,
            json!({"ok":false,"error":"Install MetaCubeXD before opening its dashboard"}),
        );
    }
    let Some(generation) = generation(service) else {
        return json_reply(
            400,
            json!({"ok":false,"error":"Start this service before opening its dashboard"}),
        );
    };
    let mut random = [0; 32];
    if getrandom::getrandom(&mut random).is_err() {
        return json_reply(
            500,
            json!({"ok":false,"error":"Cannot create dashboard session"}),
        );
    }
    let token = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut tickets = service.panel.tickets.lock().unwrap();
    tickets.retain(|_, t| t.until > Instant::now() && t.owner != principal.id);
    if tickets.len() >= 16 {
        return json_reply(
            429,
            json!({"ok":false,"error":"Too many dashboard sessions"}),
        );
    }
    tickets.insert(
        token.clone(),
        Ticket {
            owner: principal.id,
            until: Instant::now() + Duration::from_secs(3600),
            generation,
        },
    );
    json_reply(
        200,
        json!({"ok":true,"data":{"path":PREFIX,"endpoint":API_PREFIX.trim_end_matches('/'),"token":token}}),
    )
}
fn allowed(method: &Method, path: &str) -> bool {
    if path.len() > 4096
        || path.contains('\\')
        || reqwest::Url::parse(&format!("http://127.0.0.1{path}")).is_err()
    {
        return false;
    }
    if reqwest::Url::parse(&format!("http://127.0.0.1{path}")).is_ok_and(|u| u.path() != path) {
        return false;
    }
    let parts = path.trim_start_matches('/').split('/').collect::<Vec<_>>();
    match (method.as_str(), parts.as_slice()) {
        (
            "GET",
            ["version" | "configs" | "rules" | "connections" | "traffic" | "memory" | "logs"],
        ) => true,
        ("GET", ["proxies" | "group", ..]) => true,
        ("GET" | "PUT", ["providers", "proxies" | "rules", ..]) => true,
        ("PUT", ["proxies", name]) => !name.is_empty(),
        ("DELETE", ["connections"]) | ("DELETE", ["connections", _]) => true,
        ("GET", ["dns", "query"]) => true,
        ("POST", ["cache", "fakeip" | "dns", "flush"]) => true,
        _ => false,
    }
}
async fn serve(State(ctx): State<Ctx>, mut req: Request) -> Response {
    if !browser_allowed(&ctx, &req) {
        return error(403, "Request origin refused");
    }
    let path = req.uri().path().to_owned();
    if let Some(relative) = path.strip_prefix(API_PREFIX) {
        let path = format!("/{relative}");
        if !allowed(req.method(), &path) {
            return error(404, "This operation is managed in the router console");
        }
        let query = url_query(req.uri().query().unwrap_or(""));
        let token = api::header(req.headers(), "authorization")
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::to_owned)
            .or_else(|| {
                query
                    .iter()
                    .find(|(k, _)| k == "token")
                    .map(|(_, v)| v.clone())
            })
            .unwrap_or_default();
        let Some(secret) = authorize(&ctx, &token) else {
            return error(
                401,
                "Dashboard session expired. Reopen it from the router console",
            );
        };
        let mut target = reqwest::Url::parse(&format!("http://127.0.0.1:19092{path}")).unwrap();
        if !query.is_empty() {
            let mut pairs = target.query_pairs_mut();
            for (k, v) in &query {
                if k != "token" {
                    pairs.append_pair(k, v);
                }
            }
        }
        if api::header(req.headers(), "upgrade")
            .is_some_and(|s| s.eq_ignore_ascii_case("websocket"))
        {
            if req.method() != Method::GET
                || !["/traffic", "/memory", "/logs", "/connections"].contains(&path.as_str())
            {
                return error(400, "Unsupported dashboard stream");
            }
            let Ok(permit) = Arc::clone(&ctx.app.proxy.legacy().panel.streams).try_acquire_owned()
            else {
                return error(429, "Too many dashboard streams");
            };
            return websocket(ctx, &mut req, &target, secret, token, permit).await;
        }
        let method = req.method().clone();
        let body = match tokio::time::timeout(
            Duration::from_secs(15),
            axum::body::to_bytes(req.into_body(), 1024 * 1024),
        )
        .await
        {
            Ok(Ok(body)) => body,
            _ => return error(413, "Dashboard request too large or incomplete"),
        };
        let client = match reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(_) => return error(500, "Cannot connect to controller"),
        };
        let mut response = match client
            .request(method, target)
            .bearer_auth(secret)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => return error(502, "Proxy controller unavailable"),
        };
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if bytes.len() + chunk.len() > MAX_REPLY {
                        return error(502, "Controller response exceeds limit");
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(_) => return error(502, "Controller response interrupted"),
            }
        }
        let mut reply = Response::new(Body::from(bytes));
        *reply.status_mut() = StatusCode::from_u16(status).unwrap();
        reply
            .headers_mut()
            .insert("content-type", HeaderValue::from_static("application/json"));
        reply
            .headers_mut()
            .insert("cache-control", HeaderValue::from_static("no-store"));
        return reply;
    }
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return error(405, "Read-only dashboard assets");
    }
    let name = path.strip_prefix(PREFIX).unwrap_or("");
    let name = if name.is_empty() { "index.html" } else { name };
    let service = Arc::clone(ctx.app.proxy.legacy());
    let name = name.to_owned();
    let asset = tokio::task::spawn_blocking(move || service.panel_file(&name)).await;
    let (mime, bytes) = match asset {
        Ok(Ok(a)) => a,
        _ => {
            return error(
                404,
                "Dashboard asset not found; install or update the dashboard",
            )
        }
    };
    let mut reply = Response::new(if req.method() == Method::HEAD {
        Body::empty()
    } else {
        Body::from(bytes)
    });
    reply
        .headers_mut()
        .insert("content-type", HeaderValue::from_str(&mime).unwrap());
    for (key,value) in [("cache-control","no-store"),("referrer-policy","no-referrer"),("x-content-type-options","nosniff"),("content-security-policy","default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self' https: ws: wss:; img-src 'self' data: https:; font-src 'self' data:; worker-src 'self' blob:; base-uri 'self'; frame-ancestors 'none'")] {reply.headers_mut().insert(key,HeaderValue::from_static(value));}
    reply
}
fn url_query(raw: &str) -> Vec<(String, String)> {
    reqwest::Url::parse(&format!("http://localhost/?{raw}"))
        .unwrap()
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

async fn websocket(
    ctx: Ctx,
    req: &mut Request,
    target: &reqwest::Url,
    secret: String,
    token: String,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Response {
    let stream = match tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect("127.0.0.1:19092"),
    )
    .await
    {
        Ok(Ok(s)) => s,
        _ => return error(502, "Proxy controller unavailable"),
    };
    let (mut sender, connection) =
        match hyper::client::conn::http1::handshake(TokioIo::new(stream)).await {
            Ok(c) => c,
            Err(_) => return error(502, "Controller handshake failed"),
        };
    tokio::spawn(async move {
        let _ = connection.with_upgrades().await;
    });
    let uri = format!(
        "{}{}",
        target.path(),
        target.query().map(|q| format!("?{q}")).unwrap_or_default()
    );
    let mut upstream = axum::http::Request::builder()
        .method("GET")
        .uri(uri)
        .header("host", "127.0.0.1:19092")
        .header("authorization", format!("Bearer {secret}"))
        .header("connection", "Upgrade")
        .header("upgrade", "websocket");
    for name in ["sec-websocket-key", "sec-websocket-version"] {
        let Some(value) = req.headers().get(name) else {
            return error(400, "Invalid WebSocket handshake");
        };
        upstream = upstream.header(name, value);
    }
    let mut response = match tokio::time::timeout(
        Duration::from_secs(8),
        sender.send_request(upstream.body(Body::empty()).unwrap()),
    )
    .await
    {
        Ok(Ok(r)) => r,
        _ => return error(502, "Controller WebSocket unavailable"),
    };
    if response.status() != StatusCode::SWITCHING_PROTOCOLS {
        return error(502, "Controller rejected WebSocket");
    }
    let downstream = hyper::upgrade::on(req);
    let native = hyper::upgrade::on(&mut response);
    let mut reply = Response::new(Body::empty());
    *reply.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
    for name in [
        "connection",
        "upgrade",
        "sec-websocket-accept",
        "sec-websocket-protocol",
    ] {
        if let Some(value) = response.headers().get(name) {
            reply.headers_mut().insert(name, value.clone());
        }
    }
    tokio::spawn(async move {
        let _permit = permit;
        let pair = tokio::time::timeout(Duration::from_secs(8), async {
            tokio::try_join!(downstream, native)
        })
        .await;
        let Ok(Ok((downstream, native))) = pair else {
            return;
        };
        let mut a = TokioIo::new(downstream);
        let mut b = TokioIo::new(native);
        let transfer = tokio::io::copy_bidirectional(&mut a, &mut b);
        tokio::pin!(transfer);
        loop {
            tokio::select! {_=&mut transfer=>break,_=tokio::time::sleep(Duration::from_secs(5))=>{if authorize(&ctx,&token).is_none(){break;}}}
        }
    });
    reply
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        ctx: Ctx,
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("u60-panel-session-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("panel")).unwrap();
            fs::write(root.join("panel/index.html"), "<html>fixture</html>").unwrap();
            let mut app = crate::handlers::AppState::new();
            app.auth.set_password("fixture-password");
            app.proxy = Arc::new(ProxyServices::for_test(root.clone()));
            {
                let mut r = app.proxy.legacy().runtime.lock().unwrap();
                r.child = Some(
                    std::process::Command::new("/bin/sleep")
                        .arg("60")
                        .spawn()
                        .unwrap(),
                );
                r.secret = "fixture-controller-credential".into();
            }
            Self {
                ctx: Ctx::new(Arc::new(app), 2),
                root,
            }
        }
        async fn ticket(&self) -> String {
            let login = self
                .ctx
                .app
                .auth
                .login(
                    "fixture-password",
                    Kind::Bearer,
                    crate::auth::PeerKey::UNKNOWN,
                    &[],
                )
                .unwrap();
            let req = Request::builder()
                .method("POST")
                .uri("/api/proxy/mihomo/panel-session")
                .header("host", "127.0.0.1")
                .header("authorization", format!("Bearer {}", login.token.expose()))
                .body(Body::from("{}"))
                .unwrap();
            let reply = session(State(self.ctx.clone()), req).await;
            assert_eq!(reply.status(), StatusCode::OK);
            let body = axum::body::to_bytes(reply.into_body(), 8192).await.unwrap();
            serde_json::from_slice::<Value>(&body).unwrap()["data"]["token"]
                .as_str()
                .unwrap()
                .into()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Some(mut child) = self
                .ctx
                .app
                .proxy
                .legacy()
                .runtime
                .lock()
                .unwrap()
                .child
                .take()
            {
                let _ = child.kill();
                let _ = child.wait();
            }
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[tokio::test]
    async fn native_http_and_websocket_use_scoped_revocable_sessions() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let f = Fixture::new();
        let expired = f.ticket().await;
        assert!(authorize(&f.ctx, &expired).is_some());
        assert!(f
            .ctx
            .app
            .auth
            .authenticate(Kind::Bearer, &expired)
            .is_none());
        f.ctx.app.proxy.legacy().runtime.lock().unwrap().secret = "new-generation".into();
        assert!(authorize(&f.ctx, &expired).is_none());
        f.ctx.app.proxy.legacy().runtime.lock().unwrap().secret =
            "fixture-controller-credential".into();
        let token = f.ticket().await;
        let req = Request::builder()
            .method("GET")
            .uri("/proxy-panel/mihomo/api/version")
            .header("host", "127.0.0.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            serve(State(f.ctx.clone()), req).await.status(),
            StatusCode::UNAUTHORIZED
        );
        let req = Request::builder()
            .method("POST")
            .uri("/api/proxy/mihomo/panel-session")
            .header("host", "127.0.0.1")
            .header("origin", "https://foreign.invalid")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            session(State(f.ctx.clone()), req).await.status(),
            StatusCode::FORBIDDEN
        );

        // Exercise the real HTTP server's Upgrade path, including buffered
        // WebSocket bytes and both traffic directions, without a real engine.
        let listener = std::net::TcpListener::bind("127.0.0.1:19092").unwrap();
        listener.set_nonblocking(true).unwrap();
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        let native = tokio::spawn(async move {
            async fn request(stream: &mut tokio::net::TcpStream) -> String {
                let mut bytes = Vec::new();
                while !bytes.ends_with(b"\r\n\r\n") {
                    bytes.push(stream.read_u8().await.unwrap());
                    assert!(bytes.len() < 8192);
                }
                String::from_utf8(bytes).unwrap()
            }
            let (mut http, _) = listener.accept().await.unwrap();
            let headers = request(&mut http).await;
            assert!(headers.starts_with("GET /version HTTP/1.1"));
            assert!(headers
                .to_lowercase()
                .contains("authorization: bearer fixture-controller-credential"));
            http.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 19\r\nConnection: close\r\n\r\n{\"version\":\"1.0.0\"}").await.unwrap();
            drop(http);
            let (mut ws, _) = listener.accept().await.unwrap();
            let headers = request(&mut ws).await;
            assert!(
                headers.starts_with("GET /logs?level=info HTTP/1.1"),
                "{headers}"
            );
            assert!(!headers.contains("token="));
            assert!(headers
                .to_lowercase()
                .contains("authorization: bearer fixture-controller-credential"));
            ws.write_all(b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n\x81\x02{}").await.unwrap();
            let mut frame = [0; 8];
            ws.read_exact(&mut frame).await.unwrap();
            assert_eq!(frame, [0x81, 0x82, 0, 0, 0, 0, b'o', b'k']);
            let mut byte = [0];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(8), ws.read(&mut byte))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        });
        let addr = crate::httpd::test_start(f.ctx.clone(), 8, Duration::from_secs(15));
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let reply = client
            .get(format!("http://{addr}/proxy-panel/mihomo/api/version"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(reply.status(), StatusCode::OK);
        assert_eq!(reply.json::<Value>().await.unwrap()["version"], "1.0.0");
        let mut ws = tokio::net::TcpStream::connect(addr).await.unwrap();
        use base64::Engine as _;
        let key = base64::engine::general_purpose::STANDARD.encode(b"the sample nonce");
        ws.write_all(format!("GET /proxy-panel/mihomo/api/logs?token={token}&level=info HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").as_bytes()).await.unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            headers.push(ws.read_u8().await.unwrap());
            assert!(headers.len() < 8192);
        }
        assert!(String::from_utf8(headers)
            .unwrap()
            .starts_with("HTTP/1.1 101"));
        let mut frame = [0; 4];
        ws.read_exact(&mut frame).await.unwrap();
        assert_eq!(frame, [0x81, 2, b'{', b'}']);
        ws.write_all(&[0x81, 0x82, 0, 0, 0, 0, b'o', b'k'])
            .await
            .unwrap();
        f.ctx.app.auth.revoke_all();
        assert!(authorize(&f.ctx, &token).is_none());
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(8), ws.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        native.await.unwrap();
    }
    #[test]
    fn bridge_is_scoped_to_controller_operations_and_rejects_path_escape() {
        assert!(allowed(&Method::GET, "/proxies"));
        assert!(allowed(&Method::PUT, "/proxies/Example%20A"));
        assert!(allowed(&Method::DELETE, "/connections/123"));
        for path in [
            "/proxies/../configs",
            "/proxies/%2e%2e/configs",
            "//evil.invalid/configs",
            "/restart",
            "/upgrade",
            "/configs",
        ] {
            assert!(!allowed(&Method::PUT, path), "{path}");
        }
        assert!(!allowed(&Method::PATCH, "/configs"));
        assert!(!allowed(&Method::GET, "/api/device"));
    }
}
