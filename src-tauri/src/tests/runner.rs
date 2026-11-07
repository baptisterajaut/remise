use crate::runner::{parse_duration, unique};
use std::fs;

#[test]
fn reads_ffmpeg_duration_banner() {
    assert_eq!(parse_duration("  Duration: 00:01:02.50, start: 0.000000, bitrate: 2 kb/s"), Some(62.5));
    assert_eq!(parse_duration("  Duration: N/A, bitrate: N/A"), None);
    assert_eq!(parse_duration("Stream #0:0: Video: h264"), None);
}

#[test]
fn unique_never_overwrites() {
    let dir = std::env::temp_dir().join(format!("remise-unique-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join("a.compressed.pdf");
    assert_eq!(unique(p.clone()), p);
    fs::write(&p, "").unwrap();
    assert_eq!(unique(p.clone()), dir.join("a.compressed-2.pdf"));
    fs::write(dir.join("a.compressed-2.pdf"), "").unwrap();
    assert_eq!(unique(p), dir.join("a.compressed-3.pdf"));
    let _ = fs::remove_dir_all(dir);
}

fn run(tool: &str, file: &std::path::Path, params: &[(&str, &str)]) -> Result<Vec<String>, String> {
    let params = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let steps = vec![crate::runner::StepReq { tool: tool.into(), params }];
    crate::runner::run_chain(&|_| {}, tool, steps, vec![crate::tools::s(file)], file.parent().unwrap())
}

#[test]
fn checksums_round_trip() {
    let dir = std::env::temp_dir().join(format!("remise-sums-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("hello 50%.txt");
    fs::write(&file, "hello\n").unwrap();
    let out = run("file-checksum", &file, &[]).unwrap();
    let sum = dir.join("hello 50%.txt.sha256");
    assert_eq!(out, [crate::tools::s(&sum)]);
    // What `sha256sum` prints for the same file, byte for byte.
    assert_eq!(fs::read_to_string(&sum).unwrap(), "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03  hello 50%.txt\n");
    assert!(run("file-checksum", &file, &[]).is_err_and(|e| e.contains("already exists")), "never overwritten");
    // Checked from the file or from its list; nothing is written either way.
    for dropped in [&file, &sum] {
        assert_eq!(run("file-checksum", dropped, &[("mode", "check")]), Ok(vec![]));
    }
    fs::write(&file, "hell0\n").unwrap();
    for dropped in [&file, &sum] {
        assert!(run("file-checksum", dropped, &[("mode", "check")]).is_err_and(|e| e.contains("hello 50%.txt")));
    }
    let lone = dir.join("lone.txt");
    fs::write(&lone, "").unwrap();
    assert!(run("file-checksum", &lone, &[("mode", "check")]).is_err_and(|e| e.contains("lone.txt.sha256")));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn reads_checksum_lists() {
    use crate::tools::files::parse_sums;
    let h = "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03";
    let list = std::path::Path::new("/d/app.zip.sha256");
    let text = format!("{h}  a.txt\r\n\n{} *sub/b.bin\n", h.to_uppercase());
    let got = parse_sums(list, &text).unwrap();
    assert_eq!(got[0].1, std::path::Path::new("/d/a.txt"));
    assert_eq!(got[1].1, std::path::Path::new("/d/sub/b.bin"));
    // A bare hash is for the file the list is named after.
    assert_eq!(parse_sums(list, h).unwrap()[0].1, std::path::Path::new("/d/app.zip"));
    assert!(parse_sums(list, "not a hash  a.txt").is_err());
}
