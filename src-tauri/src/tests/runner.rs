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
