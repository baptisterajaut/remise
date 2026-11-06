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
fn ocr_pdf_is_one_ghostscript_pass() {
    let steps = job("pdf-ocr", &["/d/scan.pdf"], "/d/scan.ocr.pdf", &[("lang", "eng+fra")]);
    assert_eq!(steps.len(), 1);
    let (bin, a) = run_args(&steps[0]);
    assert_eq!(bin, "gs");
    for want in ["-sDEVICE=pdfocr24", "-r300", "-sOCRLanguage=eng+fra", "-sOutputFile=/d/scan.ocr.pdf", "/d/scan.pdf"] {
        assert!(a.contains(&want), "{want} in {a:?}");
    }
}

#[test]
fn ocr_image_goes_through_a_pdf_page() {
    let steps = job("pdf-ocr", &["/d/photo.jpg"], "/d/photo.ocr.pdf", &[]);
    assert_eq!(steps.len(), 2);
    assert_eq!(run_args(&steps[0]).0, "magick");
    let (bin, a) = run_args(&steps[1]);
    assert_eq!(bin, "gs");
    assert!(a.contains(&"-sOCRLanguage=eng"));
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

const WITH_SOUND: &str = "  Duration: 00:00:02.00, start: 0.000000, bitrate: 127 kb/s
  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 320x240 [SAR 1:1 DAR 4:3], 45 kb/s, 25 fps, 25 tbr, 12800 tbn (default)
  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, mono, fltp, 69 kb/s (default)";
const SILENT: &str = "  Duration: 00:00:01.00, start: 0.000000, bitrate: 91 kb/s
  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 640x360 [SAR 1:1 DAR 16:9], 81 kb/s, 30 fps, 30 tbr, 15360 tbn (default)";

#[test]
fn ffmpeg_summary_is_read() {
    let m = crate::tools::media::parse(WITH_SOUND);
    assert_eq!(m.video, Some(("h264".into(), 320, 240, Some(25.0))));
    assert_eq!(m.audio, Some(("aac".into(), "44100 Hz".into(), "mono".into())));
    assert_eq!(m.duration, Some(2.0));
    assert_eq!(crate::tools::media::parse(SILENT).audio, None);
}

/// Runs a probing tool's plan against summaries written where its probes would have put them.
fn plan_after_probes(id: &str, inputs: &[&str], probes: &[&str]) -> Vec<Step> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("remise-plan-{id}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    for (i, text) in probes.iter().enumerate() {
        std::fs::write(tmp.join(format!("probe-{i}.txt")), text).unwrap();
    }
    let t = find(id).unwrap();
    let j = Job {
        inputs: inputs.iter().map(PathBuf::from).collect(),
        out: PathBuf::from("/v/out.mp4"),
        tmp: tmp.clone(),
        params: with_defaults(t, Default::default()),
    };
    let steps = (t.build)(&j);
    let Some(Step::Then(plan)) = steps.last() else { panic!("{steps:?}") };
    let next = plan(&j).unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    next
}

#[test]
fn matching_clips_join_losslessly_with_quoted_names() {
    let steps = plan_after_probes("vid-join", &["/v/it's.mp4", "/v/b.mp4"], &[WITH_SOUND, WITH_SOUND]);
    let Step::Write(_, body) = &steps[0] else { panic!("{steps:?}") };
    assert_eq!(body, "file '/v/it'\\''s.mp4'\nfile '/v/b.mp4'\n");
    assert!(run_args(&steps[1]).1.contains(&"copy"));
}

#[test]
fn mixed_clips_are_fitted_to_the_first_and_get_silence() {
    let steps = plan_after_probes("vid-join", &["/v/a.mp4", "/v/b.mp4"], &[WITH_SOUND, SILENT]);
    let (_, a) = run_args(&steps[0]);
    let fc = a[a.iter().position(|x| *x == "-filter_complex").unwrap() + 1];
    assert!(fc.contains("pad=320:240") && fc.contains("fps=25"), "{fc}");
    assert!(a.contains(&"anullsrc=r=48000:cl=stereo") && fc.contains("[2:a]"), "the silent clip's sound is input 2: {fc}");
    assert!(fc.ends_with("concat=n=2:v=1:a=1[v][a]"));
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
