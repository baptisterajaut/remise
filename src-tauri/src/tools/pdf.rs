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
        inputs: Inputs::One,
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
                    "-dPDFSTOPONERROR",
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
        inputs: Inputs::Many,
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
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "every",
            label: "Pages per file",
            options: &[("1", "1"), ("2", "2"), ("5", "5"), ("10", "10")],
            default: "1",
        }],
        output: Output::Dir("split"),
        // qpdf has no %% escape and substitutes the first "%d" anywhere in the path: split under a
        // neutral scratch name, then move the parts into place.
        build: |j| {
            vec![
                Step::Run("qpdf", args![format!("--split-pages={}", j.p("every")), j.input(), j.tmp("part-%d.pdf")]),
                Step::Then(|j| {
                    let mut parts: Vec<_> = std::fs::read_dir(&j.tmp)
                        .map_err(|e| e.to_string())?
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| ext_of(p) == "pdf")
                        .collect();
                    parts.sort();
                    Ok(parts
                        .into_iter()
                        .map(|p| {
                            let n = stem_of(&p).trim_start_matches("part-").to_string();
                            Step::Rename(p, j.out.join(format!("{}-{n}.pdf", j.stem())))
                        })
                        .collect())
                }),
            ]
        },
    },
    Tool {
        id: "pdf-extract",
        family: Family::Pdf,
        name: "Extract pages",
        blurb: "Keep only the pages you list.",
        bins: &["qpdf"],
        accepts: PDF,
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        build: |j| vec![Step::Run("qpdf", args![j.input(), j.out(), format!("--rotate={}:{}", j.p("angle"), j.p("pages"))])],
    },
    Tool {
        id: "pdf-ocr",
        family: Family::Pdf,
        name: "OCR",
        blurb: "Turn scans and photos into a searchable PDF.",
        bins: &["gs", "magick"],
        accepts: PDF_IMG,
        inputs: Inputs::One,
        params: &[Param::Text { key: "lang", label: "Languages", default: "eng", placeholder: "eng+fra" }],
        output: Output::File { suffix: "ocr", ext: |_| "pdf".into() },
        build: |j| ocr(j, "pdfocr24"),
    },
    Tool {
        id: "pdf-to-images",
        family: Family::Pdf,
        name: "PDF to images",
        blurb: "Every page as a PNG or JPG.",
        bins: &["gs"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[
            Param::Select { key: "format", label: "Format", options: &[("png16m", "PNG"), ("jpeg", "JPG")], default: "png16m" },
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
                    "-dPDFSTOPONERROR",
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
        inputs: Inputs::Many,
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
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "text", ext: |_| "txt".into() },
        build: |j| {
            vec![Step::Run(
                "gs",
                args!["-sDEVICE=txtwrite", "-dNOPAUSE", "-dBATCH", "-dQUIET", "-dPDFSTOPONERROR", gs_out(&j.out()), j.input()],
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
        inputs: Inputs::One,
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
                    "-dPDFSTOPONERROR",
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
        inputs: Inputs::One,
        params: &[Param::Text { key: "password", label: "Password", default: "", placeholder: "required" }],
        output: Output::File { suffix: "locked", ext: |_| "pdf".into() },
        // The password goes through a file in the job's private scratch folder, never argv,
        // which any user can read in `ps` while qpdf runs.
        build: |j| {
            let pw = j.p("password");
            if pw.is_empty() {
                // qpdf would happily "encrypt" with an empty password: a lock anyone opens.
                return vec![Step::Fail("choose a password".into())];
            }
            if pw.contains(['\n', '\r']) {
                return vec![Step::Fail("a password can't span several lines".into())];
            }
            let argfile = j.tmp.join("encrypt.args");
            vec![
                Step::Write(argfile.clone(), format!("--encrypt\n{pw}\n{pw}\n256\n--\n")),
                Step::Run("qpdf", args![format!("@{}", s(&argfile)), j.input(), j.out()]),
            ]
        },
    },
    Tool {
        id: "pdf-unlock",
        family: Family::Pdf,
        name: "Unlock",
        blurb: "Remove the password (you need to know it).",
        bins: &["qpdf"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[Param::Text { key: "password", label: "Password", default: "", placeholder: "current password" }],
        output: Output::File { suffix: "unlocked", ext: |_| "pdf".into() },
        build: |j| {
            let pw = j.tmp.join("password");
            vec![
                Step::Write(pw.clone(), j.p("password").to_string()),
                Step::Run("qpdf", args![format!("--password-file={}", s(&pw)), "--decrypt", j.input(), j.out()]),
            ]
        },
    },
    Tool {
        id: "pdf-web",
        family: Family::Pdf,
        name: "Optimize for web",
        blurb: "Linearize so browsers show page 1 before the download ends.",
        bins: &["qpdf"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "web", ext: |_| "pdf".into() },
        build: |j| vec![Step::Run("qpdf", args!["--linearize", "--object-streams=generate", j.input(), j.out()])],
    },
    Tool {
        id: "pdf-pdfa",
        family: Family::Pdf,
        name: "PDF/A",
        blurb: "The archival PDF that courts, tax offices and tenders ask for.",
        bins: &["gs"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "level",
            label: "Version",
            options: &[("2", "PDF/A-2b"), ("1", "PDF/A-1b"), ("3", "PDF/A-3b")],
            default: "2",
        }],
        output: Output::File { suffix: "pdfa", ext: |_| "pdf".into() },
        build: |j| {
            let def = j.tmp.join("PDFA_def.ps");
            vec![
                Step::Write(def.clone(), PDFA_DEF.into()),
                Step::Run(
                    "gs",
                    args![
                        format!("-dPDFA={}", j.p("level")),
                        // What can't comply (transparency in A-1…) is converted or dropped,
                        // never left in to make a file that only claims to be PDF/A.
                        "-dPDFACompatibilityPolicy=1",
                        "-sColorConversionStrategy=RGB",
                        "-sDEVICE=pdfwrite",
                        "-dNOPAUSE",
                        "-dBATCH",
                        "-dQUIET",
                        "-dPDFSTOPONERROR",
                        gs_out(&j.out()),
                        s(&def),
                        j.input()
                    ],
                ),
            ]
        },
    },
    Tool {
        id: "pdf-studio",
        family: Family::Pdf,
        name: "PDF Studio",
        blurb: "Annotate, sign, fill forms, reorder: opens in PdfCraft.",
        bins: &["pdfcraft"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[],
        output: Output::None,
        build: |j| vec![Step::Launch("pdfcraft", args![j.input()])],
    },
    Tool {
        id: "pdf-organize",
        family: Family::Pdf,
        name: "Organize pages",
        blurb: "Reorder or drop pages: 3,1,2,4-z or 1-z,x5.",
        bins: &["qpdf"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[Param::Text { key: "order", label: "New order", default: "1-z", placeholder: "3,1,2,4-z  ·  1-z,x5 drops page 5" }],
        output: Output::File { suffix: "organized", ext: |_| "pdf".into() },
        build: |j| vec![Step::Run("qpdf", args![j.input(), "--pages", ".", j.p("order"), "--", j.out()])],
    },
    Tool {
        id: "pdf-page-numbers",
        family: Family::Pdf,
        name: "Page numbers",
        blurb: "Number every page, bottom of the page.",
        bins: &["gs"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[
            Param::Select {
                key: "pos",
                label: "Position",
                options: &[("center", "Centre"), ("right", "Right"), ("left", "Left")],
                default: "center",
            },
            Param::Select {
                key: "style",
                label: "Style",
                options: &[("plain", "3"), ("page", "Page 3"), ("dash", "– 3 –")],
                default: "plain",
            },
        ],
        output: Output::File { suffix: "numbered", ext: |_| "pdf".into() },
        build: |j| {
            let (pre, post) = match j.p("style") {
                "page" => ("(Page ) show ", ""),
                "dash" => ("(- ) show ", " ( -) show"),
                _ => ("", ""),
            };
            let label = format!("{pre}N 10 string cvs show{post}");
            // Measure first (gsave/grestore keeps the pen still), then place.
            let x = match j.p("pos") {
                "right" => "W 36 sub WID sub",
                "left" => "36",
                _ => "W WID sub 2 div",
            };
            let ps = format!(
                "/Helvetica findfont 10 scalefont setfont gsave nulldevice 0 0 moveto {label} currentpoint pop /WID exch def grestore \
                 {x} 24 moveto 0.2 setgray {label}"
            );
            vec![gs_overlay(j, "pdfwrite", &[], &ps)]
        },
    },
    Tool {
        id: "pdf-stamp",
        family: Family::Pdf,
        name: "Stamp",
        blurb: "A diagonal word across every page: DRAFT, PAID, COPY…",
        bins: &["gs"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[
            Param::Text { key: "text", label: "Text", default: "DRAFT", placeholder: "CONFIDENTIAL" },
            Param::Select {
                key: "color",
                label: "Ink",
                // Light tints rather than transparency: .setfillconstantalpha errors out in gs 10.08.
                options: &[("0.95 0.62 0.58", "Red"), ("0.6 0.72 0.95", "Blue"), ("0.72 0.72 0.72", "Grey")],
                default: "0.95 0.62 0.58",
            },
        ],
        output: Output::File { suffix: "stamped", ext: |_| "pdf".into() },
        build: |j| {
            let ps = format!(
                "/Helvetica-Bold findfont W 9 div scalefont setfont W 2 div H 2 div translate H W atan rotate \
                 {} dup stringwidth pop 2 div neg W 30 div neg moveto {} setrgbcolor show",
                ps_str(j.p("text")),
                j.p("color")
            );
            vec![gs_overlay(j, "pdfwrite", &[], &ps)]
        },
    },
    Tool {
        id: "pdf-redact",
        family: Family::Pdf,
        name: "Redact",
        blurb: "Black out areas for real: pages are flattened, nothing hides underneath.",
        bins: &["gs"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[Param::Text {
            key: "areas",
            label: "Areas (x,y,w,h in % from top-left; ';' between)",
            default: "0,0,100,12",
            placeholder: "5,5,40,8; 0,90,100,10",
        }],
        output: Output::File { suffix: "redacted", ext: |_| "pdf".into() },
        build: |j| match parse_areas(j.p("areas")) {
            Err(e) => vec![Step::Fail(e)],
            Ok(areas) => {
                let rects: String = areas
                    .iter()
                    .map(|[x, y, w, h]| {
                        format!("W {x} mul 100 div H {} mul 100 div W {w} mul 100 div H {h} mul 100 div rectfill ", 100.0 - y - h)
                    })
                    .collect();
                // Rasterising (pdfimage24) is what makes it a redaction and not a black box over live text.
                vec![gs_overlay(j, "pdfimage24", &["-r150"], &format!("0 setgray {rects}"))]
            }
        },
    },
    Tool {
        id: "pdf-sign",
        family: Family::Pdf,
        name: "Sign",
        blurb: "Drop a PDF and a signature image (transparent PNG is best).",
        bins: &["gs", "magick", "qpdf"],
        accepts: &["pdf", "png", "jpg", "jpeg", "webp"],
        inputs: Inputs::Many,
        params: &[
            Param::Text { key: "pages", label: "On pages", default: "z", placeholder: "z = last, 1 = first, 1-z = all" },
            Param::Select {
                key: "corner",
                label: "Corner",
                options: &[("southeast", "Bottom right"), ("southwest", "Bottom left"), ("south", "Bottom centre")],
                default: "southeast",
            },
            Param::Select {
                key: "width",
                label: "Size",
                options: &[("300", "Small"), ("434", "Medium"), ("620", "Large")],
                default: "434",
            },
        ],
        output: Output::File { suffix: "signed", ext: |_| "pdf".into() },
        // CBA: the layer is cut to page 1's size; qpdf scales it onto pages of another size
        build: |j| match pair(j, &["pdf"], &["png", "jpg", "jpeg", "webp"], "a PDF and a signature image") {
            Err(fail) => vec![fail],
            Ok((pdf, _)) => vec![
                Step::Capture {
                    bin: "magick",
                    args: args!["identify", "-density", "72", "-format", "%w %h\n", format!("{pdf}[0]")],
                    into: j.tmp.join("size.txt"),
                },
                Step::Then(sign_plan),
            ],
        },
    },
    Tool {
        id: "pdf-crop",
        family: Family::Pdf,
        name: "Crop margins",
        blurb: "Trim the same margin off every page. Vectors stay vectors.",
        bins: &["gs", "magick"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "margin",
            label: "Trim each side",
            options: &[("3", "3 %"), ("5", "5 %"), ("10", "10 %"), ("15", "15 %")],
            default: "5",
        }],
        output: Output::File { suffix: "cropped", ext: |_| "pdf".into() },
        // CBA: uses page 1's size for every page; per-page CropBox if mixed sizes matter
        build: |j| {
            vec![
                Step::Capture {
                    bin: "magick",
                    args: args!["identify", "-density", "72", "-format", "%w %h\n", format!("{}[0]", j.input())],
                    into: j.tmp.join("size.txt"),
                },
                Step::Then(|j| {
                    let size = j.read_tmp("size.txt")?;
                    let mut wh = size.split_whitespace().filter_map(|v| v.parse::<f64>().ok());
                    let (w, h) = (wh.next().ok_or("unreadable page size")?, wh.next().ok_or("unreadable page size")?);
                    let m: f64 = j.p("margin").parse().unwrap_or(5.0) / 100.0;
                    let (dx, dy) = (w * m, h * m);
                    Ok(vec![Step::Run(
                        "gs",
                        args![
                            "-sDEVICE=pdfwrite",
                            "-dFIXEDMEDIA",
                            format!("-dDEVICEWIDTHPOINTS={:.0}", w - 2.0 * dx),
                            format!("-dDEVICEHEIGHTPOINTS={:.0}", h - 2.0 * dy),
                            "-dNOPAUSE",
                            "-dBATCH",
                            "-dQUIET",
                            "-dPDFSTOPONERROR",
                            gs_out(&j.out()),
                            "-c",
                            format!("<< /PageOffset [{:.1} {:.1}] >> setpagedevice", -dx, -dy),
                            "-f",
                            j.input()
                        ],
                    )])
                }),
            ]
        },
    },
    Tool {
        id: "pdf-markdown",
        family: Family::Pdf,
        name: "PDF to Markdown",
        blurb: "Text layer as Markdown, one section per page.",
        bins: &["gs"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "text", ext: |_| "md".into() },
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
                Step::Then(|j| Ok(vec![Step::Write(j.out.clone(), to_markdown(&j.stem(), &text_pages(j)?))])),
            ]
        },
    },
    Tool {
        id: "pdf-ebook",
        family: Family::Pdf,
        name: "Ebook to PDF",
        blurb: "EPUB to a printable PDF, typeset by Typst.",
        bins: &["pandoc", "typst"],
        accepts: &["epub"],
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "book", ext: |_| "pdf".into() },
        build: |j| super::docs::pandoc(j, args![]),
    },
    Tool {
        id: "pdf-html",
        family: Family::Pdf,
        name: "HTML to PDF",
        blurb: "A web page file printed to PDF by your browser.",
        bins: &["chromium"],
        accepts: HTML,
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "print", ext: |_| "pdf".into() },
        build: |j| chromium(j, j.input(), args!["--no-pdf-header-footer", format!("--print-to-pdf={}", j.out())]),
    },
    Tool {
        id: "pdf-append",
        family: Family::Pdf,
        name: "Attach to each",
        blurb: "Drop a cover or terms page first, then PDFs: it's added to every one.",
        bins: &["qpdf"],
        accepts: PDF,
        inputs: Inputs::Many,
        params: &[Param::Select {
            key: "where",
            label: "Put it",
            options: &[("end", "At the end"), ("start", "At the start")],
            default: "end",
        }],
        output: Output::Dir("attached"),
        build: |j| {
            let Some((extra, docs)) = j.inputs.split_first().filter(|(_, rest)| !rest.is_empty()) else {
                return vec![Step::Fail("drop the page to attach first, then one or more PDFs".into())];
            };
            docs.iter()
                .map(|d| {
                    let (a, b) = if j.p("where") == "start" { (s(extra), s(d)) } else { (s(d), s(extra)) };
                    (a, b)
                })
                .zip(distinct_names(docs))
                .map(|((a, b), name)| Step::Run("qpdf", args!["--empty", "--pages", a, b, "--", s(&j.out.join(name))]))
                .collect()
        },
    },
    Tool {
        id: "pdf-flatten",
        family: Family::Pdf,
        name: "Flatten PDF",
        blurb: "Bake form fields and annotations into the page.",
        bins: &["qpdf"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "flat", ext: |_| "pdf".into() },
        build: |j| vec![Step::Run("qpdf", args!["--generate-appearances", "--flatten-annotations=all", j.input(), j.out()])],
    },
    Tool {
        id: "pdf-repair",
        family: Family::Pdf,
        name: "Repair PDF",
        blurb: "Rebuild a damaged PDF so it opens cleanly again.",
        bins: &["gs"],
        accepts: PDF,
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "repaired", ext: |_| "pdf".into() },
        // Ghostscript re-renders every page into a fresh file, which survives far worse damage than a qpdf rewrite.
        build: |j| vec![Step::Run("gs", args!["-sDEVICE=pdfwrite", "-dNOPAUSE", "-dBATCH", "-dQUIET", gs_out(&j.out()), j.input()])],
    },
    Tool {
        id: "pdf-office",
        family: Family::Pdf,
        name: "Office to PDF",
        blurb: "Word, Excel, PowerPoint and LibreOffice files to PDF, laid out by LibreOffice.",
        bins: &["soffice"],
        accepts: &["doc", "docx", "odt", "rtf", "xls", "xlsx", "ods", "csv", "ppt", "pptx", "odp"],
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "office", ext: |_| "pdf".into() },
        build: |j| {
            let profile =
                format!("-env:UserInstallation=file:///{}", j.tmp("lo").replace('\\', "/").replace(' ', "%20").trim_start_matches('/'));
            vec![
                Step::Run("soffice", args![profile, "--headless", "--convert-to", "pdf", "--outdir", s(&j.tmp), j.input()]),
                Step::Rename(j.tmp.join(format!("{}.pdf", j.stem())), j.out.clone()),
            ]
        },
    },
];

/// The `page-NNNN.txt` files gs txtwrite left in tmp, in page order.
pub fn text_pages(j: &Job) -> Result<Vec<String>, String> {
    let mut pages: Vec<_> =
        std::fs::read_dir(&j.tmp).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).filter(|p| ext_of(p) == "txt").collect();
    pages.sort();
    pages.iter().map(|p| std::fs::read_to_string(p).map_err(|e| e.to_string())).collect()
}

/// Pages of plain text (gs txtwrite keeps the layout with runs of spaces) → Markdown.
pub fn to_markdown(title: &str, pages: &[String]) -> String {
    let mut md = format!("# {title}\n");
    for (i, page) in pages.iter().enumerate() {
        md.push_str(&format!("\n## Page {}\n\n", i + 1));
        let mut blank = 0;
        for line in page.lines() {
            let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
            blank = if line.is_empty() { blank + 1 } else { 0 };
            if blank <= 1 {
                md.push_str(&line);
                md.push('\n');
            }
        }
    }
    md
}

/// Ghostscript's built-in Tesseract (`pdfocr24`: a searchable PDF, `ocr`: plain text), pages
/// rendered at 300 dpi. An image first becomes a one-page PDF at 300 dpi, so it is read 1:1.
pub fn ocr(j: &Job, device: &str) -> Vec<Step> {
    let mut steps = vec![];
    let mut pdf = j.input();
    if j.in_ext() != "pdf" {
        pdf = j.tmp("scan.pdf");
        steps.push(Step::Run("magick", args![j.input(), "-units", "PixelsPerInch", "-density", "300", pdf.clone()]));
    }
    let a = args![
        format!("-sDEVICE={device}"),
        "-r300",
        format!("-sOCRLanguage={}", j.p("lang")),
        "-dNOPAUSE",
        "-dBATCH",
        "-dQUIET",
        "-dPDFSTOPONERROR",
        gs_out(&j.out()),
        pdf
    ];
    steps.push(Step::Run("gs", a));
    steps
}

/// A transparent page the size of page 1 (at 150 dpi) with the signature in a corner, laid
/// over the chosen pages.
fn sign_plan(j: &Job) -> Result<Vec<Step>, String> {
    let (pdf, sig) = match pair(j, &["pdf"], &["png", "jpg", "jpeg", "webp"], "a PDF and a signature image") {
        Ok(p) => p,
        Err(fail) => return Ok(vec![fail]),
    };
    let (w, h) = size_of(j)?;
    let px = |pt: f64| (pt / 72.0 * 150.0).round();
    let layer = j.tmp("signature.pdf");
    let draw = args![
        "-size",
        format!("{}x{}", px(w), px(h)),
        "xc:none",
        "(",
        sig,
        "-resize",
        format!("{}x", j.p("width")),
        ")",
        "-gravity",
        j.p("corner"),
        "-geometry",
        "+90+110",
        "-composite",
        "-units",
        "PixelsPerInch",
        "-density",
        "150",
        layer.clone()
    ];
    let overlay = args![pdf, "--overlay", layer, format!("--to={}", j.p("pages")), "--from=", "--repeat=1", "--", j.out()];
    Ok(vec![Step::Run("magick", draw), Step::Run("qpdf", overlay)])
}

/// Ghostscript's lib/PDFA_def.ps, cut down: our gs builds leave lib/ out but carry the sRGB
/// profile in their ROM. RGB is forced, hence `/N 3`. Validated with veraPDF (smoke test).
const PDFA_DEF: &str = r"%!
[/_objdef {icc_PDFA} /type /stream /OBJ pdfmark
[{icc_PDFA} << /N 3 >> /PUT pdfmark
[{icc_PDFA} (%rom%iccprofiles/srgb.icc) (r) file /PUT pdfmark
[/_objdef {OutputIntent_PDFA} /type /dict /OBJ pdfmark
[{OutputIntent_PDFA} <<
  /Type /OutputIntent
  /S /GTS_PDFA1
  /DestOutputProfile {icc_PDFA}
  /OutputConditionIdentifier (sRGB)
>> /PUT pdfmark
[{Catalog} << /OutputIntents [ {OutputIntent_PDFA} ] >> /PUT pdfmark
";
