use reqwest::{
    Client, Method, Response,
    header::{AUTHORIZATION, HeaderValue},
};
use serde_json::Value;
use std::{net::SocketAddr, time::Duration};
use zeroize::Zeroizing;

pub const MAX_BODY: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Login,
    Unavailable,
    Unsupported,
    LocalOnly,
    InvalidResponse,
    EsimUnavailable,
    EsimReloading,
    EsimBusy,
    EsimNotConfigured,
    Rejected(u16),
    Busy(u64),
}
impl Error {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Login => "需要重新登录后台",
            Self::Unavailable => "后台暂时无法连接",
            Self::Unsupported => "当前后台或固件不支持此功能",
            Self::LocalOnly => "请在设备屏幕上运行此检测",
            Self::InvalidResponse => "后台返回了无法识别的数据",
            Self::EsimUnavailable => "无法访问 eUICC 卡片的管理通道",
            Self::EsimReloading => "SIM 正在重新加载，请稍后刷新",
            Self::EsimBusy => "eSIM 卡片正在处理其他操作，请稍后刷新",
            Self::EsimNotConfigured => "后台的 eSIM 卡槽或通信配置不可用",
            Self::Rejected(_) => "后台拒绝了请求，请检查设置后重试",
            Self::Busy(_) => "后台繁忙，请稍后重试",
        }
    }
}

#[derive(Clone)]
pub struct Api {
    base: String,
    requests: Client,
    streams: Client,
}
impl Api {
    pub fn new(address: SocketAddr) -> anyhow::Result<Self> {
        // Physical UI connects on the device; desktop uses an ADB/SSH tunnel.
        // Never transmit the admin password over an unencrypted LAN connection.
        anyhow::ensure!(
            address.ip().is_loopback() && address.port() != 0,
            "agent must be a loopback address with a port; use an ADB/SSH tunnel"
        );
        let builder = || {
            Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(3))
                .pool_max_idle_per_host(1)
        };
        Ok(Self {
            base: format!("http://{address}"),
            requests: builder().timeout(Duration::from_secs(30)).build()?,
            streams: builder().read_timeout(Duration::from_secs(45)).build()?,
        })
    }
    fn authorized(
        &self,
        client: &Client,
        method: Method,
        path: &str,
        token: &str,
    ) -> Result<reqwest::RequestBuilder, Error> {
        if !path.starts_with("/api/") || path.contains(['\r', '\n', '#']) {
            return Err(Error::InvalidResponse);
        }
        let mut header =
            HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| Error::Login)?;
        header.set_sensitive(true);
        Ok(client
            .request(method, format!("{}{path}", self.base))
            .header(AUTHORIZATION, header))
    }
    pub async fn login(&self, password: Zeroizing<String>) -> Result<Zeroizing<String>, Error> {
        let reply = self
            .requests
            .post(format!("{}/api/auth/login", self.base))
            .json(&serde_json::json!({"password": password.as_str()}))
            .send()
            .await
            .map_err(|_| Error::Unavailable)?;
        let mut data = data(reply, "/api/auth/login").await?;
        let token = data["token"].take().as_str().unwrap_or("").to_owned();
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::InvalidResponse);
        }
        Ok(Zeroizing::new(token))
    }
    pub async fn call(
        &self,
        token: &str,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, Error> {
        let request = self.authorized(&self.requests, method, path, token)?;
        let timeout = match path {
            "/api/update/check" | "/api/update/download" => 360,
            "/api/tailscale/setup" | "/api/tailscale/login-url" => 65,
            "/api/at/send" => 35,
            _ => 30,
        };
        let request = request.timeout(Duration::from_secs(timeout));
        let request = if let Some(body) = body {
            request.json(&body)
        } else {
            request
        };
        data(request.send().await.map_err(|_| Error::Unavailable)?, path).await
    }
    pub async fn events(&self, token: &str, topics: &[String]) -> Result<Response, Error> {
        if topics.is_empty()
            || topics
                .iter()
                .any(|t| !t.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
        {
            return Err(Error::InvalidResponse);
        }
        let response = self
            .authorized(
                &self.streams,
                Method::GET,
                &format!("/api/events?topics={}&signal_ms=2000", topics.join(",")),
                token,
            )?
            .header("Accept", "text/event-stream")
            .send()
            .await
            .map_err(|_| Error::Unavailable)?;
        status(&response)?;
        if !response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| h.split(';').next() == Some("text/event-stream"))
        {
            return Err(Error::InvalidResponse);
        }
        Ok(response)
    }
}
fn status(response: &Response) -> Result<(), Error> {
    match response.status().as_u16() {
        200..=299 => Ok(()),
        401 => Err(Error::Login),
        403 | 404 => Err(Error::Unsupported),
        429 | 503 => Err(Error::Busy(
            response
                .headers()
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(3)
                .clamp(1, 300),
        )),
        code => Err(Error::Rejected(code)),
    }
}
fn esim_error(envelope: &Value) -> Option<Error> {
    let message = envelope["error"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase();
    match envelope["kind"].as_str() {
        Some("sim_reloading") => Some(Error::EsimReloading),
        Some("management_unavailable") => Some(Error::EsimUnavailable),
        Some("not_configured") => Some(Error::EsimNotConfigured),
        Some("busy") => Some(Error::EsimBusy),
        _ if message.contains("sim is still reloading") => Some(Error::EsimReloading),
        _ if message.contains("euicc_init")
            || message.contains("open logical channel")
            || message.contains("simfilenotfound") =>
        {
            Some(Error::EsimUnavailable)
        }
        _ if message.contains("esim manager busy") => Some(Error::EsimBusy),
        _ => None,
    }
}
async fn data(mut response: Response, path: &str) -> Result<Value, Error> {
    let status_result = status(&response);
    let code = response.status().as_u16();
    // Translate only known capability/card failures; raw backend errors can
    // contain credentials or profile identifiers and never reach the screen.
    let inspect_error =
        code == 503 || (path.starts_with("/api/esim/") && matches!(code, 409 | 500 | 502));
    if !inspect_error {
        status_result.clone()?;
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Error::Unavailable)? {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(Error::InvalidResponse);
        }
        body.extend_from_slice(&chunk);
    }
    if inspect_error {
        let envelope = serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null);
        if path.starts_with("/api/esim/")
            && let Some(error) = esim_error(&envelope)
        {
            return Err(error);
        }
        let message = envelope["error"]
            .as_str()
            .unwrap_or("")
            .to_ascii_lowercase();
        if code == 503
            && (message.contains("method not found") || message.contains("ubus status 3"))
        {
            return Err(Error::Unsupported);
        }
        status_result?;
    }
    let mut envelope: Value = serde_json::from_slice(&body).map_err(|_| Error::InvalidResponse)?;
    if envelope["ok"] != true {
        return Err(Error::InvalidResponse);
    }
    // Several command endpoints acknowledge with {ok:true} and no data.
    // Readers still validate their required fields before offering edits.
    Ok(envelope["data"].take())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn authenticated_requests_do_not_follow_redirects_or_leak_errors() {
        assert!(Api::new("192.168.0.1:9090".parse().unwrap()).is_err());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let api = Api::new(listener.local_addr().unwrap()).unwrap();
        let server = std::thread::spawn(move || {
            for (code, body) in [
                (200, r#"{"ok":true,"data":{"capacity":73}}"#),
                (401, "private response"),
                (302, "private response"),
                (200, r#"{"ok":true}"#),
                (
                    503,
                    r#"{"ok":false,"error":"ubus status 3 (method not found)"}"#,
                ),
                (503, r#"{"ok":false,"error":"agent busy, retry"}"#),
                (
                    502,
                    r#"{"ok":false,"kind":"management_unavailable","error":"private card details"}"#,
                ),
                (
                    502,
                    r#"{"ok":false,"error":"euicc_init: open logical channel failed: QMI error 80 SimFileNotFound"}"#,
                ),
                (
                    409,
                    r#"{"ok":false,"error":"the SIM is still reloading after the last switch; try again in a moment"}"#,
                ),
                (
                    409,
                    r#"{"ok":false,"error":"eSIM manager busy; retry shortly"}"#,
                ),
                (
                    503,
                    r#"{"ok":false,"kind":"not_configured","error":"private configuration"}"#,
                ),
                (502, r#"{"ok":false,"error":"private unrelated failure"}"#),
                (
                    502,
                    r#"{"ok":false,"error":"euicc_init: private unrelated failure"}"#,
                ),
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = vec![];
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(
                    request
                        .to_lowercase()
                        .contains("authorization: bearer test-token")
                );
                assert!(!request.lines().next().unwrap().contains("test-token"));
                write!(socket, "HTTP/1.1 {code} Test\r\nContent-Length: {}\r\nConnection: close\r\nLocation: http://192.0.2.1/private\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                assert_eq!(
                    api.call("test-token", Method::GET, "/api/battery", None)
                        .await
                        .unwrap()["capacity"],
                    73
                );
                assert_eq!(
                    api.call("test-token", Method::GET, "/api/battery", None)
                        .await,
                    Err(Error::Login)
                );
                assert_eq!(
                    api.call("test-token", Method::GET, "/api/battery", None)
                        .await,
                    Err(Error::Rejected(302))
                );
                assert_eq!(
                    api.call("test-token", Method::POST, "/api/dns/cache/clear", None)
                        .await
                        .unwrap(),
                    Value::Null
                );
                assert_eq!(
                    api.call("test-token", Method::GET, "/api/router/qos", None)
                        .await,
                    Err(Error::Unsupported)
                );
                assert_eq!(
                    api.call("test-token", Method::GET, "/api/router/qos", None)
                        .await,
                    Err(Error::Busy(3))
                );
                for (path, error) in [
                    ("/api/esim/profiles", Error::EsimUnavailable),
                    ("/api/esim/chip", Error::EsimUnavailable),
                    ("/api/esim/chip", Error::EsimReloading),
                    ("/api/esim/chip", Error::EsimBusy),
                    ("/api/esim/profiles", Error::EsimNotConfigured),
                    ("/api/esim/profiles", Error::Rejected(502)),
                    ("/api/device", Error::Rejected(502)),
                ] {
                    assert_eq!(
                        api.call("test-token", Method::GET, path, None).await,
                        Err(error)
                    );
                }
            });
        server.join().unwrap();
    }
}
