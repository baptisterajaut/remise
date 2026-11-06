//! Every tool against the real binaries, on fixtures generated here.
//! Needs gs, qpdf, tesseract, ffmpeg, magick, 7z and potrace (PATH or REMISE_VENDOR).

use crate::bins::{command, resolve};
use crate::runner::{run_chain, StepReq};
use crate::tools::{self, Family, TOOLS};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn sh(bin: &str, args: &[&str]) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (path, args) = resolve(bin, &args).unwrap_or_else(|| panic!("{bin} not found"));
    let ok = command(&path).args(&args).stdout(Stdio::null()).stderr(Stdio::null()).status();
    assert!(ok.unwrap().success(), "fixture {bin} {args:?}");
}

fn go(tool: &str, inputs: &[&Path], params: &[(&str, &str)]) -> Result<Vec<String>, String> {
    let params = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let inputs = inputs.iter().map(|p| tools::s(p)).collect();
    run_chain(&|_| {}, tool, vec![StepReq { tool: tool.into(), params }], inputs)
}

struct Fixtures {
    dir: PathBuf,
    png: PathBuf,
    png2: PathBuf,
    pdf: PathBuf,
    mp4: PathBuf,
    mp4b: PathBuf,
    wav: PathBuf,
}

fn fixtures(test: &str) -> Fixtures {
    let dir = std::env::temp_dir().join(format!("remise-smoke-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let f = |n: &str| dir.join(n);
    // A `%` in a name has bitten printf-style CLIs before: keep it in the fixtures.
    let fx = Fixtures { png: f("a 50%.png"), png2: f("b.png"), pdf: f("doc.pdf"), mp4: f("clip.mp4"), mp4b: f("clip2.mp4"), wav: f("tone.wav"), dir };
    let p = |x: &PathBuf| x.to_str().unwrap().to_string();
    sh("magick", &["-size", "640x400", "xc:white", "-fill", "black", "-pointsize", "48", "-annotate", "+40+200", "Hello Remise", &p(&fx.png)]);
    sh("magick", &["-size", "320x200", "gradient:orange-navy", &p(&fx.png2)]);
    sh("magick", &[&p(&fx.png), &p(&fx.png2), &p(&fx.pdf)]);
    for out in [&fx.mp4, &fx.mp4b] {
        sh("ffmpeg", &["-y", "-f", "lavfi", "-i", "testsrc=s=320x240:d=3", "-f", "lavfi", "-i", "sine=d=3", "-shortest", "-pix_fmt", "yuv420p", &p(out)]);
    }
    sh("ffmpeg", &["-y", "-f", "lavfi", "-i", "sine=d=2", &p(&fx.wav)]);
    fx
}

/// Picks inputs for a tool from the fixtures by what it accepts.
fn inputs_for<'a>(t: &tools::Tool, fx: &'a Fixtures) -> Vec<&'a Path> {
    let takes = |e: &str| t.accepts.contains(&e);
    match (t.multi, t.family) {
        (true, Family::Video) => vec![&fx.mp4, &fx.mp4b],
        (true, Family::Files) => vec![&fx.png, &fx.pdf],
        (true, _) if takes("pdf") => vec![&fx.pdf, &fx.pdf],
        (true, _) => vec![&fx.png, &fx.png2],
        _ if takes("pdf") => vec![&fx.pdf],
        _ if takes("png") => vec![&fx.png],
        (_, Family::Audio) if takes("wav") => vec![&fx.wav],
        _ => vec![&fx.mp4],
    }
}

fn params_for(id: &str) -> &'static [(&'static str, &'static str)] {
    match id {
        "pdf-encrypt" => &[("password", "s3cret")],
        "pdf-extract" => &[("pages", "2")],
        "vid-trim" => &[("start", "00:00:01"), ("end", "00:00:02")],
        _ => &[],
    }
}

#[test]
#[ignore]
fn smoke_every_tool() {
    let fx = fixtures("all");
    let mut failures = vec![];
    let mut check = |tool: &str, inputs: &[&Path], params: &[(&str, &str)]| match go(tool, inputs, params) {
        Ok(out) => {
            assert!(!out.is_empty() && out.iter().all(|o| Path::new(o).exists()), "{tool}: {out:?}");
            println!("ok   {tool:20} -> {}", out.join(", "));
        }
        Err(e) => failures.push(format!("{tool}: {e}")),
    };
    // Skipped here: they need a previous output, move files, or an optional AI model / app.
    let later = ["pdf-studio", "img-remove-bg", "batch-rename", "pdf-unlock", "arc-extract"];
    for t in TOOLS.iter().filter(|t| !later.contains(&t.id)) {
        check(t.id, &inputs_for(t, &fx), params_for(t.id));
    }
    let zip = go("arc-create", &[&fx.png, &fx.pdf], &[]).unwrap();
    check("arc-extract", &[Path::new(&zip[0])], &[]);
    let locked = go("pdf-encrypt", &[&fx.pdf], &[("password", "pw")]).unwrap();
    check("pdf-unlock", &[Path::new(&locked[0])], &[("password", "pw")]);

    let (r1, r2) = (fx.dir.join("r1.txt"), fx.dir.join("r2.txt"));
    fs::write(&r1, "").unwrap();
    fs::write(&r2, "").unwrap();
    check("batch-rename", &[&r1, &r2], &[("pattern", "note-{n}")]);
    assert!(fx.dir.join("note-01.txt").exists() && fx.dir.join("note-02.txt").exists());

    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
    let _ = fs::remove_dir_all(&fx.dir);
}

#[test]
#[ignore]
fn smoke_chain_lands_one_file_next_to_input() {
    let fx = fixtures("chain");
    let steps = ["pdf-compress", "pdf-grayscale", "pdf-web"].map(|t| StepReq { tool: t.into(), params: HashMap::new() });
    let out = run_chain(&|_| {}, "chain", steps.into(), vec![tools::s(&fx.pdf)]).unwrap();
    assert!(out[0].ends_with("doc.compressed.gray.web.pdf"), "{out:?}");
    let left: Vec<_> = fs::read_dir(&fx.dir).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(left.len(), 7, "only the final file is added: {left:?}");
    let _ = fs::remove_dir_all(&fx.dir);
}

#[test]
#[ignore]
fn smoke_ffmpeg_reports_progress() {
    let fx = fixtures("progress");
    let seen = std::cell::RefCell::new(vec![]);
    let emit = |p: &crate::runner::Progress| seen.borrow_mut().push(p.pct);
    let req = StepReq { tool: "vid-compress".into(), params: HashMap::new() };
    run_chain(&emit, "p", vec![req], vec![tools::s(&fx.mp4)]).unwrap();
    let pcts: Vec<f64> = seen.borrow().iter().flatten().copied().collect();
    assert!(!pcts.is_empty() && pcts.iter().all(|p| (0.0..=1.0).contains(p)), "{pcts:?}");
    let _ = fs::remove_dir_all(&fx.dir);
}

/// Dumps the tool manifest for the UI test: `REMISE_MANIFEST=out.json cargo test -- --ignored manifest`.
#[test]
#[ignore]
fn manifest() {
    let Some(path) = std::env::var_os("REMISE_MANIFEST") else { return };
    fs::write(path, serde_json::to_string_pretty(&crate::list_tools()).unwrap()).unwrap();
}
