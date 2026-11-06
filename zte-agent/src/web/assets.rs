//! Embedded static assets (`/ui/a/<name>.<hash>.<ext>`) and the web app
//! manifest. `build.rs` hashes, compresses and lists them; text assets are
//! stored gzip-compressed only and inflated for the rare client that does
//! not accept gzip. Names change with the content, so they are cached forever.

use axum::body::Body;
use axum::http::header::{
    HeaderMap, HeaderValue, CACHE_CONTROL, CONTENT_TYPE, ETAG,
};
use axum::http::{StatusCode, Uri};
use axum::response::Response;

pub use crate::static_assets::Asset;
use crate::static_assets::{serve, none_match, status_only};
#[cfg(test)]
use crate::static_assets::{accepts_gzip, inflate};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// `GET /ui/a/{file}`.
pub async fn asset(uri: Uri, headers: HeaderMap) -> Response {
    match ASSETS.iter().find(|a| a.path == uri.path()) {
        Some(a) => serve(a, &headers),
        None => not_found(),
    }
}

/// `GET /ui/manifest.webmanifest` (generated at build time).
pub async fn manifest(headers: HeaderMap) -> Response {
    let mut resp = if none_match(&headers, MANIFEST_ETAG) {
        status_only(StatusCode::NOT_MODIFIED)
    } else {
        let mut resp = Response::new(Body::from(MANIFEST));
        resp.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/manifest+json"));
        resp
    };
    let h = resp.headers_mut();
    h.insert(ETAG, HeaderValue::from_static(MANIFEST_ETAG));
    h.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    resp
}

fn not_found() -> Response {
    let mut resp = Response::new(Body::from("Not found"));
    *resp.status_mut() = StatusCode::NOT_FOUND;
    resp.headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(*k, HeaderValue::from_static(v));
        }
        h
    }

    #[test]
    fn nine_assets_with_hashed_paths() {
        use sha2::{Digest, Sha256};
        assert_eq!(ASSETS.len(), 9);
        for a in &ASSETS {
            assert!(a.path.starts_with("/ui/a/"), "{}", a.path);
            let hash = a.etag.trim_matches('"');
            assert_eq!(hash.len(), 10);
            assert!(a.path.contains(&format!(".{hash}.")));
            assert_eq!(a.gzip, !a.etag_gz.is_empty());
            // The name carries the hash of the bytes a client ends up with.
            let identity = if a.gzip { inflate(a.body) } else { a.body.to_vec() };
            let sha: String = Sha256::digest(&identity).iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(&sha[..10], hash, "{}", a.path);
        }
        for path in [APP_CSS, THEME_LIGHT_CSS, THEME_DARK_CSS, DATASTAR_JS, ICON_SVG, ICON_180, ICON_192, ICON_512] {
            assert!(ASSETS.iter().any(|a| a.path == path));
        }
        assert!(DATASTAR_JS.starts_with("/ui/a/datastar-1.0.4."));
    }

    /// The theme stylesheets hold each scheme's colours from `app.css`
    /// (Settings › Appearance): the same properties, the dark block's for
    /// Dark and `:root`'s for Light.
    #[test]
    fn theme_sheets_follow_app_css() {
        let sheet = |path: &str| String::from_utf8(inflate(ASSETS.iter().find(|a| a.path == path).unwrap().body)).unwrap();
        let (light, dark) = (sheet(THEME_LIGHT_CSS), sheet(THEME_DARK_CSS));
        assert!(light.starts_with(":root{color-scheme:light;--bg:#f2f2f7;--bar:rgba(242, 242, 247, .86);--card:#fff;"), "{light}");
        assert!(dark.starts_with(":root{color-scheme:dark;--bg:#000;--bar:rgba(0, 0, 0, .8);--card:#1c1c1e;"), "{dark}");
        let names = |css: &str| css.split(';').filter_map(|d| d.split(':').next().filter(|k| k.starts_with("--")).map(str::to_owned)).collect::<Vec<_>>();
        assert_eq!(names(&light), names(&dark));
        assert_eq!(names(&dark).len(), 18);
        assert!(!light.contains("--btn") && !light.contains("font") && light.len() < 1024 && dark.len() < 1024);
    }

    #[test]
    fn embedded_datastar_is_the_pinned_bundle() {
        use sha2::{Digest, Sha256};
        let a = ASSETS.iter().find(|a| a.path == DATASTAR_JS).unwrap();
        let js = inflate(a.body);
        assert_eq!(js.len(), 33_553);
        let sha: String = Sha256::digest(&js).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(sha, "727844adfc825ee651fb93c544a2a739986f9a21820a94524b35f0cac470cf91");
    }

    /// The chunks of a PNG, each checked against its CRC.
    fn png_chunks(png: &[u8]) -> Vec<([u8; 4], &[u8])> {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let (mut at, mut out) = (8, Vec::new());
        while at < png.len() {
            let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let (kind, data) = (&png[at + 4..at + 8], &png[at + 8..at + 8 + len]);
            let mut crc = flate2::Crc::new();
            crc.update(kind);
            crc.update(data);
            assert_eq!(png[at + 8 + len..at + 12 + len], crc.sum().to_be_bytes(), "CRC of {kind:?}");
            out.push((kind.try_into().unwrap(), data));
            at += 12 + len;
        }
        out
    }

    #[test]
    fn icons_are_valid_pngs_of_their_size() {
        use std::io::Read;
        for (path, n) in [(ICON_180, 180u32), (ICON_192, 192), (ICON_512, 512)] {
            let a = ASSETS.iter().find(|a| a.path == path).unwrap();
            let chunks = png_chunks(a.body);
            let kinds: Vec<_> = chunks.iter().map(|(k, _)| k).collect();
            assert_eq!(kinds, [b"IHDR", b"IDAT", b"IEND"]);
            let ihdr = chunks[0].1;
            assert_eq!(ihdr[..4], n.to_be_bytes());
            assert_eq!(ihdr[4..8], n.to_be_bytes());
            assert_eq!(ihdr[8..], [8, 2, 0, 0, 0]);
            // 8-bit RGB rows, each with filter byte 0.
            let mut raw = Vec::new();
            flate2::read::ZlibDecoder::new(chunks[1].1).read_to_end(&mut raw).unwrap();
            let row = 1 + 3 * n as usize;
            assert_eq!(raw.len(), row * n as usize);
            assert!(raw.chunks(row).all(|r| r[0] == 0));
        }
        let svg = inflate(ASSETS.iter().find(|a| a.path == ICON_SVG).unwrap().body);
        let svg = String::from_utf8(svg).unwrap();
        assert!(svg.starts_with("<svg") && svg.matches("<rect").count() == 5, "{svg}");
    }

    #[test]
    fn gzip_negotiation() {
        assert!(accepts_gzip(&headers(&[("accept-encoding", "gzip, deflate, br")])));
        assert!(accepts_gzip(&headers(&[("accept-encoding", "br;q=1.0, GZIP;q=0.5")])));
        assert!(!accepts_gzip(&headers(&[("accept-encoding", "gzip;q=0")])));
        assert!(!accepts_gzip(&headers(&[("accept-encoding", "gzip; q=0.000")])));
        assert!(!accepts_gzip(&headers(&[("accept-encoding", "gzip;Q=0.")])));
        assert!(accepts_gzip(&headers(&[("accept-encoding", "gzip;q=0.001")])));
        assert!(accepts_gzip(&headers(&[("accept-encoding", "gzip;q=1")])));
        assert!(!accepts_gzip(&headers(&[("accept-encoding", "identity")])));
        assert!(!accepts_gzip(&headers(&[("accept-encoding", "x-gzipped")])));
        assert!(!accepts_gzip(&HeaderMap::new()));
        // `*` stands for gzip only when gzip is not named.
        assert!(accepts_gzip(&headers(&[("accept-encoding", "*")])));
        assert!(accepts_gzip(&headers(&[("accept-encoding", "br, *;q=0.1")])));
        assert!(!accepts_gzip(&headers(&[("accept-encoding", "*;q=0")])));
        assert!(!accepts_gzip(&headers(&[("accept-encoding", "gzip;q=0, *")])));
    }

    #[test]
    fn if_none_match_lists_and_weak_tags() {
        let h = headers(&[("if-none-match", r#""aaa", W/"0123456789-gz""#)]);
        assert!(none_match(&h, "\"0123456789-gz\""));
        assert!(!none_match(&h, "\"0123456789\""));
        assert!(none_match(&headers(&[("if-none-match", "*")]), "\"x\""));
        assert!(!none_match(&HeaderMap::new(), "\"x\""));
    }

    #[test]
    fn manifest_is_json_with_resolving_icons() {
        let m: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!((m["start_url"].as_str(), m["display"].as_str()), (Some("/ui/"), Some("standalone")));
        assert_eq!(m["scope"], "/ui/");
        let icons = m["icons"].as_array().unwrap();
        assert_eq!(icons.len(), 4);
        for icon in icons {
            let src = icon["src"].as_str().unwrap();
            assert!(ASSETS.iter().any(|a| a.path == src), "{src}");
        }
        assert_eq!(icons[2]["purpose"], "maskable");
        // The installed app's fallback colour is the light pages' own
        // (Chrome for Android takes the manifest's when a page's
        // theme-color is too bright, as #f2f2f7 is, and then shows its own
        // light bar instead of a dark one over a light page); the splash
        // stays the icon's tile.
        assert_eq!((m["theme_color"].as_str(), m["background_color"].as_str()), (Some("#f2f2f7"), Some("#0f172a")));
        let light = crate::web::render::scheme(crate::web::prefs::Theme::Light, "00ff");
        assert!(light.contains("<meta name=\"theme-color\" content=\"#f2f2f7\">"), "{light}");
    }
}
