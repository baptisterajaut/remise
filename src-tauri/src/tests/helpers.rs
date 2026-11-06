//! The pure helpers behind the newer tools.

use crate::tools::{self, epub, parse_areas, parse_date, ps_str, pdf, Job, Step};
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
fn epub_reading_order_and_merge() {
    let container = r#"<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#;
    assert_eq!(epub::rootfile(container).as_deref(), Some("OEBPS/content.opf"));
    let opf = r#"<manifest><item id="b" href="two.xhtml"/><item id='a' href='one%20a.xhtml'/></manifest>
                 <spine><itemref idref="a"/><itemref idref="b"/></spine>"#;
    assert_eq!(epub::spine(opf), ["one a.xhtml", "two.xhtml"]);
    let merged = epub::merge(&[
        r#"<html><head><link rel="stylesheet" href="s.css"/></head><body class="x"><p>One</p></body></html>"#.into(),
        "<html><body><p>Two</p></body></html>".into(),
    ]);
    assert!(merged.contains(r#"<link rel="stylesheet" href="s.css"/>"#));
    assert!(merged.find("One").unwrap() < merged.find("Two").unwrap());
    assert_eq!(merged.matches("break-after: page").count(), 2);
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
fn image_text_lets_tesseract_add_the_extension() {
    let steps = job("img-text", &["/d/shot.png"], &[("lang", "fra")]);
    let Step::Run("tesseract", a) = &steps[0] else { panic!("{steps:?}") };
    assert_eq!(a, &["/d/shot.png", "/out/x", "-l", "fra", "txt"]);
}
