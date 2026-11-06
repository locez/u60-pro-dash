//! Embed the Vite output only when it matches the checked-out source files.
use super::{gzip_bytes, hex, read, write};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const REBUILD: &str = "Build the browser app first: cd u60-web && npm ci && npm run build";
fn tree(dir: &Path, into: &mut BTreeSet<PathBuf>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in std::fs::read_dir(dir).expect(REBUILD) {
        let entry = entry.expect(REBUILD);
        let kind = entry.file_type().expect(REBUILD);
        assert!(
            !kind.is_symlink(),
            "Web source/output symlinks are not supported"
        );
        if kind.is_dir() {
            tree(&entry.path(), into);
        } else {
            into.insert(entry.path());
        }
    }
}
fn verify(base: &Path, manifest: &Path) -> BTreeSet<PathBuf> {
    println!("cargo:rerun-if-changed={}", manifest.display());
    let body = std::fs::read_to_string(manifest).expect(REBUILD);
    let mut files = BTreeSet::new();
    for line in body.lines() {
        let (expected, relative) = line
            .split_once("  ")
            .expect("Invalid browser build manifest");
        let path = Path::new(relative);
        assert!(
            !path.is_absolute()
                && path
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
            "Invalid browser build path"
        );
        let path = base.join(path);
        assert!(files.insert(path.clone()), "Duplicate browser build path");
        println!("cargo:rerun-if-changed={}", path.display());
        let actual = hex(&Sha256::digest(std::fs::read(&path).expect(REBUILD)));
        assert_eq!(
            expected,
            actual,
            "Stale browser asset: {}. {REBUILD}",
            path.display()
        );
    }
    assert!(!files.is_empty(), "Empty browser build manifest");
    files
}
pub fn build(agent: &Path, out: &Path) {
    use std::fmt::Write;
    let repo = agent.parent().unwrap();
    let web = repo.join("u60-web");
    let dist = web.join("dist");
    let inputs = verify(repo, &dist.join("inputs.sha256"));
    let mut expected = BTreeSet::new();
    tree(&web.join("src"), &mut expected);
    tree(&web.join("scripts"), &mut expected);
    for file in [
        "index.html",
        "package.json",
        "package-lock.json",
        "tsconfig.json",
        "vite.config.ts",
    ] {
        expected.insert(web.join(file));
    }
    expected.insert(repo.join("locales/web.json"));
    for file in [
        "cellular",
        "clients-model",
        "battery-model",
        "routing-model",
        "data-usage-model",
        "dns-model",
        "wireguard-model",
        "actions",
        "advanced",
    ] {
        expected.insert(agent.join(format!("ui/{file}.js")));
    }
    assert_eq!(inputs, expected, "Browser inputs changed. {REBUILD}");
    let assets = verify(&dist, &dist.join("assets.sha256"));
    let mut found = BTreeSet::new();
    tree(&dist, &mut found);
    found.remove(&dist.join("inputs.sha256"));
    found.remove(&dist.join("assets.sha256"));
    assert_eq!(assets, found, "Browser outputs changed. {REBUILD}");
    assert!(
        assets.contains(&dist.join("index.html")),
        "Missing browser entry"
    );
    let mut rs = format!(
        "pub static PRODUCT_ASSETS: [crate::web::assets::Asset; {}] = [\n",
        assets.len()
    );
    for (i, path) in assets.into_iter().enumerate() {
        let relative = path.strip_prefix(&dist).unwrap().to_str().unwrap();
        let mime = match path.extension().and_then(|s| s.to_str()) {
            Some("html") => "text/html; charset=utf-8",
            Some("js") => "text/javascript; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            Some("woff2") => "font/woff2",
            _ => panic!("Unexpected browser output: {relative}"),
        };
        let bytes = read(&path);
        let hash = hex(&Sha256::digest(&bytes));
        let gzip = !relative.ends_with(".woff2");
        let body = if gzip { gzip_bytes(&bytes) } else { bytes };
        let name = format!("product-{i}.bin");
        write(&out.join(&name), &body);
        writeln!(rs, "crate::web::assets::Asset {{path:{:?}, mime:{mime:?}, etag:{:?}, etag_gz:{:?}, body:include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{name}\")), gzip:{gzip}}},",
            format!("/console/{relative}"), format!("\"{hash}\""), format!("\"{hash}-gz\"")).unwrap();
    }
    rs.push_str("];\n");
    write(&out.join("product_assets.rs"), rs.as_bytes());
}
