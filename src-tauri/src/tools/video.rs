//! Video tools.

use super::*;

pub static TOOLS: &[Tool] = &[
    Tool {
        id: "vid-compress",
        family: Family::Video,
        name: "Compress video",
        blurb: "H.264 MP4 that plays everywhere.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "crf",
            label: "Strength",
            options: &[("23", "Light"), ("28", "Balanced"), ("32", "Strong")],
            default: "28",
        }],
        output: Output::File { suffix: "compressed", ext: |_| "mp4".into() },
        build: |j| {
            vec![ffmpeg(
                j,
                args![
                    "-c:v",
                    "libx264",
                    "-preset",
                    "medium",
                    "-crf",
                    j.p("crf"),
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "128k",
                    "-movflags",
                    "+faststart"
                ],
            )]
        },
    },
    Tool {
        id: "vid-convert",
        family: Family::Video,
        name: "Convert video",
        blurb: "MP4, WebM, MOV, MKV.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "format",
            label: "To",
            options: &[("mp4", "MP4"), ("webm", "WebM"), ("mov", "MOV"), ("mkv", "MKV")],
            default: "mp4",
        }],
        output: Output::File { suffix: "converted", ext: fmt },
        build: |j| vec![ffmpeg(j, args![])],
    },
    Tool {
        id: "vid-gif",
        family: Family::Video,
        name: "Video to GIF",
        blurb: "Palette-optimised, loops forever.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[
            Param::Select {
                key: "fps",
                label: "Frame rate",
                options: &[("10", "10 fps"), ("15", "15 fps"), ("24", "24 fps")],
                default: "15",
            },
            Param::Select {
                key: "width",
                label: "Width",
                options: &[("320", "320 px"), ("480", "480 px"), ("640", "640 px"), ("800", "800 px")],
                default: "480",
            },
        ],
        output: Output::File { suffix: "anim", ext: |_| "gif".into() },
        build: |j| {
            let vf = format!(
                "fps={},scale={}:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse",
                j.p("fps"),
                j.p("width")
            );
            vec![ffmpeg(j, args!["-vf", vf, "-loop", "0"])]
        },
    },
    Tool {
        id: "vid-trim",
        family: Family::Video,
        name: "Trim",
        blurb: "Keep a start→end slice. Frame accurate.",
        bins: &["ffmpeg"],
        accepts: AV,
        multi: false,
        params: TRIM,
        output: Output::File { suffix: "trimmed", ext: same },
        build: |j| {
            // Input-side seek + re-encode: fast and frame accurate.
            vec![Step::Run(
                "ffmpeg",
                args![
                    "-hide_banner",
                    "-nostdin",
                    "-y",
                    "-ss",
                    j.p("start"),
                    "-to",
                    j.p("end"),
                    "-i",
                    j.input(),
                    j.out()
                ],
            )]
        },
    },
    Tool {
        id: "vid-extract-audio",
        family: Family::Video,
        name: "Extract audio",
        blurb: "Soundtrack only.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "format",
            label: "Format",
            options: &[("mp3", "MP3"), ("m4a", "M4A"), ("wav", "WAV"), ("flac", "FLAC")],
            default: "mp3",
        }],
        output: Output::File { suffix: "audio", ext: fmt },
        build: |j| vec![ffmpeg(j, args!["-vn"])],
    },
    Tool {
        id: "vid-mute",
        family: Family::Video,
        name: "Mute",
        blurb: "Drop the audio track, no re-encode.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[],
        output: Output::File { suffix: "muted", ext: same },
        build: |j| vec![ffmpeg(j, args!["-c:v", "copy", "-an"])],
    },
    Tool {
        id: "vid-speed",
        family: Family::Video,
        name: "Change speed",
        blurb: "Slow-mo or timelapse, audio pitch kept.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "factor",
            label: "Speed",
            options: &[("0.5", "0.5×"), ("0.75", "0.75×"), ("1.25", "1.25×"), ("1.5", "1.5×"), ("2", "2×")],
            default: "1.5",
        }],
        output: Output::File { suffix: "speed", ext: same },
        build: |j| {
            let f = j.p("factor");
            vec![ffmpeg(j, args!["-vf", format!("setpts=PTS/{f}"), "-af", format!("atempo={f}")])]
        },
    },
    Tool {
        id: "vid-resize",
        family: Family::Video,
        name: "Resize video",
        blurb: "Scale down to a standard height.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "height",
            label: "Height",
            options: &[("2160", "2160p"), ("1080", "1080p"), ("720", "720p"), ("480", "480p")],
            default: "720",
        }],
        output: Output::File { suffix: "resized", ext: same },
        build: |j| vec![ffmpeg(j, args!["-vf", format!("scale=-2:{}", j.p("height")), "-c:a", "copy"])],
    },
    Tool {
        id: "vid-rotate",
        family: Family::Video,
        name: "Rotate video",
        blurb: "Fix phone footage.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "vf",
            label: "Action",
            options: &[
                ("transpose=1", "90° right"),
                ("transpose=2", "90° left"),
                ("transpose=1,transpose=1", "180°"),
                ("hflip", "Mirror"),
            ],
            default: "transpose=1",
        }],
        output: Output::File { suffix: "rotated", ext: same },
        build: |j| vec![ffmpeg(j, args!["-vf", j.p("vf"), "-c:a", "copy"])],
    },
    Tool {
        id: "vid-join",
        family: Family::Video,
        name: "Join videos",
        blurb: "Back to back, no re-encode (same camera/settings).",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: true,
        params: &[],
        output: Output::File { suffix: "joined", ext: same },
        // CBA: concat demuxer + stream copy, needs matching codecs; concat filter with re-encode if mixed sources matter
        build: |j| {
            let list = j.tmp.join("list.txt");
            let body: String = j
                .inputs
                .iter()
                .map(|p| format!("file '{}'\n", s(p).replace('\'', "'\\''")))
                .collect();
            vec![
                Step::Write(list.clone(), body),
                Step::Run(
                    "ffmpeg",
                    args![
                        "-hide_banner",
                        "-nostdin",
                        "-y",
                        "-f",
                        "concat",
                        "-safe",
                        "0",
                        "-i",
                        s(&list),
                        "-c",
                        "copy",
                        j.out()
                    ],
                ),
            ]
        },
    },
    Tool {
        id: "vid-frames",
        family: Family::Video,
        name: "Extract frames",
        blurb: "One still every N seconds.",
        bins: &["ffmpeg"],
        accepts: VID,
        multi: false,
        params: &[Param::Select {
            key: "every",
            label: "Every",
            options: &[("1", "1 s"), ("5", "5 s"), ("10", "10 s"), ("30", "30 s")],
            default: "5",
        }],
        output: Output::Dir("frames"),
        build: |j| {
            vec![Step::Run(
                "ffmpeg",
                args![
                    "-hide_banner",
                    "-nostdin",
                    "-y",
                    "-i",
                    j.input(),
                    "-vf",
                    format!("fps=1/{}", j.p("every")),
                    "-q:v",
                    "2",
                    j.out_pattern("-%04d.jpg")
                ],
            )]
        },
    },
];
