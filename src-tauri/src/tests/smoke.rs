//! Every tool against the real binaries, on fixtures generated here.
//! Needs gs (built with OCR), qpdf, ffmpeg, magick, 7z and potrace (PATH or REMISE_VENDOR).

use crate::bins::{command, resolve};
use crate::runner::{StepReq, run_chain};
use crate::tools::{self, Family, Inputs};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn sh(bin: &str, args: &[&str]) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (path, args) = resolve(bin, &args).unwrap_or_else(|| panic!("{bin} not found"));
    let out = command(&path).args(&args).stdin(Stdio::null()).output().unwrap();
    assert!(out.status.success(), "fixture {bin} {args:?}\n{}", String::from_utf8_lossy(&out.stderr));
}

fn go(tool: &str, inputs: &[&Path], params: &[(&str, &str)], home: &Path) -> Result<Vec<String>, String> {
    let params = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let inputs = inputs.iter().map(|p| tools::s(p)).collect();
    run_chain(&|_| {}, tool, vec![StepReq { tool: tool.into(), params }], inputs, home)
}

struct Fixtures {
    dir: PathBuf,
    png: PathBuf,
    png2: PathBuf,
    pdf: PathBuf,
    mp4: PathBuf,
    mp4b: PathBuf,
    wav: PathBuf,
    srt: PathBuf,
    html: PathBuf,
    epub: PathBuf,
    csv: PathBuf,
    md: PathBuf,
    docx: PathBuf,
    font: Option<PathBuf>,
}

fn system_font() -> Option<PathBuf> {
    [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}

fn fixtures(test: &str) -> Fixtures {
    let dir = std::env::temp_dir().join(format!("remise-smoke-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let f = |n: &str| dir.join(n);
    // A `%` in a name has bitten printf-style CLIs before: keep it in the fixtures.
    let fx = Fixtures {
        png: f("a 50%.png"),
        png2: f("b.png"),
        pdf: f("doc.pdf"),
        mp4: f("clip.mp4"),
        mp4b: f("clip2.mp4"),
        wav: f("tone.wav"),
        srt: f("clip.srt"),
        html: f("page.html"),
        epub: f("book.epub"),
        csv: f("sheet.csv"),
        md: f("notes.md"),
        docx: f("letter.docx"),
        // Copied: the specimen lands next to its input, and system font dirs are read-only.
        font: system_font().map(|f| {
            let copy = dir.join(f.file_name().unwrap());
            fs::copy(&f, &copy).unwrap();
            copy
        }),
        dir: dir.clone(),
    };
    let p = |x: &PathBuf| x.to_str().unwrap().to_string();
    let font = tools::system_font();
    sh(
        "magick",
        &[
            "-size",
            "640x400",
            "xc:white",
            "-font",
            &font,
            "-fill",
            "black",
            "-pointsize",
            "48",
            "-annotate",
            "+40+200",
            "Hello Remise",
            &p(&fx.png),
        ],
    );
    sh("magick", &["-size", "320x200", "gradient:orange-navy", &p(&fx.png2)]);
    sh("magick", &[&p(&fx.png), &p(&fx.png2), &p(&fx.pdf)]);
    for out in [&fx.mp4, &fx.mp4b] {
        sh(
            "ffmpeg",
            &[
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=s=320x240:d=3",
                "-f",
                "lavfi",
                "-i",
                "sine=d=3",
                "-shortest",
                "-pix_fmt",
                "yuv420p",
                &p(out),
            ],
        );
    }
    sh("ffmpeg", &["-y", "-f", "lavfi", "-i", "sine=d=2", &p(&fx.wav)]);
    fs::write(&fx.srt, "1\n00:00:00,000 --> 00:00:02,000\nHello Remise\n").unwrap();
    fs::write(&fx.html, "<!doctype html><h1>Hello Remise</h1><p>Printed headless.</p>").unwrap();
    fs::write(&fx.csv, "item,price\nbolt,0.10\nnut,0.05\n").unwrap();
    fs::write(&fx.md, "# Remise notes\n\nHello **world**, ça marche.\n\n- one\n- two\n\n| tool | price |\n|---|---|\n| Remise | 0 € |\n\n![gradient](b.png)\n\n# Second\n\nMore.\n").unwrap();
    // A real .docx with an image, made by pandoc itself from the Markdown above.
    let resources = format!("--resource-path={}", p(&dir));
    sh("pandoc", &[&p(&fx.md), &resources, "-o", &p(&fx.docx)]);
    sh("pandoc", &[&p(&fx.md), &resources, "--metadata", "title=Remise", "-o", &p(&fx.epub)]);
    fx
}

/// Inputs for a tool: the special mixes by id, the rest by what it accepts.
fn inputs_for<'a>(t: &tools::Tool, fx: &'a Fixtures) -> Option<Vec<&'a Path>> {
    let takes = |e: &str| t.accepts.contains(&e);
    let v: Vec<&Path> = match t.id {
        "pdf-sign" => vec![&fx.pdf, &fx.png2],
        "vid-watermark" => vec![&fx.mp4, &fx.png2],
        "vid-caption" => vec![&fx.mp4, &fx.srt],
        "vid-add-audio" => vec![&fx.mp4, &fx.wav],
        "img-attach" => vec![&fx.png2, &fx.png],
        "aud-merge" => vec![&fx.wav, &fx.wav],
        "pdf-ebook" => vec![&fx.epub],
        "pdf-office" => vec![&fx.csv],
        "doc-to-markdown" => vec![&fx.docx],
        "doc-convert" => vec![&fx.md],
        "doc-md-pdf" | "doc-md-docx" | "doc-md-slides" => vec![&fx.md],
        "pdf-html" | "create-html-image" => vec![&fx.html],
        "create-font" => vec![fx.font.as_deref()?],
        _ if t.inputs == Inputs::None => vec![],
        _ => match (t.inputs == Inputs::Many, t.family) {
            (true, Family::Video) => vec![&fx.mp4, &fx.mp4b],
            (true, Family::Files) => vec![&fx.png, &fx.pdf],
            (true, _) if takes("pdf") => vec![&fx.pdf, &fx.pdf],
            (true, _) => vec![&fx.png, &fx.png2],
            _ if takes("pdf") => vec![&fx.pdf],
            _ if takes("png") => vec![&fx.png],
            (_, Family::Audio) if takes("wav") => vec![&fx.wav],
            _ => vec![&fx.mp4],
        },
    };
    Some(v)
}

fn params_for(id: &str) -> &'static [(&'static str, &'static str)] {
    match id {
        "pdf-encrypt" => &[("password", "s3cret")],
        "pdf-extract" => &[("pages", "2")],
        "pdf-organize" => &[("order", "2,1")],
        "vid-trim" | "aud-trim" => &[("start", "00:00:01"), ("end", "00:00:02")],
        "vid-insert" => &[("at", "1.5")],
        "create-qr" => &[("text", "https://example.org/remise")],
        "create-shader" => &[("size", "1080x1080")],
        _ => &[],
    }
}

// Bundled in releases but not necessarily installed on a dev machine (CI has them vendored).
const OPTIONAL_BINS: &[&str] = &["chromium", "soffice", "waifu2x-ncnn-vulkan", "pandoc", "typst"];

/// Optional installs, the network, and tools that move files or need a previous output.
const NOT_IN_LOOP: &[&str] = &[
    "pdf-studio",
    "img-remove-bg",
    "vid-transcribe",
    "vid-download",
    "batch-rename",
    "file-sort",
    "file-dates",
    "pdf-unlock",
    "arc-extract",
];

#[test]
#[ignore]
fn smoke_every_tool() {
    let fx = fixtures("all");
    let home = fx.dir.join("made");
    fs::create_dir_all(&home).unwrap();
    let mut failures = vec![];
    let mut check = |tool: &str, inputs: &[&Path], params: &[(&str, &str)]| match go(tool, inputs, params, &home) {
        Ok(out) => {
            assert!(!out.is_empty() && out.iter().all(|o| Path::new(o).exists()), "{tool}: {out:?}");
            println!("ok   {tool:20} -> {}", out.join(", "));
        }
        Err(e) => failures.push(format!("{tool}: {e}")),
    };
    // REMISE_SMOKE_SKIP=id,id: what a given CI machine can't run (no GPU driver…).
    let skip = std::env::var("REMISE_SMOKE_SKIP").unwrap_or_default();
    for t in tools::all().filter(|t| !NOT_IN_LOOP.contains(&t.id)) {
        if skip.split(',').any(|s| s == t.id) {
            println!("skip {:20} (REMISE_SMOKE_SKIP)", t.id);
            continue;
        }
        // Browsers and LibreOffice are the user's, not bundled: test them where they exist.
        if let Some(b) = t.bins.iter().find(|b| OPTIONAL_BINS.contains(b) && !crate::bins::available(b)) {
            println!("skip {:20} (no {b} here)", t.id);
            continue;
        }
        match inputs_for(t, &fx) {
            Some(inputs) => check(t.id, &inputs, params_for(t.id)),
            None => println!("skip {:20} (no fixture)", t.id),
        }
    }
    // Second modes of tools whose variants take a different code path.
    check("img-redact", &[&fx.png], &[("mode", "pixelate")]);
    check("img-compare", &[&fx.png, &fx.png2], &[("mode", "diff")]);
    check("create-mockup", &[&fx.png], &[("device", "phone")]);
    check("create-qr", &[], &[("text", "hello"), ("format", "png")]);
    check("create-gradient", &[], &[("kind", "radial")]);
    // A .docx goes through --sandbox. (A distro pandoc without embedded data files can't write
    // docx/odt/epub in sandbox mode; the official build we bundle can, and CI runs that one.)
    for format in ["pdf", "html", "txt"] {
        check("doc-convert", &[&fx.docx], &[("format", format)]);
    }
    for format in ["odt", "epub"] {
        check("doc-convert", &[&fx.md], &[("format", format)]);
    }
    for src in ["mandelbrot", "life=ratio=0.2:mold=8:life_color=#f2a33a:death_color=#3a1d0a", "cellauto=rule=110"] {
        check("create-shader", &[], &[("src", src), ("format", "png"), ("size", "1080x1080")]);
    }

    let zip = go("arc-create", &[&fx.png, &fx.pdf], &[], &home).unwrap();
    check("arc-extract", &[Path::new(&zip[0])], &[]);
    let locked = go("pdf-encrypt", &[&fx.pdf], &[("password", "pw")], &home).unwrap();
    check("pdf-unlock", &[Path::new(&locked[0])], &[("password", "pw")]);

    let loose = fx.dir.join("loose");
    fs::create_dir_all(&loose).unwrap();
    let (r1, r2, pic, song) = (loose.join("r1.txt"), loose.join("r2.txt"), loose.join("pic.jpg"), loose.join("song.mp3"));
    for f in [&r1, &r2, &pic, &song] {
        fs::write(f, "").unwrap();
    }
    check("file-dates", &[&r1], &[("date", "2020-05-17 10:30")]);
    let modified = fs::metadata(&r1).unwrap().modified().unwrap();
    assert_eq!(Some(modified), tools::parse_date("2020-05-17 10:30"));
    check("batch-rename", &[&r1, &r2], &[("pattern", "note-{n}")]);
    assert!(loose.join("note-01.txt").exists() && loose.join("note-02.txt").exists());
    check("file-sort", &[&pic, &song], &[("by", "kind")]);
    assert!(loose.join("Images/pic.jpg").exists() && loose.join("Audio/song.mp3").exists());

    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
    // REMISE_KEEP=1 leaves the outputs around for eyeballing.
    if std::env::var_os("REMISE_KEEP").is_none() {
        let _ = fs::remove_dir_all(&fx.dir);
    }
}

#[test]
#[ignore]
fn smoke_chain_lands_one_file_next_to_input() {
    let fx = fixtures("chain");
    let before = fs::read_dir(&fx.dir).unwrap().count();
    let steps = ["pdf-compress", "pdf-grayscale", "pdf-web"].map(|t| StepReq { tool: t.into(), params: HashMap::new() });
    let out = run_chain(&|_| {}, "chain", steps.into(), vec![tools::s(&fx.pdf)], &fx.dir).unwrap();
    assert!(out[0].ends_with("doc.compressed.gray.web.pdf"), "{out:?}");
    assert_eq!(fs::read_dir(&fx.dir).unwrap().count(), before + 1, "only the final file is added");
    let _ = fs::remove_dir_all(&fx.dir);
}

#[test]
#[ignore]
fn smoke_ffmpeg_reports_progress() {
    let fx = fixtures("progress");
    let seen = std::cell::RefCell::new(vec![]);
    let emit = |p: &crate::runner::Progress| seen.borrow_mut().push(p.pct);
    let req = StepReq { tool: "vid-compress".into(), params: HashMap::new() };
    run_chain(&emit, "p", vec![req], vec![tools::s(&fx.mp4)], &fx.dir).unwrap();
    let pcts: Vec<f64> = seen.borrow().iter().flatten().copied().collect();
    assert!(!pcts.is_empty() && pcts.iter().all(|p| (0.0..=1.0).contains(p)), "{pcts:?}");
    let _ = fs::remove_dir_all(&fx.dir);
}

/// The first-use downloads for real (REMISE_USER_TOOLS must point at a scratch folder), then the
/// tools that need them: Transcribe everywhere, and HTML to PDF with the downloaded Chromium
/// where there is one to download (Linux, macOS).
#[test]
#[ignore]
fn smoke_downloads_then_use_them() {
    let Some(root) = crate::bins::user_tools() else {
        println!("skip (set REMISE_USER_TOOLS to a scratch folder)");
        return;
    };
    let fx = fixtures("downloads");
    let model = crate::fetch::find("whisper-model").unwrap();
    crate::fetch::fetch(&|_| {}, model).unwrap();
    let out = go("vid-transcribe", &[&fx.mp4], &[("lang", "en")], &fx.dir).unwrap();
    assert!(Path::new(&out[0]).is_file(), "{out:?}");
    if let Some(browser) = crate::fetch::find("chromium") {
        let exe = crate::fetch::fetch(&|_| {}, browser).unwrap();
        assert!(exe.starts_with(root) && crate::bins::which("chromium") == Some(exe));
        let pdf = go("pdf-html", &[&fx.html], &[], &fx.dir).unwrap();
        assert!(fs::read(&pdf[0]).unwrap().starts_with(b"%PDF"));
    }
    crate::fetch::fetch(&|_| {}, crate::fetch::find("soffice").unwrap()).unwrap();
    let pdf = go("pdf-office", &[&fx.docx], &[], &fx.dir).unwrap();
    assert!(fs::read(&pdf[0]).unwrap().starts_with(b"%PDF"), "LibreOffice, downloaded and unpacked, converts");
    // PdfCraft only opens a window; check it lands, unpacked from its zip / tar.gz / dmg.
    let editor = crate::fetch::fetch(&|_| {}, crate::fetch::find("pdfcraft").unwrap()).unwrap();
    assert!(editor.is_file() && crate::bins::which("pdfcraft") == Some(editor));
    // Background removal runs the app itself as a helper: REMISE_SELF is the built `remise`.
    if std::env::var_os("REMISE_SELF").is_some() {
        crate::fetch::fetch(&|_| {}, crate::fetch::find("bg-model").unwrap()).unwrap();
        let png = go("img-remove-bg", &[&fx.png2], &[], &fx.dir).unwrap();
        let (magick, args) = resolve("magick", &["identify".into(), "-format".into(), "%[channels]".into(), png[0].clone()]).unwrap();
        let channels = command(&magick).args(&args).output().unwrap().stdout;
        assert!(String::from_utf8_lossy(&channels).contains('a'), "the cut-out has an alpha channel");
    } else {
        println!("skip img-remove-bg (set REMISE_SELF to the built app)");
    }
    let _ = fs::remove_dir_all(&fx.dir);
}

/// Stop kills the program mid-way and leaves nothing next to the input.
#[test]
#[ignore]
fn smoke_stop_kills_the_running_program() {
    let fx = fixtures("stop");
    let long = fx.dir.join("long.mp4");
    sh("ffmpeg", &["-y", "-f", "lavfi", "-i", "testsrc=s=1280x720:d=60", "-pix_fmt", "yuv420p", "-preset", "ultrafast", &tools::s(&long)]);
    let before = fs::read_dir(&fx.dir).unwrap().count();
    let stopper = std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(500));
        crate::runner::stop("stopme");
    });
    let started = std::time::Instant::now();
    let req = StepReq { tool: "vid-compress".into(), params: HashMap::from([("crf".into(), "18".into())]) };
    let res = run_chain(&|_| {}, "stopme", vec![req], vec![tools::s(&long)], &fx.dir);
    stopper.join().unwrap();
    assert_eq!(res, Err(crate::runner::STOPPED_MSG.to_string()));
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "stopped quickly");
    assert_eq!(fs::read_dir(&fx.dir).unwrap().count(), before, "no partial output");
    let _ = fs::remove_dir_all(&fx.dir);
}

/// A document's own images make it through Pandoc's sandbox; anything pointing elsewhere on the
/// disk (absolute path, `..`, `file://`, raw HTML, a YAML `css:`) or isn't an image does not.
#[test]
#[ignore]
fn smoke_pandoc_keeps_own_images_only() {
    let fx = fixtures("pandoc");
    let secret = tools::s(&fx.png2);
    let evil = fx.dir.join("sub").join("evil.md");
    fs::create_dir_all(evil.parent().unwrap()).unwrap();
    fs::write(evil.with_file_name("id_rsa"), "PRIVATE KEY").unwrap();
    let body =
        format!("---\ncss: {secret}\n---\n![a]({secret}) ![b](../b.png) ![c](file://{secret}) ![d](id_rsa)\n\n<img src=\"{secret}\">\n");
    fs::write(&evil, body).unwrap();
    let html = |input: &Path| {
        let out = go("doc-convert", &[input], &[("format", "html")], &fx.dir).unwrap();
        fs::read_to_string(&out[0]).unwrap()
    };
    assert!(html(&fx.md).contains("data:image/png"), "own image kept");
    let leaked = html(&evil);
    assert!(!leaked.contains("data:") && !leaked.contains("PRIVATE"), "{leaked}");
    // In a .docx the image file names sit uncompressed in the zip directory.
    let docx = go("doc-md-docx", &[&fx.md], &[], &fx.dir).unwrap();
    assert!(fs::read(&docx[0]).unwrap().windows(11).any(|w| w == b"word/media/"), "docx image kept");
    let _ = fs::remove_dir_all(&fx.dir);
}

/// Dumps the tool manifest for the UI test: `REMISE_MANIFEST=out.json cargo test -- --ignored manifest`.
#[test]
#[ignore]
fn manifest() {
    let Some(path) = std::env::var_os("REMISE_MANIFEST") else {
        return;
    };
    fs::write(path, serde_json::to_string_pretty(&crate::list_tools()).unwrap()).unwrap();
}
