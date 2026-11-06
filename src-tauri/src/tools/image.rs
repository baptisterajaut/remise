//! Image tools.

use super::*;

pub static TOOLS: &[Tool] = &[
    Tool {
        id: "img-convert",
        family: Family::Image,
        name: "Convert image",
        blurb: "PNG, JPG, WebP, AVIF, TIFF, GIF.",
        bins: &["magick"],
        accepts: IMG,
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::Many,
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
    Tool {
        id: "img-crop",
        family: Family::Image,
        name: "Crop",
        blurb: "Centre-crop to a ratio: square, portrait, story, widescreen.",
        bins: &["magick"],
        accepts: IMG,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "ratio",
            label: "Ratio",
            options: &[("1:1", "1:1"), ("4:5", "4:5"), ("9:16", "9:16"), ("16:9", "16:9"), ("3:2", "3:2"), ("4:3", "4:3")],
            default: "1:1",
        }],
        output: Output::File { suffix: "cropped", ext: same },
        build: |j| vec![magick(j, args!["-auto-orient", "-gravity", "center", "-crop", j.p("ratio"), "+repage"])],
    },
    Tool {
        id: "img-redact",
        family: Family::Image,
        name: "Redact image",
        blurb: "Black out or pixelate areas (faces, plates, card numbers).",
        bins: &["magick"],
        accepts: IMG,
        inputs: Inputs::One,
        params: &[
            Param::Text {
                key: "areas",
                label: "Areas (x,y,w,h in % from top-left; ';' between)",
                default: "35,35,30,30",
                placeholder: "10,20,30,10; 60,70,20,15",
            },
            Param::Select {
                key: "mode",
                label: "How",
                options: &[("black", "Black box"), ("pixelate", "Pixelate")],
                default: "black",
            },
        ],
        output: Output::File { suffix: "redacted", ext: same },
        build: |j| match parse_areas(j.p("areas")) {
            Err(e) => vec![Step::Fail(e)],
            Ok(areas) => {
                let draw: String = areas
                    .iter()
                    .map(|[x, y, w, h]| {
                        format!("rectangle %[fx:w*{x}/100],%[fx:h*{y}/100] %[fx:w*{}/100],%[fx:h*{}/100] ", x + w, y + h)
                    })
                    .collect();
                if j.p("mode") == "black" {
                    return vec![magick(j, args!["-auto-orient", "-fill", "black", "-draw", draw])];
                }
                vec![measure(j, &j.input()), Step::Then(pixelate_plan)]
            }
        },
    },
    Tool {
        id: "img-seo",
        family: Family::Image,
        name: "SEO image",
        blurb: "Web-ready: WebP, ≤1600 px, no metadata, slug file name.",
        bins: &["magick"],
        accepts: IMG,
        inputs: Inputs::One,
        params: &[],
        output: Output::None,
        build: |j| {
            vec![
                Step::Run("magick", args![j.input(), "-auto-orient", "-strip", "-resize", "1600x1600>", "-quality", "80", j.tmp("seo.webp")]),
                Step::Then(|j| {
                    let dir = j.inputs[0].parent().unwrap_or(&j.tmp).to_path_buf();
                    let slug = slugify(&j.stem());
                    let target = (1..)
                        .map(|n| dir.join(if n == 1 { format!("{slug}.webp") } else { format!("{slug}-{n}.webp") }))
                        .find(|p| !p.exists())
                        .unwrap();
                    Ok(vec![Step::Rename(j.tmp.join("seo.webp"), target)])
                }),
            ]
        },
    },
    Tool {
        id: "img-compare",
        family: Family::Image,
        name: "Compare images",
        blurb: "Two images side by side, or what changed between them.",
        bins: &["magick"],
        accepts: IMG,
        inputs: Inputs::Many,
        params: &[Param::Select {
            key: "mode",
            label: "Show",
            options: &[("side", "Side by side"), ("diff", "Differences")],
            default: "side",
        }],
        output: Output::File { suffix: "compare", ext: |_| "png".into() },
        build: |j| {
            let [a, b] = &j.inputs[..] else { return vec![Step::Fail("drop exactly two images".into())] };
            if j.p("mode") == "diff" {
                return vec![measure(j, &s(a)), Step::Then(diff_plan)];
            }
            vec![Step::Run(
                "magick",
                args![s(a), s(b), "-resize", "x900", "-background", "white", "-splice", "12x0", "+append", "-chop", "12x0", j.out()],
            )]
        },
    },
    Tool {
        id: "img-attach",
        family: Family::Image,
        name: "Logo on each",
        blurb: "Drop a logo first, then photos: the logo lands on each one.",
        bins: &["magick"],
        accepts: IMG,
        inputs: Inputs::Many,
        params: &[
            Param::Select {
                key: "corner",
                label: "Corner",
                options: &[("southeast", "Bottom right"), ("southwest", "Bottom left"), ("northeast", "Top right"), ("northwest", "Top left")],
                default: "southeast",
            },
            Param::Select {
                key: "size",
                label: "Logo width",
                options: &[("10", "10 %"), ("15", "15 %"), ("25", "25 %")],
                default: "15",
            },
        ],
        output: Output::Dir("attached"),
        build: |j| {
            if j.inputs.len() < 2 {
                return vec![Step::Fail("drop the logo first, then one or more photos".into())];
            }
            // The logo is sized from each photo's width, which only identify knows.
            let mut steps: Vec<Step> = j.inputs[1..]
                .iter()
                .enumerate()
                .map(|(i, p)| Step::Capture {
                    bin: "magick",
                    args: args!["identify", "-format", "%w", format!("{}[0]", s(p))],
                    into: j.tmp.join(format!("{i}.w")),
                })
                .collect();
            steps.push(Step::Then(attach_plan));
            steps
        },
    },
    Tool {
        id: "img-ai-upscale",
        family: Family::Image,
        name: "AI Upscale",
        blurb: "Real-ESRGAN 4× on your GPU (optional install).",
        bins: &["realesrgan-ncnn-vulkan"],
        accepts: &["png", "jpg", "jpeg", "webp"],
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "model",
            label: "Model",
            options: &[("realesrgan-x4plus", "Photo"), ("realesrgan-x4plus-anime", "Illustration")],
            default: "realesrgan-x4plus",
        }],
        output: Output::File { suffix: "ai4x", ext: |_| "png".into() },
        // {bindir}: the models ship in a folder next to the binary.
        build: |j| {
            vec![Step::Run(
                "realesrgan-ncnn-vulkan",
                args!["-i", j.input(), "-o", j.out(), "-s", "4", "-n", j.p("model"), "-m", "{bindir}/models"],
            )]
        },
    },
    Tool {
        id: "img-text",
        family: Family::Image,
        name: "Extract text",
        blurb: "Read the text out of a photo or screenshot (OCR).",
        bins: &["tesseract"],
        accepts: &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp"],
        inputs: Inputs::One,
        params: &[Param::Text { key: "lang", label: "Languages", default: "eng", placeholder: "eng+fra" }],
        output: Output::File { suffix: "text", ext: |_| "txt".into() },
        // Tesseract appends ".txt" to the output base itself.
        build: |j| vec![Step::Run("tesseract", args![j.input(), s(&j.out.with_extension("")), "-l", j.p("lang"), "txt"])],
    },
];

/// `identify` the first frame's size into tmp/size.txt; read it back with `size_of`.
fn measure(j: &Job, image: &str) -> Step {
    Step::Capture { bin: "magick", args: args!["identify", "-format", "%w %h", format!("{image}[0]")], into: j.tmp.join("size.txt") }
}

fn size_of(j: &Job) -> Result<(f64, f64), String> {
    let text = j.read_tmp("size.txt")?;
    let mut it = text.split_whitespace().filter_map(|v| v.parse::<f64>().ok());
    it.next().zip(it.next()).ok_or_else(|| format!("unreadable image size `{text}`"))
}

/// Each area cut out, pixelated, and pasted back where it was.
fn pixelate_plan(j: &Job) -> Result<Vec<Step>, String> {
    let (w, h) = size_of(j)?;
    let mut a = args![j.input(), "-auto-orient"];
    for [x, y, aw, ah] in parse_areas(j.p("areas"))? {
        let (px, py) = ((w * x / 100.0).round(), (h * y / 100.0).round());
        let (pw, ph) = ((w * aw / 100.0).round().max(1.0), (h * ah / 100.0).round().max(1.0));
        a.extend(args![
            "(", "+clone", "-crop", format!("{pw}x{ph}+{px}+{py}"), "+repage", "-scale", "6%", "-scale", format!("{pw}x{ph}!"), ")",
            "-geometry", format!("+{px}+{py}"), "-composite"
        ]);
    }
    a.push(j.out());
    Ok(vec![Step::Run("magick", a)])
}

/// What changed: second image stretched to the first's size, absolute difference, contrast up.
fn diff_plan(j: &Job) -> Result<Vec<Step>, String> {
    let (w, h) = size_of(j)?;
    Ok(vec![Step::Run(
        "magick",
        args![
            // sRGB, no alpha: a grey+alpha first image would otherwise colour the whole result.
            s(&j.inputs[0]), "-alpha", "off", "-colorspace", "sRGB",
            "(", s(&j.inputs[1]), "-alpha", "off", "-colorspace", "sRGB", "-resize", format!("{w}x{h}!"), ")",
            "-compose", "difference", "-composite", "-auto-level", j.out()
        ],
    )])
}

fn attach_plan(j: &Job) -> Result<Vec<Step>, String> {
    let logo = s(&j.inputs[0]);
    let size: f64 = j.p("size").parse().unwrap_or(15.0);
    j.inputs[1..]
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let w: f64 = j.read_tmp(&format!("{i}.w"))?.trim().parse().map_err(|_| format!("can't read the size of {}", p.display()))?;
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let (logo_w, margin) = ((w * size / 100.0).round(), (w / 40.0).round());
            Ok(Step::Run(
                "magick",
                args![
                    s(p), "-auto-orient", "(", logo.clone(), "-resize", format!("{logo_w}x"), ")",
                    "-gravity", j.p("corner"), "-geometry", format!("+{margin}+{margin}"), "-composite",
                    s(&j.out.join(name))
                ],
            ))
        })
        .collect()
}

/// `Mon Été 2024!` → `mon-ete-2024`.
pub fn slugify(name: &str) -> String {
    let folded: String = name
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' | 'À' | 'Á' | 'Â' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' | 'Î' | 'Ï' => 'i',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' | 'Ô' | 'Ö' => 'o',
            'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            c => c.to_ascii_lowercase(),
        })
        .collect();
    let slug = folded.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join("-");
    if slug.is_empty() { "image".into() } else { slug }
}
