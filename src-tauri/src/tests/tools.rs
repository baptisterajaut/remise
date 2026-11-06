//! Argument building: pure, no binaries needed.

use crate::tools::*;
use std::path::PathBuf;

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
    let mut ids: Vec<_> = all().map(|t| t.id).collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), total);
    for t in all() {
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
    let renames: Vec<_> = steps
        .iter()
        .map(|s| match s {
            Step::Rename(a, b) => (a.clone(), b.clone()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(renames[0], ("/p/a.JPG".into(), "/p/trip-01.JPG".into()));
    assert_eq!(renames[1], ("/p/b.jpg".into(), "/p/trip-02.jpg".into()));
}

#[test]
fn watermark_neutralizes_at_file_include() {
    let steps = job("img-watermark", &["/i.png"], "/i.w.png", &[("text", "@/etc/passwd")]);
    let (_, a) = run_args(&steps[0]);
    assert_eq!(a[a.len() - 2], "\\@/etc/passwd");
}
