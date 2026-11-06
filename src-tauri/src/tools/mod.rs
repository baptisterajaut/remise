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
}

#[derive(Serialize, Debug)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Param {
    Select {
        key: &'static str,
        label: &'static str,
        options: &'static [(&'static str, &'static str)],
        default: &'static str,
    },
    Text {
        key: &'static str,
        label: &'static str,
        default: &'static str,
        placeholder: &'static str,
    },
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

pub struct Tool {
    pub id: &'static str,
    pub family: Family,
    pub name: &'static str,
    pub blurb: &'static str,
    /// Binaries the tool needs on PATH. Empty = built in.
    pub bins: &'static [&'static str],
    /// Lowercase extensions. Empty = anything (files and folders).
    pub accepts: &'static [&'static str],
    /// All dropped files go into one job (merge, join, archive) instead of one job per file.
    pub multi: bool,
    pub params: &'static [Param],
    pub output: Output,
    pub build: fn(&Job) -> Vec<Step>,
}

#[derive(Debug, PartialEq)]
pub enum Step {
    Run(&'static str, Vec<String>),
    /// Write the sorted list of `dir/*.ext` paths, one per line, to `out`.
    ListDir { dir: PathBuf, ext: &'static str, out: PathBuf },
    Write(PathBuf, String),
    Rename(PathBuf, PathBuf),
    /// Start another app and don't wait for it.
    Launch(&'static str, Vec<String>),
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
    /// `<out dir>/<stem><tail>` for printf-style patterns (gs, ffmpeg): `%` in the path is
    /// escaped, `tail` is left alone so it can carry `%03d`.
    fn out_pattern(&self, tail: &str) -> String {
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
const AV: &[&str] = &[
    "mp4", "mov", "mkv", "webm", "avi", "m4v", "mp3", "wav", "flac", "m4a", "aac", "ogg", "opus",
];
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
mod files;
mod image;
mod pdf;
mod video;

/// Every tool, in wall order.
pub fn all() -> impl Iterator<Item = &'static Tool> {
    [pdf::TOOLS, image::TOOLS, video::TOOLS, audio::TOOLS, files::TOOLS].into_iter().flatten()
}

pub fn find(id: &str) -> Option<&'static Tool> {
    all().find(|t| t.id == id)
}
