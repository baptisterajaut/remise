use crate::bins::find_in;
use std::fs;
use std::path::PathBuf;

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

#[test]
fn first_dir_wins_so_vendor_beats_path() {
    let root = std::env::temp_dir().join(format!("remise-bins-{}", std::process::id()));
    let (vendor, path) = (root.join("vendor/7zip"), root.join("usr/bin"));
    fs::create_dir_all(&vendor).unwrap();
    fs::create_dir_all(&path).unwrap();
    // The system has `7z`, the bundle has `7za`: the bundled one must still win.
    fs::write(path.join(exe("7z")), "").unwrap();
    fs::write(vendor.join(exe("7za")), "").unwrap();
    assert_eq!(find_in(&[vendor.clone(), path], "7z"), Some(vendor.join(exe("7za"))));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ghostscript_has_a_windows_name() {
    let dir = std::env::temp_dir().join(format!("remise-gs-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("gswin64c.exe"), "").unwrap();
    assert_eq!(find_in(std::slice::from_ref(&dir), "gs"), Some(dir.join("gswin64c.exe")));
    assert_eq!(find_in(&[PathBuf::from("/nonexistent")], "gs"), None);
    let _ = fs::remove_dir_all(dir);
}
