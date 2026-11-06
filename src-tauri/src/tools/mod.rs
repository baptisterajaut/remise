//! The tool registry. A tool is data: what it accepts, which knobs it has, and a pure
//! function turning a job (inputs, params, output path) into a list of steps.
//! Adding a tool = adding one entry to `TOOLS`. Nothing in here touches the filesystem.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    Pdf,
    Image,
    Video,
    Audio,
    Files,
    Create,
    Docs,
}

#[derive(Serialize, Debug)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Param {
    Select { key: &'static str, label: &'static str, options: &'static [(&'static str, &'static str)], default: &'static str },
    Text { key: &'static str, label: &'static str, default: &'static str, placeholder: &'static str },
}

impl Param {
    pub fn key(&self) -> &'static str {
        match self {
            Param::Select { key, .. } | Param::Text { key, .. } => key,
        }
    }
    fn default(&self) -> &'static str {
        match self {
            Param::Select { default, .. } | Param::Text { default, .. } => default,
        }
    }
}

pub enum Output {
    /// One file next to the (first) input: `<stem>.<suffix>.<ext>`.
    File { suffix: &'static str, ext: fn(&Job) -> String },
    /// A folder next to the input: `<stem>.<suffix>/`.
    Dir(&'static str),
    /// Nothing new is written (rename in place, launch another app).
    None,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Inputs {
    /// One job per dropped file.
    One,
    /// All dropped files go into one job (merge, join, archive).
    Many,
    /// A generator: no input, params only; the result lands in Downloads.
    None,
}

pub struct Tool {
    pub id: &'static str,
    pub family: Family,
    pub name: &'static str,
    pub blurb: &'static str,
    /// Binaries the tool needs on PATH. Empty = built in.
    pub bins: &'static [&'static str],
    /// Lowercase extensions. Empty = anything (files and folders).
    pub accepts: &'static [&'static str],
    pub inputs: Inputs,
    pub params: &'static [Param],
    pub output: Output,
    pub build: fn(&Job) -> Vec<Step>,
}

#[derive(Debug)]
pub enum Step {
    Run(&'static str, Vec<String>),
    /// Same, from inside `dir` (tools that write relative paths, like Pandoc's extracted media).
    RunIn(PathBuf, &'static str, Vec<String>),
    Write(PathBuf, String),
    Rename(PathBuf, PathBuf),
    /// Start another app and don't wait for it.
    Launch(&'static str, Vec<String>),
    /// `ffmpeg -i <input>`'s stream summary (its stderr) into a file, for `media::parse`.
    Probe(PathBuf, PathBuf),
    /// Run and save stdout to a file, for a later `Then` to read.
    Capture {
        bin: &'static str,
        args: Vec<String>,
        into: PathBuf,
    },
    /// Decide the next steps from what earlier steps left in `tmp` (page sizes, file lists…).
    Then(fn(&Job) -> Result<Vec<Step>, String>),
    /// Set a file's modification time.
    Touch(PathBuf, std::time::SystemTime),
    /// Stop with a message for the user (wrong mix of inputs and such).
    Fail(String),
    Copy(PathBuf, PathBuf),
    /// Run a step and carry on whatever happens (best-effort updates).
    Try(Box<Step>),
}

pub struct Job {
    pub inputs: Vec<PathBuf>,
    pub out: PathBuf,
    pub tmp: PathBuf,
    pub params: HashMap<String, String>,
}

impl Job {
    pub fn input(&self) -> String {
        s(&self.inputs[0])
    }
    pub fn out(&self) -> String {
        s(&self.out)
    }
    pub fn tmp(&self, name: &str) -> String {
        s(&self.tmp.join(name))
    }
    pub fn p(&self, key: &str) -> &str {
        self.params.get(key).map(String::as_str).unwrap_or("")
    }
    pub fn in_ext(&self) -> String {
        ext_of(&self.inputs[0])
    }
    pub fn stem(&self) -> String {
        stem_of(&self.inputs[0])
    }
    /// First input with one of these extensions.
    pub fn pick(&self, exts: &[&str]) -> Option<String> {
        self.inputs.iter().find(|p| exts.contains(&ext_of(p).as_str())).map(|p| s(p))
    }
    /// Reads a file an earlier `Capture`/tool left in `tmp`.
    pub fn read_tmp(&self, name: &str) -> Result<String, String> {
        std::fs::read_to_string(self.tmp.join(name)).map_err(|e| format!("{name}: {e}"))
    }
    /// `<out dir>/<stem><tail>` for printf-style patterns (gs, ffmpeg): `%` in the path is
    /// escaped, `tail` is left alone so it can carry `%03d`.
    pub fn out_pattern(&self, tail: &str) -> String {
        format!("{}{}", s(&self.out.join(self.stem())).replace('%', "%%"), tail)
    }
}

pub fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
pub fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}
pub fn stem_of(p: &Path) -> String {
    p.file_stem().map(|e| e.to_string_lossy().into_owned()).unwrap_or_else(|| "output".into())
}

macro_rules! args {
    ($($x:expr),* $(,)?) => { vec![$(String::from($x)),*] };
}

/// Fills in defaults for every param the caller did not send.
pub fn with_defaults(tool: &Tool, mut params: HashMap<String, String>) -> HashMap<String, String> {
    for p in tool.params {
        params.entry(p.key().into()).or_insert_with(|| p.default().into());
    }
    params
}

/// Gs treats `%` in output paths as a printf pattern.
fn gs_out(path: &str) -> String {
    format!("-sOutputFile={}", path.replace('%', "%%"))
}

/// Space-separated option tokens from a select value (values are ours, never user text).
fn tokens(v: &str) -> Vec<String> {
    v.split_whitespace().map(String::from).collect()
}

fn ffmpeg(j: &Job, mid: Vec<String>) -> Step {
    let mut a = args!["-hide_banner", "-nostdin", "-y", "-i", j.input()];
    a.extend(mid);
    a.push(j.out());
    Step::Run("ffmpeg", a)
}

/// `magick <input> <mid…> <out>` with an explicit output (folders, patterns).
fn magick_to(j: &Job, mid: Vec<String>, out: String) -> Step {
    let mut a = args![j.input()];
    a.extend(mid);
    a.push(out);
    Step::Run("magick", a)
}

fn magick(j: &Job, mid: Vec<String>) -> Step {
    let mut a = args![j.input()];
    a.extend(mid);
    a.push(j.out());
    Step::Run("magick", a)
}

const PDF: &[&str] = &["pdf"];
const IMG: &[&str] = &["png", "jpg", "jpeg", "webp", "avif", "tif", "tiff", "bmp", "gif", "heic"];
const VID: &[&str] = &["mp4", "mov", "mkv", "webm", "avi", "m4v"];
const AUD: &[&str] = &["mp3", "wav", "flac", "m4a", "aac", "ogg", "opus"];
const AV: &[&str] = &["mp4", "mov", "mkv", "webm", "avi", "m4v", "mp3", "wav", "flac", "m4a", "aac", "ogg", "opus"];
const PDF_IMG: &[&str] = &["pdf", "png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp"];
const ARCHIVES: &[&str] = &["zip", "7z", "rar", "tar", "gz", "tgz", "xz", "bz2", "zst", "iso"];

fn same(j: &Job) -> String {
    j.in_ext()
}
fn fmt(j: &Job) -> String {
    j.p("format").to_string()
}

const TRIM: &[Param] = &[
    Param::Text { key: "start", label: "Start", default: "00:00:00", placeholder: "hh:mm:ss" },
    Param::Text { key: "end", label: "End", default: "00:00:10", placeholder: "hh:mm:ss" },
];

mod audio;
pub mod create;
pub mod docs;
pub mod files;
pub mod image;
pub mod media;
pub mod pdf;
mod video;

/// Every tool, in wall order.
pub fn all() -> impl Iterator<Item = &'static Tool> {
    [pdf::TOOLS, docs::TOOLS, image::TOOLS, video::TOOLS, audio::TOOLS, create::TOOLS, files::TOOLS].into_iter().flatten()
}

pub fn find(id: &str) -> Option<&'static Tool> {
    all().find(|t| t.id == id)
}

// ───────────────────────────── shared helpers for the newer tools ─────────────────────────────

const HTML: &[&str] = &["html", "htm"];
const FONTS: &[&str] = &["ttf", "otf"];

/// Areas given as `x,y,w,h` in % of the page/image from the top-left, `;`-separated.
pub fn parse_areas(spec: &str) -> Result<Vec<[f64; 4]>, String> {
    let bad = || format!("areas look like `x,y,w,h` in % (e.g. 5,5,40,8; 0,90,100,10), got `{spec}`");
    spec.split(';')
        .filter(|a| !a.trim().is_empty())
        .map(|a| {
            let n: Vec<f64> = a.split(',').map(|v| v.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|_| bad())?;
            match n[..] {
                [x, y, w, h] if [x, y, w, h].iter().all(|v| (0.0..=100.0).contains(v)) => Ok([x, y, w, h]),
                _ => Err(bad()),
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|v| if v.is_empty() { Err(bad()) } else { Ok(v) })
}

/// A font file ImageMagick can always open. Homebrew's ImageMagick has no usable default font,
/// so text tools name one; falls back to the name of a font IM knows when none is found.
pub fn system_font() -> String {
    [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "C:\\Windows\\Fonts\\arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
    ]
    .iter()
    .find(|p| Path::new(p).is_file())
    .map_or_else(|| "DejaVu-Sans".to_string(), |p| p.to_string())
}

/// `identify` the first frame's size into tmp/size.txt; read it back with `size_of`.
pub fn measure(j: &Job, image: &str) -> Step {
    Step::Capture { bin: "magick", args: args!["identify", "-format", "%w %h", format!("{image}[0]")], into: j.tmp.join("size.txt") }
}

pub fn size_of(j: &Job) -> Result<(f64, f64), String> {
    let text = j.read_tmp("size.txt")?;
    let mut it = text.split_whitespace().filter_map(|v| v.parse::<f64>().ok());
    it.next().zip(it.next()).ok_or_else(|| format!("unreadable image size `{text}`"))
}

/// File names for a batch landing in one folder: `a/x.pdf` and `b/x.pdf` become `x.pdf`, `x-2.pdf`.
pub fn distinct_names(paths: &[PathBuf]) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    paths
        .iter()
        .map(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
            let n = seen.entry(name.to_lowercase()).or_insert(0);
            *n += 1;
            match *n {
                1 => name,
                k => {
                    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                    format!("{}-{k}{ext}", stem_of(p))
                }
            }
        })
        .collect()
}

/// A PostScript string literal.
pub fn ps_str(s: &str) -> String {
    format!("({})", s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)"))
}

/// Ghostscript pass that draws `ps` on top of every page (EndPage code 0 = showpage; any other
/// code must answer false or gs emits extra blank pages). In `ps`, W and H are the page size
/// in points and N the 1-based page number.
fn gs_overlay(j: &Job, device: &str, extra: &[&str], ps: &str) -> Step {
    let proc = format!(
        "<< /EndPage {{ 1 index 1 add /N exch def exch pop 0 eq {{ gsave currentpagedevice /PageSize get aload pop \
         /H exch def /W exch def {ps} grestore true }} {{ false }} ifelse }} >> setpagedevice"
    );
    let mut a = args![format!("-sDEVICE={device}")];
    a.extend(extra.iter().map(|e| e.to_string()));
    // gs answers 0 even when it drops a page it couldn't draw; make that an error.
    a.extend(args!["-dNOPAUSE", "-dBATCH", "-dQUIET", "-dPDFSTOPONERROR", gs_out(&j.out()), "-c", proc, "-f", j.input()]);
    Step::Run("gs", a)
}

/// Headless Chromium/Chrome/Edge with a throwaway profile (a running browser keeps its own).
fn chromium(j: &Job, input: String, mode: Vec<String>) -> Step {
    let mut a = args![
        "--headless",
        "--disable-gpu",
        "--hide-scrollbars",
        "--no-first-run",
        "--no-default-browser-check",
        format!("--user-data-dir={}", j.tmp("chrome"))
    ];
    a.extend(mode);
    a.push(input);
    Step::Run("chromium", a)
}

/// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`, read as UTC (the caller shifts it to local time).
pub fn parse_date(s: &str) -> Option<std::time::SystemTime> {
    let (date, time) = s.trim().split_once(' ').unwrap_or((s.trim(), "00:00"));
    let d: Vec<i64> = date.split('-').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    let t: Vec<i64> = time.split(':').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    let [y, m, day]: [i64; 3] = d.try_into().ok()?;
    let [hh, mm]: [i64; 2] = t.try_into().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || !(0..24).contains(&hh) || !(0..60).contains(&mm) {
        return None;
    }
    // Days from civil (Howard Hinnant), proleptic Gregorian.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    let secs = u64::try_from(days * 86_400 + hh * 3600 + mm * 60).ok()?;
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

/// The `(file, new name)` pairs a file tool's steps would rename, for a preview before running:
/// the name relative to the file's folder, so a move into a subfolder reads `Images/x.jpg`.
pub fn planned_renames(job: &Job, steps: Vec<Step>) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for step in steps {
        match step {
            Step::Fail(e) => return Err(e),
            Step::Then(plan) => out.extend(planned_renames(job, plan(job)?)?),
            Step::Rename(from, to) => {
                let rel = from.parent().and_then(|dir| to.strip_prefix(dir).ok()).unwrap_or(&to).to_path_buf();
                out.push((from.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), s(&rel)));
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Inputs split by kind for tools that take a mix (a PDF and a signature, a video and a logo…).
fn pair(j: &Job, a: &[&str], b: &[&str], what: &str) -> Result<(String, String), Step> {
    match (j.pick(a), j.pick(b)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(Step::Fail(format!("drop {what}"))),
    }
}
