//! Documents through Pandoc. PDFs are typeset by Typst (one small binary, no LaTeX install);
//! Pandoc finds it on PATH, where the bundled copy is.

use super::*;

/// What Pandoc reads well, by extension.
const DOCS: &[&str] = &["docx", "odt", "md", "markdown", "html", "htm", "epub", "rtf", "rst", "tex", "org", "ipynb", "textile"];
const MARKDOWN: &[&str] = &["md", "markdown"];

/// `pandoc <input> <extra…> -o <out>`; Typst does the PDF typesetting.
/// Always in `--sandbox`: an HTML `<img src=/abs/path>`, a LaTeX `\input`, an RST include or a
/// YAML `css:` would otherwise pull local files into the output. `safe-images.lua` then lets the
/// document's own images back in (see the filter for what counts as its own).
pub fn pandoc(j: &Job, extra: Vec<String>) -> Vec<Step> {
    let filter = j.tmp.join("safe-images.lua");
    let mut a = args![j.input(), "--sandbox", format!("--lua-filter={}", s(&filter))];
    a.extend(extra);
    if j.out.extension().is_some_and(|e| e == "pdf") {
        a.extend(args!["--pdf-engine=typst"]);
    }
    a.extend(args!["--standalone", "-o", j.out()]);
    vec![Step::Write(filter, include_str!("safe-images.lua").into()), Step::Run("pandoc", a)]
}

/// Page setup for Typst PDFs.
const PAGE: &[Param] = &[
    Param::Select { key: "paper", label: "Paper", options: &[("a4", "A4"), ("us-letter", "US Letter"), ("a5", "A5")], default: "a4" },
    Param::Select { key: "size", label: "Text size", options: &[("10pt", "10 pt"), ("11pt", "11 pt"), ("12pt", "12 pt")], default: "11pt" },
];

fn page_vars(j: &Job) -> Vec<String> {
    args!["-V", format!("papersize={}", j.p("paper")), "-V", format!("fontsize={}", j.p("size"))]
}

pub static TOOLS: &[Tool] = &[
    Tool {
        id: "doc-convert",
        family: Family::Docs,
        name: "Convert document",
        blurb: "Word, OpenDocument, HTML, EPUB, Markdown, LaTeX… into each other.",
        bins: &["pandoc"],
        accepts: DOCS,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "format",
            label: "To",
            options: &[
                ("docx", "Word"),
                ("odt", "OpenDocument"),
                ("pdf", "PDF"),
                ("html", "Web page"),
                ("epub", "EPUB"),
                ("rtf", "RTF"),
                ("txt", "Plain text"),
            ],
            default: "docx",
        }],
        output: Output::File { suffix: "converted", ext: fmt },
        build: |j| {
            let extra = match j.p("format") {
                "txt" => args!["-t", "plain"],
                "html" => args!["--embed-resources"],
                _ => args![],
            };
            pandoc(j, extra)
        },
    },
    Tool {
        id: "doc-to-markdown",
        family: Family::Docs,
        name: "To Markdown",
        blurb: "Word, web pages, EPUB… as clean Markdown, images in a media folder.",
        bins: &["pandoc"],
        accepts: &["docx", "odt", "html", "htm", "epub", "rtf", "rst", "tex", "org", "ipynb", "textile"],
        inputs: Inputs::One,
        params: &[],
        output: Output::Dir("markdown"),
        // From inside the output folder, so the image links stay relative (media/…).
        build: |j| {
            vec![Step::RunIn(
                j.out.clone(),
                "pandoc",
                args![j.input(), "--sandbox", "-t", "gfm", "--wrap=none", "--extract-media=media", "-o", format!("{}.md", j.stem())],
            )]
        },
    },
    Tool {
        id: "doc-md-pdf",
        family: Family::Docs,
        name: "Markdown to PDF",
        blurb: "Typeset Markdown into a clean PDF. No LaTeX needed.",
        bins: &["pandoc", "typst"],
        accepts: MARKDOWN,
        inputs: Inputs::One,
        params: PAGE,
        output: Output::File { suffix: "typeset", ext: |_| "pdf".into() },
        build: |j| pandoc(j, page_vars(j)),
    },
    Tool {
        id: "doc-md-docx",
        family: Family::Docs,
        name: "Markdown to Word",
        blurb: "A .docx with real headings, lists and tables.",
        bins: &["pandoc"],
        accepts: MARKDOWN,
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "doc", ext: |_| "docx".into() },
        build: |j| pandoc(j, args![]),
    },
    Tool {
        id: "doc-md-slides",
        family: Family::Docs,
        name: "Markdown to slides",
        blurb: "A PowerPoint deck: each heading starts a slide.",
        bins: &["pandoc"],
        accepts: MARKDOWN,
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "slides", ext: |_| "pptx".into() },
        build: |j| pandoc(j, args![]),
    },
    Tool {
        id: "doc-pdf-ebook",
        family: Family::Docs,
        name: "PDF to Ebook",
        blurb: "A reflowable EPUB from a PDF's text, for any e-reader.",
        bins: &["gs", "pandoc"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "ebook", ext: |_| "epub".into() },
        // CBA: text only (from the PDF's text layer); images and columns don't survive
        build: |j| {
            vec![
                Step::Run(
                    "gs",
                    args![
                        "-sDEVICE=txtwrite",
                        "-dNOPAUSE",
                        "-dBATCH",
                        "-dQUIET",
                        "-dPDFSTOPONERROR",
                        gs_out(&j.tmp("page-%04d.txt")),
                        j.input()
                    ],
                ),
                Step::Then(|j| {
                    let md = j.tmp.join("book.md");
                    Ok(vec![
                        Step::Write(md.clone(), super::pdf::to_markdown(&j.stem(), &super::pdf::text_pages(j)?)),
                        Step::Run("pandoc", args![s(&md), "--sandbox", "--metadata", format!("title={}", j.stem()), "-o", j.out()]),
                    ])
                }),
            ]
        },
    },
];
