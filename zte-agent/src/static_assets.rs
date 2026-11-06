//! HTTP delivery for build-time embedded assets, shared by the product console
//! and the retired renderer's regression fixtures.
use axum::body::Body;
use axum::http::StatusCode;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, ETAG, HeaderMap, HeaderValue, IF_NONE_MATCH,
    VARY,
};
use axum::response::Response;

pub struct Asset {
    pub path: &'static str,
    pub mime: &'static str,
    /// Strong ETag of the identity representation.
    pub etag: &'static str,
    /// Strong ETag of the gzip representation (`""` when not gzip-stored).
    pub etag_gz: &'static str,
    pub body: &'static [u8],
    /// `body` is gzip-compressed.
    pub gzip: bool,
}

const IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub(crate) fn serve(a: &Asset, headers: &HeaderMap) -> Response {
    let gzip = a.gzip && accepts_gzip(headers);
    let etag = if gzip { a.etag_gz } else { a.etag };
    let mut resp = if none_match(headers, etag) {
        status_only(StatusCode::NOT_MODIFIED)
    } else {
        let body = if gzip || !a.gzip {
            Body::from(a.body)
        } else {
            Body::from(inflate(a.body))
        };
        let mut resp = Response::new(body);
        resp.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(a.mime));
        if gzip {
            resp.headers_mut()
                .insert(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        }
        resp
    };
    let h = resp.headers_mut();
    h.insert(ETAG, HeaderValue::from_static(etag));
    h.insert(CACHE_CONTROL, HeaderValue::from_static(IMMUTABLE));
    if a.gzip {
        h.insert(VARY, HeaderValue::from_static("Accept-Encoding"));
    }
    resp
}

pub(crate) fn status_only(status: StatusCode) -> Response {
    let mut resp = Response::new(Body::empty());
    *resp.status_mut() = status;
    resp
}

/// An `Accept-Encoding` entry `gzip` whose `q` is not zero, or, when gzip is
/// not named at all, such a `*` entry.
pub(crate) fn accepts_gzip(headers: &HeaderMap) -> bool {
    let (mut gzip, mut star) = (None, false);
    for v in headers
        .get_all("accept-encoding")
        .iter()
        .filter_map(|v| v.to_str().ok())
    {
        for entry in v.split(',') {
            let mut parts = entry.split(';');
            let coding = parts.next().unwrap_or("").trim();
            let ok = !parts.any(|p| is_q_zero(p.trim()));
            if coding.eq_ignore_ascii_case("gzip") {
                gzip = Some(gzip.unwrap_or(false) || ok);
            } else if coding == "*" {
                star |= ok;
            }
        }
    }
    gzip.unwrap_or(star)
}

/// `q=0`, `q=0.` or `q=0.000` (a qvalue has at most three decimals).
fn is_q_zero(p: &str) -> bool {
    let Some(q) = p.strip_prefix("q=").or_else(|| p.strip_prefix("Q=")) else {
        return false;
    };
    let (int, frac) = q.split_once('.').unwrap_or((q, ""));
    int == "0" && frac.bytes().all(|b| b == b'0')
}

/// `If-None-Match` lists `etag` or `*` (weak comparison: `W/` is ignored).
pub(crate) fn none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|v| {
            v.split(',')
                .map(str::trim)
                .any(|t| t == "*" || t.strip_prefix("W/").unwrap_or(t) == etag)
        })
}

/// The original bytes of a build.rs gzip member: a plain 10-byte header (no
/// name or extra fields; build.rs checks), the deflate data, and an 8-byte
/// trailer ending in the original length.
pub(crate) fn inflate(gz: &[u8]) -> Vec<u8> {
    let n = gz.len();
    if n < 18 {
        return Vec::new();
    }
    let mut size = [0u8; 4];
    size.copy_from_slice(&gz[n - 4..]);
    let mut out = Vec::with_capacity(u32::from_le_bytes(size) as usize);
    // The bytes were written by build.rs; a failure would be a build bug.
    let _ = flate2::Decompress::new(false).decompress_vec(
        &gz[10..n - 8],
        &mut out,
        flate2::FlushDecompress::Finish,
    );
    out
}
