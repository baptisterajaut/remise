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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: true,
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
];
