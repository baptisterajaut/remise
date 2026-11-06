//! Finding the CLIs: bundled copies in `vendor/` first, then PATH and the usual install dirs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static VENDOR: OnceLock<Option<PathBuf>> = OnceLock::new();
static USER_TOOLS: OnceLock<Option<PathBuf>> = OnceLock::new();

/// A writable per-user tools dir (app data). Tools that must stay current (yt-dlp) are copied
/// there from the bundle and update themselves; it is searched before the bundle.
pub fn set_user_tools(dir: Option<PathBuf>) {
    let _ = USER_TOOLS.set(std::env::var_os("REMISE_USER_TOOLS").map(PathBuf::from).or(dir));
}

pub fn user_tools() -> Option<&'static Path> {
    USER_TOOLS.get_or_init(|| std::env::var_os("REMISE_USER_TOOLS").map(PathBuf::from)).as_deref()
}

/// Whether `path` is one of the copies shipped inside the app.
pub fn is_bundled(path: &Path) -> bool {
    vendor().is_some_and(|v| path.starts_with(v))
}

/// Called once at startup with the app's bundled `vendor/` dir. `REMISE_VENDOR` wins (tests, dev).
pub fn set_vendor(dir: Option<PathBuf>) {
    let dir = std::env::var_os("REMISE_VENDOR").map(PathBuf::from).or(dir).filter(|d| d.is_dir());
    let _ = VENDOR.set(dir);
}

fn vendor() -> Option<&'static Path> {
    VENDOR.get_or_init(|| std::env::var_os("REMISE_VENDOR").map(PathBuf::from)).as_deref()
}

/// Names a binary goes by across platforms.
fn aliases(bin: &str) -> Vec<&str> {
    match bin {
        // Any Chromium will do for headless printing; Edge ships with every Windows.
        "chromium" => vec![
            "chromium", "chromium-browser", "google-chrome", "google-chrome-stable", "chrome", "msedge",
            "Google Chrome", "Chromium", "Microsoft Edge",
        ],
        "gs" => vec!["gs", "gswin64c", "gswin32c"],
        "7z" => vec!["7z", "7zz", "7za", "7zzs"],
        other => vec![other],
    }
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    // User-updated tools first, then the bundle: <dir>/<tool>/ and <dir>/<tool>/bin/.
    for root in [user_tools(), vendor()].into_iter().flatten() {
        for e in fs::read_dir(root).into_iter().flatten().flatten() {
            dirs.push(e.path().join("bin"));
            dirs.push(e.path());
        }
    }
    dirs.extend(std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default());
    // Apps launched from Finder don't inherit the shell PATH, so Homebrew would be invisible.
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"].map(PathBuf::from));
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/bin"));
    }
    // Browsers live in app bundles on macOS.
    for app in ["Google Chrome", "Chromium", "Microsoft Edge"] {
        dirs.push(PathBuf::from(format!("/Applications/{app}.app/Contents/MacOS")));
    }
    dirs.push(PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS"));
    // Windows installers for these don't touch PATH.
    for pf in ["ProgramFiles", "ProgramFiles(x86)"].iter().filter_map(std::env::var_os) {
        let pf = PathBuf::from(pf);
        dirs.push(pf.join("7-Zip"));
        dirs.push(pf.join("Tesseract-OCR"));
        dirs.push(pf.join("Microsoft/Edge/Application"));
        dirs.push(pf.join("Google/Chrome/Application"));
        dirs.push(pf.join("LibreOffice/program"));
        if let Ok(rd) = fs::read_dir(pf.join("gs")) {
            dirs.extend(rd.flatten().map(|e| e.path().join("bin")));
        }
    }
    dirs
}

/// First directory wins, so a bundled copy beats anything installed.
pub fn which(bin: &str) -> Option<PathBuf> {
    find_in(&search_dirs(), bin)
}

pub fn find_in(dirs: &[PathBuf], bin: &str) -> Option<PathBuf> {
    let names = aliases(bin);
    dirs.iter().find_map(|d| {
        names
            .iter()
            .flat_map(|n| [d.join(n), d.join(format!("{n}.exe"))])
            .find(|c| c.is_file())
    })
}

/// The program to run for `bin` and the arguments to give it.
/// ImageMagick 6 (Ubuntu 22.04/24.04, Debian 12) has no `magick`: `convert` and `montage` instead.
// CBA: IM6's distro policy.xml still forbids PDF writes (Images to PDF); bundling the IM7 AppImage is the upgrade path
pub fn resolve(bin: &str, args: &[String]) -> Option<(PathBuf, Vec<String>)> {
    if let Some(p) = which(bin) {
        return Some((p, args.to_vec()));
    }
    // Never on Windows, where `convert.exe` is the FAT→NTFS disk converter.
    if bin != "magick" || cfg!(windows) {
        return None;
    }
    match args.first().map(String::as_str) {
        Some(sub @ ("identify" | "montage" | "compare" | "composite" | "mogrify")) => which(sub).map(|p| (p, args[1..].to_vec())),
        _ => which("convert").map(|p| (p, args.to_vec())),
    }
}

pub fn available(bin: &str) -> bool {
    resolve(bin, &[]).is_some()
}

/// A command with no console window on Windows and the env the bundled tools need to find
/// their data once relocated.
pub fn command(path: &Path) -> Command {
    let mut c = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    if let Some(v) = vendor() {
        // Tools that call other tools (yt-dlp → ffmpeg, ImageMagick → gs) find the bundled ones.
        let bundled = fs::read_dir(v).into_iter().flatten().flatten().flat_map(|e| [e.path().join("bin"), e.path()]);
        let path = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
        if let Ok(joined) = std::env::join_paths(bundled.chain(path)) {
            c.env("PATH", joined);
        }
        let env = [
            ("TESSDATA_PREFIX", "tesseract/tessdata"),
            ("MAGICK_GHOSTSCRIPT_PATH", "ghostscript/bin"),
        ];
        for (key, sub) in env {
            if v.join(sub).is_dir() {
                c.env(key, v.join(sub));
            }
        }
    }
    c
}
