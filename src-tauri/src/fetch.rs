//! Programs and models too big to put in every installer, downloaded when the user asks for them
//! (the tool's Run button says what and how big). Pinned URL and SHA-256, into the per-user tools
//! folder, unpacked aside and moved in at the end: never half-installed.

use crate::bins;
use crate::runner::{Emit, Progress};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

pub struct Download {
    /// The name tools ask for in `Tool::bins`.
    pub bin: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
    /// The file to use inside the unpacked download (a .zip), or the download's own name.
    pub path: &'static str,
}

/// Headless Chromium (Chrome for Testing) for HTML → PDF/image where no browser is installed.
/// Windows always has Edge.
#[cfg(any(target_os = "linux", target_os = "macos"))]
macro_rules! chrome {
    ($asset:literal) => {
        concat!("https://storage.googleapis.com/chrome-for-testing-public/157.0.8103.38/", $asset)
    };
}
#[cfg(target_os = "linux")]
const BROWSER: &[Download] = &[Download {
    bin: "chromium",
    name: "Chromium",
    url: chrome!("linux64/chrome-headless-shell-linux64.zip"),
    sha256: "deafffa74660b34853a4119c361fd427ac8c9fcb62db51ddb8a797cd83a4ba2d",
    bytes: 128_168_542,
    path: "chrome-headless-shell-linux64/chrome-headless-shell",
}];
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const BROWSER: &[Download] = &[Download {
    bin: "chromium",
    name: "Chromium",
    url: chrome!("mac-arm64/chrome-headless-shell-mac-arm64.zip"),
    sha256: "e6a6b13551aa12f951d0891d79282606b13742837c8fa0505e1272bae116ce82",
    bytes: 104_773_711,
    path: "chrome-headless-shell-mac-arm64/chrome-headless-shell",
}];
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const BROWSER: &[Download] = &[Download {
    bin: "chromium",
    name: "Chromium",
    url: chrome!("mac-x64/chrome-headless-shell-mac-x64.zip"),
    sha256: "79821745d0a72d2ba752d981284ed0a9e6fa320dad133596941a17dba6186de3",
    bytes: 110_442_262,
    path: "chrome-headless-shell-mac-x64/chrome-headless-shell",
}];
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const BROWSER: &[Download] = &[];

/// PdfCraft (MIT/Apache-2.0), the editor PDF Studio opens: portable builds only.
macro_rules! pdfcraft {
    ($asset:literal) => {
        concat!("https://github.com/storytold/pdfcraft/releases/download/v0.19.0/", $asset)
    };
}
#[cfg(target_os = "linux")]
const EDITOR: &[Download] = &[Download {
    bin: "pdfcraft",
    name: "PdfCraft",
    url: pdfcraft!("pdfcraft-0.19.0-linux-x86_64.tar.gz"),
    sha256: "3657aa40fe146d4af40fab0507c97f6ee2c6ce5ba9938567194439be3335ba8b",
    bytes: 114_926_704,
    path: "pdfcraft-0.19.0-linux-x86_64/bin/pdfcraft",
}];
#[cfg(windows)]
const EDITOR: &[Download] = &[Download {
    bin: "pdfcraft",
    name: "PdfCraft",
    url: pdfcraft!("pdfcraft-0.19.0-windows-x64-portable.zip"),
    sha256: "73efe9ea0f4ff81bf3b7708ae9779e021db9b9acfa4962b7dc6444c2fef08a90",
    bytes: 116_309_798,
    path: "pdfcraft-0.19.0-windows-x64-portable/pdfcraft.exe",
}];
#[cfg(target_os = "macos")]
const EDITOR: &[Download] = &[Download {
    bin: "pdfcraft",
    name: "PdfCraft",
    url: pdfcraft!("pdfcraft-0.19.0-macos-universal.dmg"),
    sha256: "8b41d907b1a030b4ba8df03b5eb969f7090f8f41ece23bcb799077f208a10c96",
    bytes: 129_238_239,
    path: "PdfCraft.app/Contents/MacOS/PdfCraft",
}];
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
const EDITOR: &[Download] = &[];

/// LibreOffice for Office → PDF, the only faithful renderer of Word/Excel/PowerPoint layouts.
/// The Document Foundation's archive keeps every build at a fixed URL; on Linux, the AppImage
/// its download page points to (LibreItalia), unpacked so it needs no FUSE.
#[cfg(any(windows, target_os = "macos"))]
macro_rules! libreoffice {
    ($path:literal) => {
        concat!("https://downloadarchive.documentfoundation.org/libreoffice/old/26.8.2.2/", $path)
    };
}
#[cfg(target_os = "linux")]
const OFFICE: &[Download] = &[Download {
    bin: "soffice",
    name: "LibreOffice",
    url: "https://appimages.libreitalia.org/LibreOffice-26.8.2.2.basic-x86_64.AppImage",
    sha256: "2a5452966b0c2b3125585e3c1be895b511042982daca6253dfd6eb95febf07db",
    bytes: 300_643_520,
    path: "opt/libreoffice26.8/program/soffice",
}];
#[cfg(windows)]
const OFFICE: &[Download] = &[Download {
    bin: "soffice",
    name: "LibreOffice",
    url: libreoffice!("win/x86_64/LibreOffice_26.8.2.2_Win_x86-64.msi"),
    sha256: "ff4b911403b67ee9e6249f820f9782c44bb32c94fdaa45b0da53a8e419e5307a",
    bytes: 375_934_976,
    path: "program/soffice.exe",
}];
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const OFFICE: &[Download] = &[Download {
    bin: "soffice",
    name: "LibreOffice",
    url: libreoffice!("mac/aarch64/LibreOffice_26.8.2.2_MacOS_aarch64.dmg"),
    sha256: "9f24fe83ab1ed5c95eb3df580119f32932f713e577feeac02bf619fb00749b55",
    bytes: 299_236_406,
    path: "LibreOffice.app/Contents/MacOS/soffice",
}];
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const OFFICE: &[Download] = &[Download {
    bin: "soffice",
    name: "LibreOffice",
    url: libreoffice!("mac/x86_64/LibreOffice_26.8.2.2_MacOS_x86-64.dmg"),
    sha256: "0af062779cc17f6f852918b2424c745f7dd2d20d51d7fb8a0925eb92ec0417a7",
    bytes: 309_690_507,
    path: "LibreOffice.app/Contents/MacOS/soffice",
}];
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
const OFFICE: &[Download] = &[];

/// yt-dlp (Unlicense; a PyInstaller build carrying its own Python): 18-41 MB that only Video
/// downloader uses, and it updates itself (`-U`) in place once downloaded. QuickJS stays bundled.
macro_rules! ytdlp {
    ($asset:literal) => {
        concat!("https://github.com/yt-dlp/yt-dlp/releases/download/2026.10.21/", $asset)
    };
}
#[cfg(target_os = "linux")]
const YTDLP: &[Download] = &[Download {
    bin: "yt-dlp",
    name: "yt-dlp",
    url: ytdlp!("yt-dlp_linux"),
    sha256: "d8bd33fca9d5b23bd5b9952b3fca36db5c46331933fda9f598a3da14ce2c243f",
    bytes: 40_569_080,
    path: "yt-dlp",
}];
#[cfg(windows)]
const YTDLP: &[Download] = &[Download {
    bin: "yt-dlp",
    name: "yt-dlp",
    url: ytdlp!("yt-dlp.exe"),
    sha256: "33121d192d706e62f684a06f80918e3f3a8ba52ac3a65213b6951c39c7d9f555",
    bytes: 17_880_111,
    path: "yt-dlp.exe",
}];
// A universal binary: the same file for both Macs.
#[cfg(target_os = "macos")]
const YTDLP: &[Download] = &[Download {
    bin: "yt-dlp",
    name: "yt-dlp",
    url: ytdlp!("yt-dlp_macos"),
    sha256: "7a6fe71d7d680f1fb0ac18d4949873d77db2859e90017c0f44bd94b23be4ee40",
    bytes: 37_180_736,
    path: "yt-dlp",
}];
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
const YTDLP: &[Download] = &[];

/// Whisper "small": far better than "base" outside English, still quick on a laptop CPU. MIT.
/// BiRefNet general lite, as converted to ONNX by rembg: clean edges on hair and products. MIT.
const MODELS: &[Download] = &[
    Download {
        bin: "bg-model",
        name: "the background-removal model",
        url: "https://github.com/danielgatis/rembg/releases/download/v0.0.0/BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx",
        sha256: "5600024376f572a557870a5eb0afb1e5961636bef4e1e22132025467d0f03333",
        bytes: 224_005_088,
        path: "birefnet-general-lite.onnx",
    },
    Download {
        bin: "whisper-model",
        name: "the Whisper speech model",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-small-q5_1.bin",
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
        bytes: 190_085_487,
        path: "ggml-small-q5_1.bin",
    },
];

pub fn find(bin: &str) -> Option<&'static Download> {
    BROWSER.iter().chain(EDITOR).chain(OFFICE).chain(YTDLP).chain(MODELS).find(|d| d.bin == bin)
}

/// The downloaded file for `bin`, if it's there.
pub fn installed(bin: &str) -> Option<PathBuf> {
    let d = find(bin)?;
    Some(program(d, &bins::user_tools()?.join(d.bin))).filter(|p| p.is_file())
}

/// Downloads, checks and unpacks `d`; reports progress as job `dl-<bin>`.
pub fn fetch(emit: Emit, d: &Download) -> Result<PathBuf, String> {
    fetch_into(emit, d, bins::user_tools().ok_or("no folder for downloaded tools")?)
}

pub fn fetch_into(emit: Emit, d: &Download, root: &Path) -> Result<PathBuf, String> {
    // One at a time: a second click must not write into the same .part.
    static BUSY: Mutex<()> = Mutex::new(());
    let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(done) = Some(program(d, &root.join(d.bin))).filter(|p| p.is_file()) {
        return Ok(done);
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    // Ends with the download's own name: msiexec only opens a file named .msi.
    let part = root.join(format!("{}.part-{}", d.bin, d.url.rsplit('/').next().unwrap_or_default()));
    download(emit, d, &part)?;
    let sum = sha256(&part).map_err(|e| e.to_string())?;
    if sum != d.sha256 {
        let _ = fs::remove_file(&part);
        return Err(format!("{} didn't download correctly (checksum mismatch); try again", d.name));
    }
    let staging = root.join(format!("{}.new", d.bin));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    if let Err(e) = unpack(d, &part, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }
    let dest = root.join(d.bin);
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&staging, &dest).map_err(|e| e.to_string())?;
    let exe = program(d, &dest);
    make_executable(&exe);
    Ok(exe)
}

fn download(emit: Emit, d: &Download, part: &Path) -> Result<(), String> {
    let job = format!("dl-{}", d.bin);
    let curl = bins::which("curl").ok_or("curl is missing")?;
    let mut child = bins::command(&curl)
        .args(["-fsSL", "--retry", "3", "--connect-timeout", "30", "-o"])
        .arg(part)
        .arg(d.url)
        .spawn()
        .map_err(|e| e.to_string())?;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        let got = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
        emit(&Progress { job: &job, step: 0, steps: 1, pct: Some((got as f64 / d.bytes as f64).min(1.0)) });
        std::thread::sleep(Duration::from_millis(300));
    };
    if status.success() {
        Ok(())
    } else {
        let _ = fs::remove_file(part);
        Err(format!("couldn't download {} (are you online?)", d.name))
    }
}

fn sha256(path: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    io::copy(&mut fs::File::open(path)?, &mut hasher)?;
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// By the download's extension: the system `tar` for tar.gz, an administrative install (which
/// only unpacks: no rights needed, nothing registered) for msi, `hdiutil` + `ditto` for a dmg on
/// macOS, 7-Zip for zip, AppImage (SquashFS) and other dmg; anything else is the file itself.
fn unpack(d: &Download, part: &Path, into: &Path) -> Result<(), String> {
    let name = d.url.rsplit('/').next().unwrap_or_default();
    let (src, dest) = (part.display().to_string(), into.display().to_string());
    let result = if name.ends_with(".tar.gz") {
        run("tar", &["-xzf", &src, "-C", &dest], b"")
    } else if name.ends_with(".msi") {
        msi(part, into)
    } else if name.ends_with(".dmg") && cfg!(target_os = "macos") {
        mount_and_copy(part, into)
    } else if [".zip", ".dmg", ".AppImage"].iter().any(|e| name.ends_with(e)) {
        run("7z", &["x", "-y", "-bso0", "-bsp0", &format!("-o{dest}"), &src], b"").or_else(only_links_skipped)
    } else {
        return fs::rename(part, into.join(d.path)).map_err(|e| e.to_string());
    };
    let _ = fs::remove_file(part);
    result.map_err(|e| format!("couldn't unpack {}: {e}", d.name))?;
    locate(d, into)
}

fn run(bin: &str, args: &[&str], input: &[u8]) -> Result<(), String> {
    let path = bins::which(bin).ok_or_else(|| format!("{bin} is missing"))?;
    let mut child = bins::command(&path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input);
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(format!("{bin} ({}): {}", out.status, String::from_utf8_lossy(&out.stderr).trim())) }
}

/// 7-Zip skips symlinks that climb with `..` (LibreOffice's AppImage has `usr/bin/* →
/// ../../opt/…`) and exits with 2. Remise calls the program by its real path, which `locate`
/// then checks, so that alone isn't a failure.
pub fn only_links_skipped(err: String) -> Result<(), String> {
    let errors: Vec<&str> = err.lines().filter(|l| l.contains("ERROR")).collect();
    if !errors.is_empty() && errors.iter().all(|l| l.contains("Dangerous link path was ignored")) { Ok(()) } else { Err(err) }
}

/// An administrative install only unpacks (no rights needed, nothing registered). msiexec wants
/// backslashes only, and says nothing on stderr: its log tells why it failed.
fn msi(part: &Path, into: &Path) -> Result<(), String> {
    let win = |p: &Path| p.display().to_string().replace('/', "\\");
    let log = into.with_extension("msi.log");
    let target = format!("TARGETDIR={}", win(into));
    let res = run("msiexec", &["/a", &win(part), "/qn", "/norestart", &target, "/l*v", &win(&log)], b"");
    let why = res.as_ref().err().map(|e| {
        // The log is UTF-16 on Windows; keep the lines that name the error.
        let raw = fs::read(&log).unwrap_or_default();
        let text: String = if raw.starts_with(&[0xFF, 0xFE]) {
            char::decode_utf16(raw[2..].as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b))).map(|c| c.unwrap_or('?')).collect()
        } else {
            String::from_utf8_lossy(&raw).into_owned()
        };
        let errors: Vec<&str> = text.lines().filter(|l| l.contains("Error") || l.contains("error code")).take(4).collect();
        format!("{e} {}", errors.join(" / "))
    });
    let _ = fs::remove_file(&log);
    why.map_or(Ok(()), Err)
}

/// Mount, copy each .app with `ditto` (it keeps links, attributes and the signature), unmount.
fn mount_and_copy(dmg: &Path, into: &Path) -> Result<(), String> {
    let mnt = into.with_extension("mnt");
    let (image, at) = (dmg.display().to_string(), mnt.display().to_string());
    // The SHA-256 is already checked; "Y" answers a licence prompt if the image has one.
    run("hdiutil", &["attach", "-nobrowse", "-readonly", "-noautoopen", "-noverify", "-mountpoint", &at, &image], b"Y\n")?;
    let apps: Vec<_> = fs::read_dir(&mnt).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| tools_ext(p) == "app").collect();
    let copied = apps.iter().try_for_each(|app| {
        let to = into.join(app.file_name().unwrap_or_default());
        run("ditto", &[&app.display().to_string(), &to.display().to_string()], b"")
    });
    let _ = run("hdiutil", &["detach", &at, "-force"], b"");
    let _ = fs::remove_dir(&mnt);
    if apps.is_empty() { Err("no app in the disk image".into()) } else { copied }
}

fn tools_ext(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Where an installer put the program can vary (an msi's folder layout): if it isn't at `path`,
/// look for a file ending with the same last two components and note where, for `installed`.
fn locate(d: &Download, dir: &Path) -> Result<(), String> {
    if dir.join(d.path).is_file() {
        return Ok(());
    }
    let tail: Vec<&str> = d.path.rsplit('/').take(2).collect();
    let found = find_file(dir, &|p| {
        let names: Vec<String> = p.iter().rev().take(2).map(|c| c.to_string_lossy().into_owned()).collect();
        names == tail
    })
    .ok_or_else(|| format!("{} unpacked, but {} isn't in it", d.name, d.path))?;
    let rel = found.strip_prefix(dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(MOVED), rel.to_string_lossy().as_bytes()).map_err(|e| e.to_string())
}

/// Records where `locate` found the program.
const MOVED: &str = ".remise-path";

fn find_file(dir: &Path, wanted: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    for e in fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        let found = if p.is_dir() { find_file(&p, wanted) } else { Some(p.clone()).filter(|p| wanted(p)) };
        if found.is_some() {
            return found;
        }
    }
    None
}

/// The program inside an installed download.
fn program(d: &Download, dir: &Path) -> PathBuf {
    match fs::read_to_string(dir.join(MOVED)) {
        Ok(rel) => dir.join(rel.trim()),
        Err(_) => dir.join(d.path),
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn make_executable(_: &Path) {}
