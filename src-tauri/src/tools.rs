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

pub static TOOLS: &[Tool] = &[
    // ───────────────────────────── PDF ─────────────────────────────
    Tool {
        id: "pdf-compress",
        family: Family::Pdf,
        name: "Compress PDF",
        blurb: "Shrink scans and exports by re-encoding images.",
        bins: &["gs"],
        accepts: PDF,
        multi: false,
        params: &[Param::Select {
            key: "level",
            label: "Strength",
            options: &[("printer", "Light"), ("ebook", "Balanced"), ("screen", "Strong")],
            default: "ebook",
        }],
        output: Output::File { suffix: "compressed", ext: |_| "pdf".into() },
        build: |j| {
            vec![Step::Run(
                "gs",
                args![
                    "-sDEVICE=pdfwrite",
                    "-dCompatibilityLevel=1.5",
                    format!("-dPDFSETTINGS=/{}", j.p("level")),
                    "-dDetectDuplicateImages=true",
                    "-dNOPAUSE",
                    "-dBATCH",
                    "-dQUIET",
                    gs_out(&j.out()),
                    j.input(),
                ],
            )]
        },
    },
    Tool {
        id: "pdf-merge",
        family: Family::Pdf,
        name: "Merge PDFs",
        blurb: "Stitch files together, in the order you dropped them.",
        bins: &["qpdf"],
        accepts: PDF,
        multi: true,
        params: &[],
        output: Output::File { suffix: "merged", ext: |_| "pdf".into() },
        build: |j| {
            let mut a = args!["--empty", "--pages"];
            a.extend(j.inputs.iter().map(|p| s(p)));
            a.extend(args!["--", j.out()]);
            vec![Step::Run("qpdf", a)]
        },
    },
    Tool {
        id: "pdf-split",
        family: Family::Pdf,
        name: "Split PDF",
        blurb: "One file every N pages.",
        bins: &["qpdf"],
        accepts: PDF,
        multi: false,
        params: &[Param::Select {
            key: "every",
            label: "Pages per file",
            options: &[("1", "1"), ("2", "2"), ("5", "5"), ("10", "10")],
            default: "1",
        }],
        output: Output::Dir("split"),
        build: |j| {
            vec![Step::Run(
                "qpdf",
                // qpdf has no %% escape: it substitutes the first "%d" it finds.
                args![
                    format!("--split-pages={}", j.p("every")),
                    j.input(),
                    format!("{}-%d.pdf", s(&j.out.join(j.stem())))
                ],
            )]
        },
    },
    Tool {
        id: "pdf-extract",
        family: Family::Pdf,
        name: "Extract pages",
        blurb: "Keep only the pages you list.",
        bins: &["qpdf"],
        accepts: PDF,
        multi: false,
        params: &[Param::Text { key: "pages", label: "Pages", default: "1-3", placeholder: "1-3,5,8-z" }],
        output: Output::File { suffix: "pages", ext: |_| "pdf".into() },
        build: |j| vec![Step::Run("qpdf", args![j.input(), "--pages", ".", j.p("pages"), "--", j.out()])],
    },
    Tool {
        id: "pdf-rotate",
        family: Family::Pdf,
        name: "Rotate pages",
        blurb: "Fix sideways scans, all pages or a range.",
        bins: &["qpdf"],
        accepts: PDF,
        multi: false,
        params: &[
            Param::Select {
                key: "angle",
                label: "Rotation",
                options: &[("+90", "90° right"), ("-90", "90° left"), ("+180", "180°")],
                default: "+90",
            },
            Param::Text { key: "pages", label: "Pages", default: "1-z", placeholder: "1-z = all" },
        ],
        output: Output::File { suffix: "rotated", ext: |_| "pdf".into() },
        build: |j| {
            vec![Step::Run(
                "qpdf",
                args![j.input(), j.out(), format!("--rotate={}:{}", j.p("angle"), j.p("pages"))],
            )]
        },
    },
    Tool {
        id: "pdf-ocr",
        family: Family::Pdf,
        name: "OCR",
        blurb: "Turn scans and photos into a searchable PDF.",
        bins: &["gs", "tesseract"],
        accepts: PDF_IMG,
        multi: false,
        params: &[Param::Text { key: "lang", label: "Languages", default: "eng", placeholder: "eng+fra" }],
        output: Output::File { suffix: "ocr", ext: |_| "pdf".into() },
        build: |j| {
            // Tesseract appends ".pdf" to the output base itself.
            let base = s(&j.out.with_extension(""));
            if j.in_ext() != "pdf" {
                return vec![Step::Run("tesseract", args![j.input(), base, "-l", j.p("lang"), "pdf"])];
            }
            let list = j.tmp.join("pages.txt");
            vec![
                Step::Run(
                    "gs",
                    args![
                        "-sDEVICE=png16m",
                        "-r300",
                        "-dNOPAUSE",
                        "-dBATCH",
                        "-dQUIET",
                        gs_out(&j.tmp("p-%04d.png")),
                        j.input()
                    ],
                ),
                Step::ListDir { dir: j.tmp.clone(), ext: "png", out: list.clone() },
                Step::Run("tesseract", args![s(&list), base, "-l", j.p("lang"), "pdf"]),
            ]
        },
    },
    Tool {
        id: "pdf-to-images",
        family: Family::Pdf,
        name: "PDF to images",
        blurb: "Every page as a PNG or JPG.",
        bins: &["gs"],
        accepts: PDF,
        multi: false,
        params: &[
            Param::Select {
                key: "format",
                label: "Format",
                options: &[("png16m", "PNG"), ("jpeg", "JPG")],
                default: "png16m",
            },
            Param::Select {
                key: "dpi",
                label: "Resolution",
                options: &[("72", "72 dpi"), ("150", "150 dpi"), ("300", "300 dpi")],
                default: "150",
            },
        ],
        output: Output::Dir("pages"),
        build: |j| {
            let ext = if j.p("format") == "jpeg" { "jpg" } else { "png" };
            vec![Step::Run(
                "gs",
                args![
                    format!("-sDEVICE={}", j.p("format")),
                    format!("-r{}", j.p("dpi")),
                    "-dTextAlphaBits=4",
                    "-dGraphicsAlphaBits=4",
                    "-dNOPAUSE",
                    "-dBATCH",
                    "-dQUIET",
                    // out_pattern already escaped the stem; gs_out would double it.
                    format!("-sOutputFile={}", j.out_pattern(&format!("-%03d.{ext}"))),
                    j.input()
                ],
            )]
        },
    },
    Tool {
        id: "images-to-pdf",
        family: Family::Pdf,
        name: "Images to PDF",
        blurb: "One page per image, in drop order.",
        bins: &["magick"],
        accepts: IMG,
        multi: true,
        params: &[],
        output: Output::File { suffix: "images", ext: |_| "pdf".into() },
        build: |j| {
            let mut a: Vec<String> = j.inputs.iter().map(|p| s(p)).collect();
            a.extend(args!["-auto-orient", j.out()]);
            vec![Step::Run("magick", a)]
        },
    },
    Tool {
        id: "pdf-text",
        family: Family::Pdf,
        name: "Extract text",
        blurb: "Plain text out of the PDF's text layer.",
        bins: &["gs"],
        accepts: PDF,
        multi: false,
        params: &[],
        output: Output::File { suffix: "text", ext: |_| "txt".into() },
        build: |j| {
            vec![Step::Run(
                "gs",
                args!["-sDEVICE=txtwrite", "-dNOPAUSE", "-dBATCH", "-dQUIET", gs_out(&j.out()), j.input()],
            )]
        },
    },
    Tool {
        id: "pdf-grayscale",
        family: Family::Pdf,
        name: "Grayscale PDF",
        blurb: "Drop the colour, keep the vectors.",
        bins: &["gs"],
        accepts: PDF,
        multi: false,
        params: &[],
        output: Output::File { suffix: "gray", ext: |_| "pdf".into() },
        build: |j| {
            vec![Step::Run(
                "gs",
                args![
                    "-sDEVICE=pdfwrite",
                    "-sColorConversionStrategy=Gray",
                    "-dProcessColorModel=/DeviceGray",
                    "-dNOPAUSE",
                    "-dBATCH",
                    "-dQUIET",
                    gs_out(&j.out()),
                    j.input()
                ],
            )]
        },
    },
    Tool {
        id: "pdf-encrypt",
        family: Family::Pdf,
        name: "Encrypt",
        blurb: "AES-256 password protection.",
        bins: &["qpdf"],
        accepts: PDF,
        multi: false,
        params: &[Param::Text { key: "password", label: "Password", default: "", placeholder: "required" }],
        output: Output::File { suffix: "locked", ext: |_| "pdf".into() },
        // CBA: password goes through argv (visible in `ps` while qpdf runs), qpdf @argfile if that matters
        build: |j| {
            let pw = j.p("password");
            vec![Step::Run("qpdf", args!["--encrypt", pw, pw, "256", "--", j.input(), j.out()])]
        },
    },
    Tool {
        id: "pdf-unlock",
        family: Family::Pdf,
        name: "Unlock",
        blurb: "Remove the password (you need to know it).",
        bins: &["qpdf"],
        accepts: PDF,
        multi: false,
        params: &[Param::Text { key: "password", label: "Password", default: "", placeholder: "current password" }],
        output: Output::File { suffix: "unlocked", ext: |_| "pdf".into() },
        build: |j| {
            vec![Step::Run(
                "qpdf",
                args![format!("--password={}", j.p("password")), "--decrypt", j.input(), j.out()],
            )]
        },
    },
    Tool {
        id: "pdf-web",
        family: Family::Pdf,
        name: "Optimize for web",
        blurb: "Linearize so browsers show page 1 before the download ends.",
        bins: &["qpdf"],
        accepts: PDF,
        multi: false,
        params: &[],
        output: Output::File { suffix: "web", ext: |_| "pdf".into() },
        build: |j| vec![Step::Run("qpdf", args!["--linearize", "--object-streams=generate", j.input(), j.out()])],
    },
    Tool {
        id: "pdf-studio",
        family: Family::Pdf,
        name: "PDF Studio",
        blurb: "Annotate, sign, redact, reorder — opens in PdfCraft.",
        bins: &["pdfcraft"],
        accepts: PDF,
        multi: false,
        params: &[],
        output: Output::None,
        build: |j| vec![Step::Launch("pdfcraft", args![j.input()])],
    },
    // ──────────────────────────── Image ────────────────────────────
    Tool {
        id: "img-convert",
        family: Family::Image,
        name: "Convert image",
        blurb: "PNG, JPG, WebP, AVIF, TIFF, GIF.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[Param::Select {
            key: "format",
            label: "To",
            options: &[
                ("webp", "WebP"),
                ("png", "PNG"),
                ("jpg", "JPG"),
                ("avif", "AVIF"),
                ("tiff", "TIFF"),
                ("gif", "GIF"),
            ],
            default: "webp",
        }],
        output: Output::File { suffix: "converted", ext: fmt },
        build: |j| vec![magick(j, args!["-auto-orient", "-quality", "90"])],
    },
    Tool {
        id: "img-compress",
        family: Family::Image,
        name: "Compress image",
        blurb: "Smaller file, same format, metadata stripped.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[Param::Select {
            key: "quality",
            label: "Quality",
            options: &[("85", "High"), ("72", "Balanced"), ("55", "Small")],
            default: "72",
        }],
        output: Output::File { suffix: "compressed", ext: same },
        build: |j| {
            vec![magick(
                j,
                args!["-auto-orient", "-strip", "-quality", j.p("quality"), "-define", "png:compression-level=9"],
            )]
        },
    },
    Tool {
        id: "img-resize",
        family: Family::Image,
        name: "Resize",
        blurb: "Fit within a size, never upscales.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[Param::Select {
            key: "size",
            label: "Fit within",
            options: &[
                ("3840x3840>", "3840 px"),
                ("2560x2560>", "2560 px"),
                ("1920x1920>", "1920 px"),
                ("1280x1280>", "1280 px"),
                ("800x800>", "800 px"),
                ("512x512>", "512 px"),
                ("50%", "Half size"),
            ],
            default: "1920x1920>",
        }],
        output: Output::File { suffix: "resized", ext: same },
        build: |j| vec![magick(j, args!["-auto-orient", "-resize", j.p("size")])],
    },
    Tool {
        id: "img-rotate",
        family: Family::Image,
        name: "Rotate & flip",
        blurb: "Quarter turns and mirrors.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[Param::Select {
            key: "op",
            label: "Action",
            options: &[
                ("-rotate 90", "90° right"),
                ("-rotate -90", "90° left"),
                ("-rotate 180", "180°"),
                ("-flop", "Mirror horizontally"),
                ("-flip", "Mirror vertically"),
            ],
            default: "-rotate 90",
        }],
        output: Output::File { suffix: "rotated", ext: same },
        build: |j| {
            let mut a = args!["-auto-orient"];
            a.extend(tokens(j.p("op")));
            vec![magick(j, a)]
        },
    },
    Tool {
        id: "img-trim",
        family: Family::Image,
        name: "Auto-crop borders",
        blurb: "Trims uniform edges (scans, screenshots).",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[],
        output: Output::File { suffix: "trimmed", ext: same },
        build: |j| vec![magick(j, args!["-auto-orient", "-fuzz", "8%", "-trim", "+repage"])],
    },
    Tool {
        id: "img-strip",
        family: Family::Image,
        name: "Strip metadata",
        blurb: "Removes EXIF, GPS and camera data.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[],
        output: Output::File { suffix: "clean", ext: same },
        build: |j| vec![magick(j, args!["-auto-orient", "-strip"])],
    },
    Tool {
        id: "img-remove-bg",
        family: Family::Image,
        name: "Remove background",
        blurb: "Local AI cut-out (rembg). Transparent PNG.",
        bins: &["rembg"],
        accepts: IMG,
        multi: false,
        params: &[],
        output: Output::File { suffix: "cutout", ext: |_| "png".into() },
        build: |j| vec![Step::Run("rembg", args!["i", j.input(), j.out()])],
    },
    Tool {
        id: "img-vectorize",
        family: Family::Image,
        name: "Vectorize",
        blurb: "Logos and line art to a clean SVG.",
        bins: &["magick", "potrace"],
        accepts: IMG,
        multi: false,
        params: &[Param::Select {
            key: "threshold",
            label: "Threshold",
            options: &[("35%", "Light"), ("50%", "Medium"), ("65%", "Dark")],
            default: "50%",
        }],
        output: Output::File { suffix: "vector", ext: |_| "svg".into() },
        build: |j| {
            let bmp = j.tmp("mask.pbm");
            vec![
                Step::Run(
                    "magick",
                    args![
                        j.input(),
                        "-background",
                        "white",
                        "-alpha",
                        "remove",
                        "-colorspace",
                        "gray",
                        "-threshold",
                        j.p("threshold"),
                        bmp.clone()
                    ],
                ),
                Step::Run("potrace", args!["-s", bmp, "-o", j.out()]),
            ]
        },
    },
    Tool {
        id: "img-stylize",
        family: Family::Image,
        name: "Stylize",
        blurb: "Grayscale, sepia, blur, sharpen, poster.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[Param::Select {
            key: "effect",
            label: "Effect",
            options: &[
                ("-colorspace Gray", "Grayscale"),
                ("-sepia-tone 80%", "Sepia"),
                ("-blur 0x6", "Blur"),
                ("-unsharp 0x1.5+1.2+0.02", "Sharpen"),
                ("-posterize 5", "Posterize"),
                ("-paint 4", "Oil paint"),
                ("-charcoal 2", "Charcoal"),
                ("-negate", "Invert"),
            ],
            default: "-colorspace Gray",
        }],
        output: Output::File { suffix: "styled", ext: same },
        build: |j| {
            let mut a = args!["-auto-orient"];
            a.extend(tokens(j.p("effect")));
            vec![magick(j, a)]
        },
    },
    Tool {
        id: "img-watermark",
        family: Family::Image,
        name: "Watermark",
        blurb: "Text stamped in a corner.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[
            Param::Text { key: "text", label: "Text", default: "© ", placeholder: "© you" },
            Param::Select {
                key: "pos",
                label: "Corner",
                options: &[
                    ("southeast", "Bottom right"),
                    ("southwest", "Bottom left"),
                    ("northeast", "Top right"),
                    ("center", "Center"),
                ],
                default: "southeast",
            },
            Param::Select {
                key: "size",
                label: "Size",
                options: &[("28", "Small"), ("56", "Medium"), ("112", "Large")],
                default: "56",
            },
        ],
        output: Output::File { suffix: "watermarked", ext: same },
        build: |j| {
            // A leading '@' makes ImageMagick read the text from a file.
            let text = j.p("text");
            let text = if text.starts_with('@') { format!("\\{text}") } else { text.to_string() };
            vec![magick(
                j,
                args![
                    "-auto-orient",
                    "-gravity",
                    j.p("pos"),
                    "-pointsize",
                    j.p("size"),
                    "-fill",
                    "rgba(255,255,255,0.6)",
                    "-stroke",
                    "rgba(0,0,0,0.35)",
                    "-strokewidth",
                    "1",
                    "-annotate",
                    "+24+24",
                    text
                ],
            )]
        },
    },
    Tool {
        id: "img-upscale",
        family: Family::Image,
        name: "Upscale",
        blurb: "Lanczos + sharpen. Not AI, but honest.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[Param::Select {
            key: "factor",
            label: "Factor",
            options: &[("200%", "2×"), ("300%", "3×"), ("400%", "4×")],
            default: "200%",
        }],
        output: Output::File { suffix: "upscaled", ext: same },
        build: |j| vec![magick(j, args!["-filter", "Lanczos", "-resize", j.p("factor"), "-unsharp", "0x0.75"])],
    },
    Tool {
        id: "img-icon",
        family: Family::Image,
        name: "Icon pack",
        blurb: "Multi-size .ico, 16 to 256 px.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[],
        output: Output::File { suffix: "icon", ext: |_| "ico".into() },
        build: |j| {
            vec![magick(j, args!["-background", "none", "-define", "icon:auto-resize=256,128,64,48,32,16"])]
        },
    },
    Tool {
        id: "img-palette",
        family: Family::Image,
        name: "Color palette",
        blurb: "Dominant colours as a swatch strip.",
        bins: &["magick"],
        accepts: IMG,
        multi: false,
        params: &[Param::Select {
            key: "n",
            label: "Colours",
            options: &[("5", "5"), ("8", "8"), ("12", "12")],
            default: "8",
        }],
        output: Output::File { suffix: "palette", ext: |_| "png".into() },
        build: |j| {
            vec![magick(
                j,
                args!["-resize", "256x256", "-colors", j.p("n"), "-unique-colors", "-scale", "8000%"],
            )]
        },
    },
    Tool {
        id: "img-contact-sheet",
        family: Family::Image,
        name: "Contact sheet",
        blurb: "All dropped images on one grid.",
        bins: &["magick"],
        accepts: IMG,
        multi: true,
        params: &[Param::Select {
            key: "cols",
            label: "Columns",
            options: &[("3", "3"), ("4", "4"), ("6", "6")],
            default: "4",
        }],
        output: Output::File { suffix: "sheet", ext: |_| "jpg".into() },
        build: |j| {
            let mut a = args!["montage"];
            a.extend(j.inputs.iter().map(|p| s(p)));
            a.extend(args![
                "-auto-orient",
                "-tile",
                format!("{}x", j.p("cols")),
                "-geometry",
                "360x360+8+8",
                "-background",
                "#1a1a1a",
                j.out()
            ]);
            vec![Step::Run("magick", a)]
        },
    },
    // ──────────────────────────── Video ────────────────────────────
    Tool {
        id: "vid-compress",
        family: Family::Video,
        name: "Compress video",
        blurb: "H.264 MP4 that plays everywhere.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "crf",
            label: "Strength",
            options: &[("23", "Light"), ("28", "Balanced"), ("32", "Strong")],
            default: "28",
        }],
        output: Output::File { suffix: "compressed", ext: |_| "mp4".into() },
        build: |j| {
            vec![ffmpeg(
                j,
                args![
                    "-c:v",
                    "libx264",
                    "-preset",
                    "medium",
                    "-crf",
                    j.p("crf"),
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "128k",
                    "-movflags",
                    "+faststart"
                ],
            )]
        },
    },
    Tool {
        id: "vid-convert",
        family: Family::Video,
        name: "Convert video",
        blurb: "MP4, WebM, MOV, MKV.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "format",
            label: "To",
            options: &[("mp4", "MP4"), ("webm", "WebM"), ("mov", "MOV"), ("mkv", "MKV")],
            default: "mp4",
        }],
        output: Output::File { suffix: "converted", ext: fmt },
        build: |j| vec![ffmpeg(j, args![])],
    },
    Tool {
        id: "vid-gif",
        family: Family::Video,
        name: "Video to GIF",
        blurb: "Palette-optimised, loops forever.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[
            Param::Select {
                key: "fps",
                label: "Frame rate",
                options: &[("10", "10 fps"), ("15", "15 fps"), ("24", "24 fps")],
                default: "15",
            },
            Param::Select {
                key: "width",
                label: "Width",
                options: &[("320", "320 px"), ("480", "480 px"), ("640", "640 px"), ("800", "800 px")],
                default: "480",
            },
        ],
        output: Output::File { suffix: "anim", ext: |_| "gif".into() },
        build: |j| {
            let vf = format!(
                "fps={},scale={}:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse",
                j.p("fps"),
                j.p("width")
            );
            vec![ffmpeg(j, args!["-vf", vf, "-loop", "0"])]
        },
    },
    Tool {
        id: "vid-trim",
        family: Family::Video,
        name: "Trim",
        blurb: "Keep a start→end slice. Frame accurate.",
        bins: &["ffmpeg"],
        accepts: AV,
        multi: false,
        params: TRIM,
        output: Output::File { suffix: "trimmed", ext: same },
        build: |j| {
            // Input-side seek + re-encode: fast and frame accurate.
            vec![Step::Run(
                "ffmpeg",
                args![
                    "-hide_banner",
                    "-nostdin",
                    "-y",
                    "-ss",
                    j.p("start"),
                    "-to",
                    j.p("end"),
                    "-i",
                    j.input(),
                    j.out()
                ],
            )]
        },
    },
    Tool {
        id: "vid-extract-audio",
        family: Family::Video,
        name: "Extract audio",
        blurb: "Soundtrack only.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "format",
            label: "Format",
            options: &[("mp3", "MP3"), ("m4a", "M4A"), ("wav", "WAV"), ("flac", "FLAC")],
            default: "mp3",
        }],
        output: Output::File { suffix: "audio", ext: fmt },
        build: |j| vec![ffmpeg(j, args!["-vn"])],
    },
    Tool {
        id: "vid-mute",
        family: Family::Video,
        name: "Mute",
        blurb: "Drop the audio track, no re-encode.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[],
        output: Output::File { suffix: "muted", ext: same },
        build: |j| vec![ffmpeg(j, args!["-c:v", "copy", "-an"])],
    },
    Tool {
        id: "vid-speed",
        family: Family::Video,
        name: "Change speed",
        blurb: "Slow-mo or timelapse, audio pitch kept.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "factor",
            label: "Speed",
            options: &[("0.5", "0.5×"), ("0.75", "0.75×"), ("1.25", "1.25×"), ("1.5", "1.5×"), ("2", "2×")],
            default: "1.5",
        }],
        output: Output::File { suffix: "speed", ext: same },
        build: |j| {
            let f = j.p("factor");
            vec![ffmpeg(j, args!["-vf", format!("setpts=PTS/{f}"), "-af", format!("atempo={f}")])]
        },
    },
    Tool {
        id: "vid-resize",
        family: Family::Video,
        name: "Resize video",
        blurb: "Scale down to a standard height.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "height",
            label: "Height",
            options: &[("2160", "2160p"), ("1080", "1080p"), ("720", "720p"), ("480", "480p")],
            default: "720",
        }],
        output: Output::File { suffix: "resized", ext: same },
        build: |j| vec![ffmpeg(j, args!["-vf", format!("scale=-2:{}", j.p("height")), "-c:a", "copy"])],
    },
    Tool {
        id: "vid-rotate",
        family: Family::Video,
        name: "Rotate video",
        blurb: "Fix phone footage.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "vf",
            label: "Action",
            options: &[
                ("transpose=1", "90° right"),
                ("transpose=2", "90° left"),
                ("transpose=1,transpose=1", "180°"),
                ("hflip", "Mirror"),
            ],
            default: "transpose=1",
        }],
        output: Output::File { suffix: "rotated", ext: same },
        build: |j| vec![ffmpeg(j, args!["-vf", j.p("vf"), "-c:a", "copy"])],
    },
    Tool {
        id: "vid-join",
        family: Family::Video,
        name: "Join videos",
        blurb: "Back to back, no re-encode (same camera/settings).",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: true,
        params: &[],
        output: Output::File { suffix: "joined", ext: same },
        // CBA: concat demuxer + stream copy, needs matching codecs; concat filter with re-encode if mixed sources matter
        build: |j| {
            let list = j.tmp.join("list.txt");
            let body: String = j
                .inputs
                .iter()
                .map(|p| format!("file '{}'\n", s(p).replace('\'', "'\\''")))
                .collect();
            vec![
                Step::Write(list.clone(), body),
                Step::Run(
                    "ffmpeg",
                    args![
                        "-hide_banner",
                        "-nostdin",
                        "-y",
                        "-f",
                        "concat",
                        "-safe",
                        "0",
                        "-i",
                        s(&list),
                        "-c",
                        "copy",
                        j.out()
                    ],
                ),
            ]
        },
    },
    Tool {
        id: "vid-frames",
        family: Family::Video,
        name: "Extract frames",
        blurb: "One still every N seconds.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "every",
            label: "Every",
            options: &[("1", "1 s"), ("5", "5 s"), ("10", "10 s"), ("30", "30 s")],
            default: "5",
        }],
        output: Output::Dir("frames"),
        build: |j| {
            vec![Step::Run(
                "ffmpeg",
                args![
                    "-hide_banner",
                    "-nostdin",
                    "-y",
                    "-i",
                    j.input(),
                    "-vf",
                    format!("fps=1/{}", j.p("every")),
                    "-q:v",
                    "2",
                    j.out_pattern("-%04d.jpg")
                ],
            )]
        },
    },
    // ──────────────────────────── Audio ────────────────────────────
    Tool {
        id: "aud-convert",
        family: Family::Audio,
        name: "Convert audio",
        blurb: "MP3, M4A, WAV, FLAC, OGG, Opus.",
        bins: &["ffmpeg"],
        accepts: AV,
        multi: false,
        params: &[Param::Select {
            key: "format",
            label: "To",
            options: &[
                ("mp3", "MP3"),
                ("m4a", "M4A"),
                ("wav", "WAV"),
                ("flac", "FLAC"),
                ("ogg", "OGG"),
                ("opus", "Opus"),
            ],
            default: "mp3",
        }],
        output: Output::File { suffix: "converted", ext: fmt },
        build: |j| vec![ffmpeg(j, args!["-vn"])],
    },
    Tool {
        id: "aud-compress",
        family: Family::Audio,
        name: "Compress audio",
        blurb: "MP3 at the bitrate you pick.",
        bins: &["ffmpeg"],
        accepts: AUD,
        multi: false,
        params: &[Param::Select {
            key: "bitrate",
            label: "Bitrate",
            options: &[("192k", "192 kbps"), ("128k", "128 kbps"), ("96k", "96 kbps"), ("64k", "64 kbps")],
            default: "128k",
        }],
        output: Output::File { suffix: "compressed", ext: |_| "mp3".into() },
        build: |j| vec![ffmpeg(j, args!["-vn", "-c:a", "libmp3lame", "-b:a", j.p("bitrate")])],
    },
    Tool {
        id: "aud-volume",
        family: Family::Audio,
        name: "Volume & normalize",
        blurb: "Broadcast loudness or a fixed gain.",
        bins: &["ffmpeg"],
        accepts: AV,
        multi: false,
        params: &[Param::Select {
            key: "af",
            label: "Adjust",
            options: &[
                ("loudnorm=I=-16:TP=-1.5:LRA=11", "Normalize (−16 LUFS)"),
                ("volume=6dB", "+6 dB"),
                ("volume=3dB", "+3 dB"),
                ("volume=-3dB", "−3 dB"),
                ("volume=-6dB", "−6 dB"),
            ],
            default: "loudnorm=I=-16:TP=-1.5:LRA=11",
        }],
        output: Output::File { suffix: "leveled", ext: same },
        build: |j| vec![ffmpeg(j, args!["-af", j.p("af"), "-c:v", "copy"])],
    },
    Tool {
        id: "aud-speed",
        family: Family::Audio,
        name: "Audio speed",
        blurb: "Faster or slower, pitch kept.",
        bins: &["ffmpeg"],
        accepts: AUD,
        multi: false,
        params: &[Param::Select {
            key: "factor",
            label: "Speed",
            options: &[("0.75", "0.75×"), ("1.25", "1.25×"), ("1.5", "1.5×"), ("2", "2×")],
            default: "1.25",
        }],
        output: Output::File { suffix: "speed", ext: same },
        build: |j| vec![ffmpeg(j, args!["-af", format!("atempo={}", j.p("factor"))])],
    },
    Tool {
        id: "aud-waveform",
        family: Family::Audio,
        name: "Waveform image",
        blurb: "The whole track as a PNG.",
        bins: &["ffmpeg"],
        accepts: AV,
        multi: false,
        params: &[],
        output: Output::File { suffix: "waveform", ext: |_| "png".into() },
        build: |j| {
            vec![ffmpeg(
                j,
                args!["-filter_complex", "showwavespic=s=1600x400:colors=#f2a33a", "-frames:v", "1"],
            )]
        },
    },
    // ──────────────────────────── Files ────────────────────────────
    Tool {
        id: "arc-create",
        family: Family::Files,
        name: "Create archive",
        blurb: "Zip or 7z everything you drop.",
        bins: &["7z"],
        accepts: &[],
        multi: true,
        params: &[Param::Select {
            key: "format",
            label: "Format",
            options: &[("zip", "ZIP"), ("7z", "7z")],
            default: "zip",
        }],
        output: Output::File { suffix: "archive", ext: fmt },
        build: |j| {
            let mut a = args!["a", format!("-t{}", j.p("format")), "-y", j.out()];
            a.extend(j.inputs.iter().map(|p| s(p)));
            vec![Step::Run("7z", a)]
        },
    },
    Tool {
        id: "arc-extract",
        family: Family::Files,
        name: "Extract archive",
        blurb: "zip, 7z, rar, tar, iso…",
        bins: &["7z"],
        accepts: ARCHIVES,
        multi: false,
        params: &[],
        output: Output::Dir("extracted"),
        build: |j| vec![Step::Run("7z", args!["x", "-y", format!("-o{}", j.out()), j.input()])],
    },
    Tool {
        id: "batch-rename",
        family: Family::Files,
        name: "Batch rename",
        blurb: "{name} and {n} patterns, extension kept.",
        bins: &[],
        accepts: &[],
        multi: true,
        params: &[
            Param::Text { key: "pattern", label: "Pattern", default: "{name}-{n}", placeholder: "holiday-{n}" },
            Param::Text { key: "start", label: "First number", default: "1", placeholder: "1" },
        ],
        output: Output::None,
        // CBA: no live preview / undo, the patterns are simple enough to eyeball
        build: |j| {
            let start: usize = j.p("start").trim().parse().unwrap_or(1);
            let width = (start + j.inputs.len()).saturating_sub(1).to_string().len().max(2);
            j.inputs
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let n = format!("{:0width$}", start + i);
                    let mut name = j.p("pattern").replace("{name}", &stem_of(p)).replace("{n}", &n);
                    let ext = p.extension().map(|e| e.to_string_lossy().into_owned());
                    if let Some(ext) = ext {
                        name = format!("{name}.{ext}");
                    }
                    Step::Rename(p.clone(), p.with_file_name(name))
                })
                .collect()
        },
    },
];

pub fn find(id: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(tool: &str, inputs: &[&str], out: &str, params: &[(&str, &str)]) -> Vec<Step> {
        let t = find(tool).unwrap();
        let p = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let j = Job {
            inputs: inputs.iter().map(PathBuf::from).collect(),
            out: PathBuf::from(out),
            tmp: PathBuf::from("/tmp/job"),
            params: with_defaults(t, p),
        };
        (t.build)(&j)
    }

    fn run_args(step: &Step) -> (&'static str, Vec<&str>) {
        match step {
            Step::Run(bin, a) => (bin, a.iter().map(String::as_str).collect()),
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn ids_are_unique_and_params_have_valid_defaults() {
        let mut ids: Vec<_> = TOOLS.iter().map(|t| t.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), TOOLS.len());
        for t in TOOLS {
            for p in t.params {
                if let Param::Select { options, default, .. } = p {
                    assert!(options.iter().any(|(v, _)| v == default), "{}: bad default", t.id);
                }
            }
        }
    }

    #[test]
    fn compress_pdf_uses_level_and_escapes_percent() {
        let steps = job("pdf-compress", &["/d/in.pdf"], "/d/100%.compressed.pdf", &[("level", "screen")]);
        let (bin, a) = run_args(&steps[0]);
        assert_eq!(bin, "gs");
        assert!(a.contains(&"-dPDFSETTINGS=/screen"));
        assert!(a.contains(&"-sOutputFile=/d/100%%.compressed.pdf"));
        assert_eq!(*a.last().unwrap(), "/d/in.pdf");
    }

    #[test]
    fn merge_keeps_drop_order() {
        let steps = job("pdf-merge", &["/b.pdf", "/a.pdf"], "/b.merged.pdf", &[]);
        let (_, a) = run_args(&steps[0]);
        assert_eq!(a, ["--empty", "--pages", "/b.pdf", "/a.pdf", "--", "/b.merged.pdf"]);
    }

    #[test]
    fn ocr_pdf_rasterizes_lists_then_recognizes() {
        let steps = job("pdf-ocr", &["/d/scan.pdf"], "/d/scan.ocr.pdf", &[("lang", "fra")]);
        assert_eq!(steps.len(), 3);
        assert!(matches!(steps[1], Step::ListDir { ext: "png", .. }));
        let (bin, a) = run_args(&steps[2]);
        assert_eq!(bin, "tesseract");
        let list = s(&PathBuf::from("/tmp/job").join("pages.txt"));
        assert_eq!(a, [list.as_str(), "/d/scan.ocr", "-l", "fra", "pdf"]);
    }

    #[test]
    fn ocr_image_is_a_single_tesseract_call() {
        let steps = job("pdf-ocr", &["/d/photo.jpg"], "/d/photo.ocr.pdf", &[]);
        let (_, a) = run_args(&steps[0]);
        assert_eq!(a, ["/d/photo.jpg", "/d/photo.ocr", "-l", "eng", "pdf"]);
    }

    #[test]
    fn gif_filter_carries_fps_and_width() {
        let steps = job("vid-gif", &["/v.mp4"], "/v.anim.gif", &[("fps", "10"), ("width", "320")]);
        let (_, a) = run_args(&steps[0]);
        assert!(a.iter().any(|x| x.starts_with("fps=10,scale=320:-1")));
    }

    #[test]
    fn trim_seeks_on_input_side() {
        let steps = job("vid-trim", &["/v.mp4"], "/v.trimmed.mp4", &[("start", "00:00:03"), ("end", "00:00:07")]);
        let (_, a) = run_args(&steps[0]);
        let ss = a.iter().position(|x| *x == "-ss").unwrap();
        let i = a.iter().position(|x| *x == "-i").unwrap();
        assert!(ss < i);
        assert_eq!(a[ss + 1], "00:00:03");
    }

    #[test]
    fn frame_pattern_escapes_path_percent_but_not_counter() {
        let steps = job("vid-frames", &["/d/50%off.mp4"], "/d/50%off.frames", &[]);
        let (_, a) = run_args(&steps[0]);
        let dir = s(&PathBuf::from("/d/50%off.frames").join("50%off")).replace('%', "%%");
        assert_eq!(*a.last().unwrap(), format!("{dir}-%04d.jpg"));
    }

    #[test]
    fn join_quotes_single_quotes_in_concat_list() {
        let steps = job("vid-join", &["/v/it's.mp4", "/v/b.mp4"], "/v/it's.joined.mp4", &[]);
        match &steps[0] {
            Step::Write(_, body) => assert_eq!(body, "file '/v/it'\\''s.mp4'\nfile '/v/b.mp4'\n"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rename_numbers_and_keeps_extension() {
        let steps = job("batch-rename", &["/p/a.JPG", "/p/b.jpg"], "", &[("pattern", "trip-{n}")]);
        assert_eq!(steps[0], Step::Rename("/p/a.JPG".into(), "/p/trip-01.JPG".into()));
        assert_eq!(steps[1], Step::Rename("/p/b.jpg".into(), "/p/trip-02.jpg".into()));
    }

    #[test]
    fn watermark_neutralizes_at_file_include() {
        let steps = job("img-watermark", &["/i.png"], "/i.w.png", &[("text", "@/etc/passwd")]);
        let (_, a) = run_args(&steps[0]);
        assert_eq!(a[a.len() - 2], "\\@/etc/passwd");
    }
}
