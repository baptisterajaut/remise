//! Create tools: things made from a screenshot, a font, a web page, or from nothing at all.

use super::*;

/// ImageMagick's rounded-corners recipe (alpha mask with four quarter circles).
fn rounded(r: u32) -> Vec<String> {
    args![
        "-alpha",
        "set",
        "(",
        "+clone",
        "-alpha",
        "extract",
        "-draw",
        format!("fill black polygon 0,0 0,{r} {r},0 fill white circle {r},{r} {r},0"),
        "(",
        "+clone",
        "-flip",
        ")",
        "-compose",
        "Multiply",
        "-composite",
        "(",
        "+clone",
        "-flop",
        ")",
        "-compose",
        "Multiply",
        "-composite",
        ")",
        "-alpha",
        "off",
        "-compose",
        "CopyOpacity",
        "-composite",
        "-compose",
        "over"
    ]
}

fn shadow(opacity: u32) -> Vec<String> {
    args![
        "(",
        "+clone",
        "-background",
        "black",
        "-shadow",
        format!("{opacity}x22+0+18"),
        ")",
        "+swap",
        "-background",
        "none",
        "-layers",
        "merge",
        "+repage"
    ]
}

const PAPER: &str = "#f3efe6";

fn mockup(j: &Job) -> Vec<String> {
    let mut a = args![j.input(), "-auto-orient"];
    match j.p("device") {
        "phone" => {
            a.extend(args!["-resize", "1170x2532>"]);
            a.extend(rounded(60));
            a.extend(args!["-bordercolor", "#111111", "-border", "28"]);
            a.extend(rounded(88));
        }
        _ => {
            a.extend(args![
                "-background",
                "#e9e6df",
                "-gravity",
                "north",
                "-splice",
                "0x46",
                "-fill",
                "#ff5f57",
                "-draw",
                "circle 24,23 30,23",
                "-fill",
                "#febc2e",
                "-draw",
                "circle 46,23 52,23",
                "-fill",
                "#28c840",
                "-draw",
                "circle 68,23 74,23"
            ]);
            a.extend(rounded(14));
        }
    }
    a.extend(shadow(40));
    a.extend(args!["-background", PAPER, "-flatten", "-bordercolor", PAPER, "-border", "80", j.out()]);
    a
}

const SIZES: &[(&str, &str)] =
    &[("1920x1080", "1920×1080"), ("1080x1080", "1080 square"), ("1080x1920", "1080×1920 story"), ("3840x2160", "4K")];

pub static TOOLS: &[Tool] = &[
    Tool {
        id: "create-mockup",
        family: Family::Create,
        name: "Device mockup",
        blurb: "Put a screenshot in a browser window or a phone.",
        bins: &["magick"],
        accepts: IMG,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "device",
            label: "Frame",
            options: &[("browser", "Browser"), ("phone", "Phone")],
            default: "browser",
        }],
        output: Output::File { suffix: "mockup", ext: |_| "png".into() },
        build: |j| vec![Step::Run("magick", mockup(j))],
    },
    Tool {
        id: "create-beautify",
        family: Family::Create,
        name: "Beautify shot",
        blurb: "Rounded corners, soft shadow, gradient backdrop.",
        bins: &["magick"],
        accepts: IMG,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "colors",
            label: "Backdrop",
            options: &[
                ("#ff9a8b #a18cd1", "Peach"),
                ("#43cea2 #185a9d", "Lagoon"),
                ("#f7971e #ffd200", "Sun"),
                ("#232526 #414345", "Graphite"),
            ],
            default: "#ff9a8b #a18cd1",
        }],
        output: Output::File { suffix: "beautified", ext: |_| "png".into() },
        // Sizes measured first: ImageMagick 6 (Ubuntu, Debian) has no %[fx:] in -extent.
        build: |j| vec![measure(j, &j.input()), Step::Then(beautify_plan)],
    },
    Tool {
        id: "create-html-image",
        family: Family::Create,
        name: "HTML to image",
        blurb: "Screenshot of a web page file (needs Chrome, Edge or Chromium).",
        bins: &["chromium"],
        accepts: HTML,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "viewport",
            label: "Viewport",
            options: &[("1440,900", "Desktop"), ("820,1180", "Tablet"), ("390,844", "Phone"), ("1200,630", "Social card")],
            default: "1440,900",
        }],
        output: Output::File { suffix: "shot", ext: |_| "png".into() },
        build: |j| vec![chromium(j, j.input(), args![format!("--window-size={}", j.p("viewport")), format!("--screenshot={}", j.out())])],
    },
    Tool {
        id: "create-gradient",
        family: Family::Create,
        name: "Gradient",
        blurb: "Wallpapers and backdrops, any size.",
        bins: &["magick"],
        accepts: &[],
        inputs: Inputs::None,
        params: &[
            Param::Select {
                key: "colors",
                label: "Colours",
                options: &[
                    ("#ff9a8b-#a18cd1", "Peach"),
                    ("#43cea2-#185a9d", "Lagoon"),
                    ("#f7971e-#ffd200", "Sun"),
                    ("#0f2027-#2c5364", "Night"),
                    ("#e0431a-#1d1914", "Ember"),
                ],
                default: "#ff9a8b-#a18cd1",
            },
            Param::Select {
                key: "kind",
                label: "Shape",
                options: &[("135", "Diagonal"), ("180", "Vertical"), ("radial", "Radial")],
                default: "135",
            },
            Param::Select { key: "size", label: "Size", options: SIZES, default: "1920x1080" },
        ],
        output: Output::File { suffix: "gradient", ext: |_| "png".into() },
        build: |j| {
            let colors = j.p("colors");
            let fill = match j.p("kind") {
                "radial" => args![format!("radial-gradient:{colors}")],
                angle => args!["-define", format!("gradient:angle={angle}"), format!("gradient:{colors}")],
            };
            let mut a = args!["-size", j.p("size")];
            a.extend(fill);
            a.push(j.out());
            vec![Step::Run("magick", a)]
        },
    },
    Tool {
        id: "create-shader",
        family: Family::Create,
        name: "Shader backdrop",
        blurb: "Procedural motion backgrounds: plasma gradients, fractals, life.",
        bins: &["ffmpeg"],
        accepts: &[],
        inputs: Inputs::None,
        params: &[
            Param::Select {
                key: "src",
                label: "Pattern",
                options: &[
                    ("gradients=n=4:speed=0.015", "Flowing gradient"),
                    ("mandelbrot", "Mandelbrot zoom"),
                    ("life=ratio=0.2:mold=8:life_color=#f2a33a:death_color=#3a1d0a", "Game of life"),
                    ("cellauto=rule=110", "Cellular automaton"),
                ],
                default: "gradients=n=4:speed=0.015",
            },
            Param::Select { key: "size", label: "Size", options: SIZES, default: "1920x1080" },
            Param::Select { key: "format", label: "As", options: &[("mp4", "6 s loop (MP4)"), ("png", "Still (PNG)")], default: "mp4" },
        ],
        output: Output::File { suffix: "shader", ext: fmt },
        build: |j| {
            // `name=opt:opt`: the first option follows '=', the rest ':'.
            let sep = if j.p("src").contains('=') { ':' } else { '=' };
            let src = format!("{}{sep}s={}:r=30", j.p("src"), j.p("size"));
            let mut a = args!["-hide_banner", "-nostdin", "-y", "-f", "lavfi", "-i", src];
            if j.p("format") == "png" {
                a.extend(args!["-frames:v", "1"]);
            } else {
                a.extend(args!["-t", "6", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart"]);
            }
            a.push(j.out());
            vec![Step::Run("ffmpeg", a)]
        },
    },
    Tool {
        id: "create-font",
        family: Family::Create,
        name: "Font specimen",
        blurb: "A specimen sheet for any .ttf or .otf.",
        bins: &["magick"],
        accepts: FONTS,
        inputs: Inputs::One,
        params: &[Param::Text { key: "sample", label: "Headline", default: "Handgloves", placeholder: "Your words" }],
        output: Output::File { suffix: "specimen", ext: |_| "png".into() },
        build: |j| {
            // A leading '@' makes ImageMagick read the text from a file.
            let sample = j.p("sample").trim_start_matches('@').to_string();
            vec![Step::Run(
                "magick",
                args![
                    "-size",
                    "1800x1200",
                    "xc:#faf5e9",
                    "-fill",
                    "#1d1914",
                    "-font",
                    j.input(),
                    "-pointsize",
                    "190",
                    "-annotate",
                    "+80+260",
                    sample,
                    "-pointsize",
                    "64",
                    "-annotate",
                    "+84+420",
                    "The quick brown fox jumps over the lazy dog",
                    "-pointsize",
                    "44",
                    "-annotate",
                    "+84+560",
                    "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                    "-annotate",
                    "+84+640",
                    "abcdefghijklmnopqrstuvwxyz",
                    "-annotate",
                    "+84+720",
                    "0123456789 & ! ? @ # % ( ) « » € £",
                    "-pointsize",
                    "26",
                    "-annotate",
                    "+84+860",
                    "Pack my box with five dozen liquor jugs. Sphinx of black quartz, judge my vow.",
                    "-fill",
                    "#8a7f6f",
                    "-pointsize",
                    "22",
                    "-annotate",
                    "+84+1130",
                    j.inputs[0].file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    j.out()
                ],
            )]
        },
    },
    Tool {
        id: "create-carousel",
        family: Family::Create,
        name: "Carousel",
        blurb: "Slice a wide image into swipeable posts.",
        bins: &["magick"],
        accepts: IMG,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "tile",
            label: "Post size",
            options: &[("1080x1350", "4:5 portrait"), ("1080x1080", "Square")],
            default: "1080x1350",
        }],
        output: Output::Dir("carousel"),
        build: |j| vec![measure(j, &j.input()), Step::Then(carousel_plan)],
    },
    Tool {
        id: "create-qr",
        family: Family::Create,
        name: "QR code",
        blurb: "Text or a link as a crisp QR code.",
        bins: &[],
        accepts: &[],
        inputs: Inputs::None,
        params: &[
            Param::Text { key: "text", label: "Content", default: "https://", placeholder: "a link, wifi, any text" },
            Param::Select { key: "format", label: "As", options: &[("svg", "SVG"), ("png", "PNG")], default: "svg" },
        ],
        output: Output::File { suffix: "qr", ext: fmt },
        build: |j| {
            let svg = match qr_svg(j.p("text")) {
                Ok(svg) => svg,
                Err(e) => return vec![Step::Fail(e)],
            };
            if j.p("format") == "svg" {
                return vec![Step::Write(j.out.clone(), svg)];
            }
            let tmp = j.tmp.join("qr.svg");
            vec![Step::Write(tmp.clone(), svg), Step::Run("magick", args!["-density", "300", s(&tmp), j.out()])]
        },
    },
];

/// Rounded, shadowed shot centred on a diagonal gradient 180 px bigger than the shot.
fn beautify_plan(j: &Job) -> Result<Vec<Step>, String> {
    let (w, h) = size_of(j)?;
    let (c1, c2) = j.p("colors").split_once(' ').unwrap_or(("#ff9a8b", "#a18cd1"));
    let mut a = args!["-size", format!("{}x{}", w + 180.0, h + 180.0), "-define", "gradient:angle=135", format!("gradient:{c1}-{c2}")];
    a.extend(args!["(", j.input(), "-auto-orient"]);
    a.extend(rounded(24));
    a.extend(shadow(45));
    a.extend(args![")", "-gravity", "center", "-composite", j.out()]);
    Ok(vec![Step::Run("magick", a)])
}

/// Scaled to the post height, padded to a whole number of posts, cut into posts.
fn carousel_plan(j: &Job) -> Result<Vec<Step>, String> {
    let (w, h) = size_of(j)?;
    let tile = j.p("tile");
    let (tw, th) = tile.split_once('x').and_then(|(a, b)| Some((a.parse::<f64>().ok()?, b.parse::<f64>().ok()?))).ok_or("bad post size")?;
    let scaled_w = (w * th / h).round();
    let padded_w = (scaled_w / tw).ceil().max(1.0) * tw;
    Ok(vec![magick_to(
        j,
        args![
            "-auto-orient",
            "-resize",
            format!("x{th}"),
            "-background",
            "white",
            "-gravity",
            "west",
            "-extent",
            format!("{padded_w}x{th}"),
            "-crop",
            tile,
            "+repage",
            "+adjoin"
        ],
        j.out_pattern("-%02d.png"),
    )])
}

pub fn qr_svg(text: &str) -> Result<String, String> {
    if text.is_empty() {
        return Err("nothing to encode".into());
    }
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(|e| format!("can't make a QR code: {e}"))?;
    Ok(code.render::<qrcode::render::svg::Color>().min_dimensions(512, 512).quiet_zone(true).build())
}
