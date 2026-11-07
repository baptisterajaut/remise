//! The downloader against `file://` URLs (curl reads those too): no network needed.

use crate::fetch::{Download, fetch_into};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn file_url(p: &Path) -> &'static str {
    let p = p.to_string_lossy().replace('\\', "/");
    leak(if p.starts_with('/') { format!("file://{p}") } else { format!("file:///{p}") })
}

fn model(dir: &Path, body: &[u8], sha: Option<&str>) -> Download {
    let src = dir.join("model.bin");
    fs::write(&src, body).unwrap();
    let real = Sha256::digest(body).iter().map(|b| format!("{b:02x}")).collect::<String>();
    Download {
        bin: "test-model",
        dir: "test-model",
        name: "the test model",
        url: file_url(&src),
        sha256: leak(sha.map(String::from).unwrap_or(real)),
        bytes: body.len() as u64,
        path: "model.bin",
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("remise-fetch-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_good_download_lands_whole_and_leaves_nothing_behind() {
    let dir = scratch("good");
    let root = dir.join("tools");
    let d = model(&dir, b"weights", None);
    let seen = std::cell::Cell::new(0);
    let got = fetch_into(&|_| seen.set(seen.get() + 1), &d, &root).unwrap();
    assert_eq!(got, root.join("test-model").join("model.bin"));
    assert_eq!(fs::read(&got).unwrap(), b"weights");
    let left: Vec<_> = fs::read_dir(&root).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(left, ["test-model"], "no .part or .new left over");
    // Already there: no second download.
    fs::remove_file(dir.join("model.bin")).unwrap();
    assert_eq!(fetch_into(&|_| {}, &d, &root).unwrap(), got);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_wrong_checksum_installs_nothing() {
    let dir = scratch("bad");
    let root = dir.join("tools");
    let d = model(&dir, b"tampered", Some(&"0".repeat(64)));
    let err = fetch_into(&|_| {}, &d, &root).unwrap_err();
    assert!(err.contains("checksum"), "{err}");
    assert!(!root.join("test-model").exists());
    assert!(!root.join("test-model.part").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_download_says_so() {
    let dir = scratch("gone");
    let mut d = model(&dir, b"x", None);
    d.url = file_url(&dir.join("nowhere.bin"));
    let err = fetch_into(&|_| {}, &d, &dir.join("tools")).unwrap_err();
    assert!(err.contains("couldn't download"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_archive_is_unpacked_and_its_program_found() {
    let dir = scratch("tgz");
    fs::create_dir_all(dir.join("pkg/bin")).unwrap();
    fs::write(dir.join("pkg/bin/tool"), "#!/bin/sh\n").unwrap();
    let ok = std::process::Command::new("tar").current_dir(&dir).args(["-czf", "pkg.tar.gz", "pkg"]).status().unwrap();
    assert!(ok.success());
    let body = fs::read(dir.join("pkg.tar.gz")).unwrap();
    let d = Download {
        bin: "test-tool",
        dir: "test-tool",
        name: "the test tool",
        url: file_url(&dir.join("pkg.tar.gz")),
        sha256: leak(Sha256::digest(&body).iter().map(|b| format!("{b:02x}")).collect()),
        bytes: body.len() as u64,
        path: "pkg/bin/tool",
    };
    let exe = fetch_into(&|_| {}, &d, &dir.join("tools")).unwrap();
    assert_eq!(exe, dir.join("tools/test-tool/pkg/bin/tool"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&exe).unwrap().permissions().mode() & 0o111, 0o111, "made executable");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn skipped_links_alone_are_not_a_failure() {
    use crate::fetch::only_links_skipped;
    let links = "7z (exit status: 2): ERROR: Dangerous link path was ignored : usr/bin/x : ../../opt/x\nERROR: Dangerous link path was ignored : usr/bin/y : ../../opt/y";
    assert!(only_links_skipped(links.into()).is_ok());
    assert!(only_links_skipped("7z (exit status: 2): ERROR: Data Error : opt/x".into()).is_err());
    assert!(only_links_skipped("7z (exit status: 2): disk full".into()).is_err());
}

/// A typo in a pin only shows up as "checksum mismatch" on the user's first download.
#[test]
fn every_pin_is_a_sha256() {
    for bin in ["chromium", "pdfcraft", "pdfcraft-cli", "soffice", "yt-dlp", "bg-model", "whisper-model"] {
        let Some(d) = crate::fetch::find(bin) else { continue };
        assert!(d.sha256.len() == 64 && d.sha256.bytes().all(|b| b.is_ascii_hexdigit()), "{bin}: {}", d.sha256);
        assert!(d.url.starts_with("https://") && d.bytes > 0, "{bin}");
    }
}

/// On Linux and Windows the editor's archive has the command line too: one folder, one download.
#[test]
fn pdfcraft_and_its_command_line_share_a_download_where_they_can() {
    let (Some(editor), Some(cli)) = (crate::fetch::find("pdfcraft"), crate::fetch::find("pdfcraft-cli")) else { return };
    assert_eq!(editor.url == cli.url, editor.dir == cli.dir);
    assert_eq!(editor.url == cli.url, !cfg!(target_os = "macos"));
}
