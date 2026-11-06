//! PDF tools.

use super::*;

pub static TOOLS: &[Tool] = &[
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
];
