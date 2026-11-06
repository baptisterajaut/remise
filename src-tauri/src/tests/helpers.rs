//! The pure helpers behind the newer tools.

use crate::tools::{self, Job, Step, parse_areas, parse_date, pdf, ps_str};
use std::collections::HashMap;
use std::path::PathBuf;

fn job(tool: &str, inputs: &[&str], params: &[(&str, &str)]) -> Vec<Step> {
    let t = tools::find(tool).unwrap();
    let p = params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>();
    let j = Job {
        inputs: inputs.iter().map(PathBuf::from).collect(),
        out: PathBuf::from("/out/x"),
        tmp: PathBuf::from("/tmp/job"),
        params: tools::with_defaults(t, p),
    };
    (t.build)(&j)
}

#[test]
fn areas_parse_and_reject() {
    assert_eq!(parse_areas("5,5,40,8; 0,90,100,10").unwrap(), vec![[5.0, 5.0, 40.0, 8.0], [0.0, 90.0, 100.0, 10.0]]);
    assert!(parse_areas("5,5,40").is_err());
    assert!(parse_areas("5,5,140,8").is_err());
    assert!(parse_areas("").is_err());
}

#[test]
fn postscript_strings_are_escaped() {
    assert_eq!(ps_str(r"a(b)c\d"), r"(a\(b\)c\\d)");
}

#[test]
fn dates_round_trip_through_month_folders() {
    let t = parse_date("2024-02-29 13:45").unwrap();
    let secs = t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    assert_eq!(secs, 1_709_214_300);
    assert_eq!(tools::files::month_of(t), "2024-02");
    assert_eq!(parse_date("1970-01-01"), Some(std::time::UNIX_EPOCH));
    assert!(parse_date("2024-13-01").is_none() && parse_date("tomorrow").is_none());
}

#[test]
fn slugs_fold_accents_and_punctuation() {
    assert_eq!(tools::image::slugify("Mon Été 2024 — Plage!"), "mon-ete-2024-plage");
    assert_eq!(tools::image::slugify("%%%"), "image");
}

#[test]
fn files_are_sorted_by_kind() {
    assert_eq!(tools::files::kind_of("jpg"), "Images");
    assert_eq!(tools::files::kind_of("mkv"), "Videos");
    assert_eq!(tools::files::kind_of("docx"), "Documents");
    assert_eq!(tools::files::kind_of("7z"), "Archives");
    assert_eq!(tools::files::kind_of("xyz"), "Other");
}

#[test]
fn markdown_has_a_section_per_page_and_tidy_lines() {
    let md = pdf::to_markdown("doc", &["  Hello    world  \n\n\n\nBye".into(), "Second".into()]);
    assert_eq!(md, "# doc\n\n## Page 1\n\nHello world\n\nBye\n\n## Page 2\n\nSecond\n");
}

#[test]
fn qr_codes_are_svg() {
    let svg = tools::create::qr_svg("https://example.org").unwrap();
    assert!(svg.contains("<svg") && svg.contains("</svg>"));
    assert!(tools::create::qr_svg("").is_err());
}

/// gs exits 0 even when it drops a page it couldn't draw. Every gs run that reads a user's
/// PDF must stop on error, except Repair, whose whole job is to get past damage.
#[test]
fn every_ghostscript_run_fails_loudly_except_repair() {
    for t in tools::all().filter(|t| t.bins.contains(&"gs") && t.id != "pdf-repair") {
        let input = if t.accepts.contains(&"pdf") { "/d/a.pdf" } else { "/d/a.png" };
        let steps = job(t.id, &[input], &[("areas", "1,1,10,10"), ("password", "x")]);
        for step in &steps {
            if let Step::Run("gs", a) = step {
                assert!(a.iter().any(|x| x == "-dPDFSTOPONERROR"), "{}: {a:?}", t.id);
            }
        }
    }
}

#[test]
fn ghostscript_overlays_fail_loudly() {
    let steps = job("pdf-page-numbers", &["/d/a.pdf"], &[]);
    let Step::Run("gs", a) = &steps[0] else { panic!("{steps:?}") };
    assert!(a.contains(&"-dPDFSTOPONERROR".to_string()));
    assert!(a.iter().any(|x| x.contains("{ false } ifelse")), "EndPage must answer false off showpage");
}

#[test]
fn mixed_input_tools_explain_what_is_missing() {
    let steps = job("pdf-sign", &["/d/a.pdf"], &[]);
    assert!(matches!(&steps[0], Step::Fail(m) if m.contains("signature")));
    let steps = job("vid-add-audio", &["/d/a.mp4"], &[]);
    assert!(matches!(&steps[0], Step::Fail(m) if m.contains("audio")));
}

#[test]
fn bad_params_become_messages_not_commands() {
    assert!(matches!(&job("pdf-redact", &["/d/a.pdf"], &[("areas", "oops")])[0], Step::Fail(_)));
    assert!(matches!(&job("file-dates", &["/d/a"], &[("date", "soon")])[0], Step::Fail(_)));
    assert!(matches!(&job("vid-download", &[], &[("url", "file:///etc/passwd")])[0], Step::Fail(_)));
}

#[test]
fn attach_to_each_puts_the_page_where_asked() {
    let end = job("pdf-append", &["/d/terms.pdf", "/d/a.pdf"], &[]);
    let Step::Run("qpdf", a) = &end[0] else { panic!("{end:?}") };
    assert_eq!(a[..4], ["--empty", "--pages", "/d/a.pdf", "/d/terms.pdf"]);
    let start = job("pdf-append", &["/d/cover.pdf", "/d/a.pdf"], &[("where", "start")]);
    let Step::Run("qpdf", a) = &start[0] else { panic!("{start:?}") };
    assert_eq!(a[2..4], ["/d/cover.pdf", "/d/a.pdf"]);
    assert!(matches!(&job("pdf-append", &["/d/only.pdf"], &[])[0], Step::Fail(_)));
}

#[test]
fn image_text_reads_the_image_as_a_300_dpi_page() {
    let steps = job("img-text", &["/d/shot.png"], &[("lang", "fra")]);
    let [Step::Run("magick", m), Step::Run("gs", g)] = &steps[..] else { panic!("{steps:?}") };
    assert_eq!(m[1..5], ["-units", "PixelsPerInch", "-density", "300"]);
    assert!(g.contains(&"-sDEVICE=ocr".to_string()) && g.contains(&"-sOCRLanguage=fra".to_string()));
    assert_eq!(g.last(), m.last(), "gs reads the page magick wrote");
}

#[test]
fn pandoc_typesets_pdf_with_typst_and_keeps_markdown_images_relative() {
    let pdf = {
        let t = tools::find("doc-convert").unwrap();
        let j = Job {
            inputs: vec![PathBuf::from("/d/notes/a.docx")],
            out: PathBuf::from("/out/a.converted.pdf"),
            tmp: PathBuf::from("/tmp/job"),
            params: tools::with_defaults(t, [("format".to_string(), "pdf".to_string())].into()),
        };
        (t.build)(&j)
    };
    let Step::Run("pandoc", a) = &pdf[1] else { panic!("{pdf:?}") };
    assert!(a.contains(&"--pdf-engine=typst".to_string()));

    let md = job("doc-to-markdown", &["/d/a.docx"], &[]);
    let Step::RunIn(dir, "pandoc", a) = &md[0] else { panic!("{md:?}") };
    assert_eq!(dir, &PathBuf::from("/out/x"));
    assert!(a.contains(&"--extract-media=media".to_string()) && a.contains(&"a.md".to_string()));
}

#[test]
fn every_pandoc_run_is_sandboxed_with_the_image_filter() {
    for (id, input) in
        [("doc-convert", "/d/page.html"), ("doc-md-docx", "/d/notes.md"), ("doc-md-pdf", "/d/notes.md"), ("pdf-ebook", "/d/book.epub")]
    {
        let steps = job(id, &[input], &[]);
        let [Step::Write(filter, lua), Step::Run("pandoc", a)] = &steps[..] else { panic!("{id}: {steps:?}") };
        assert!(lua.contains("mediabag"), "{id}");
        assert!(a.contains(&"--sandbox".to_string()), "{id}");
        assert!(a.contains(&format!("--lua-filter={}", tools::s(filter))), "{id}");
    }
}

#[test]
fn empty_password_is_refused() {
    assert!(matches!(&job("pdf-encrypt", &["/d/a.pdf"], &[("password", "")])[0], Step::Fail(_)));
    // Never on the command line, where `ps` shows it.
    for (id, pw) in [("pdf-encrypt", "s3cret"), ("pdf-unlock", "s3cret")] {
        let steps = job(id, &["/d/a.pdf"], &[("password", pw)]);
        let [Step::Write(_, body), Step::Run("qpdf", a)] = &steps[..] else { panic!("{steps:?}") };
        assert!(body.contains(pw) && !a.iter().any(|x| x.contains(pw)), "{id}: {a:?}");
    }
}

#[test]
fn batch_rename_refuses_paths_and_name_clashes() {
    for pattern in ["../x", "a/b", "sub\\x", ""] {
        assert!(matches!(&job("batch-rename", &["/d/a.txt"], &[("pattern", pattern)])[0], Step::Fail(_)), "{pattern}");
    }
    assert!(matches!(&job("batch-rename", &["/d/a.txt", "/d/b.txt"], &[("pattern", "same")])[0], Step::Fail(_)));
}

#[test]
fn same_names_from_different_folders_stay_distinct() {
    let names = tools::distinct_names(&[PathBuf::from("/a/x.pdf"), PathBuf::from("/b/x.pdf"), PathBuf::from("/c/X.PDF")]);
    assert_eq!(names, ["x.pdf", "x-2.pdf", "X-3.PDF"]);
}

#[test]
fn file_dates_are_local_time() {
    let at = |tz: &str| {
        let steps = job("file-dates", &["/d/a.txt"], &[("date", "2024-01-01 12:00"), ("tz", tz)]);
        let Step::Touch(_, when) = steps[0] else { panic!("{steps:?}") };
        when.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
    };
    let noon_utc = 1_704_110_400;
    assert_eq!(at("0"), noon_utc);
    assert_eq!(at("60"), noon_utc - 3600, "Paris in winter: noon there is 11:00 UTC");
    assert_eq!(at("-300"), noon_utc + 5 * 3600, "New York: noon there is 17:00 UTC");
}
