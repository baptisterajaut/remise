//! Audio tools.

use super::*;

pub static TOOLS: &[Tool] = &[
    Tool {
        id: "aud-convert",
        family: Family::Audio,
        name: "Convert audio",
        blurb: "MP3, M4A, WAV, FLAC, OGG, Opus.",
        bins: &["ffmpeg"],
        accepts: AV,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "format",
            label: "To",
            options: &[
                ("mp3", "MP3"),
                ("m4a", "M4A"),
                ("wav", "WAV"),
                ("flac", "FLAC"),
                ("ogg", "OGG"),
                ("opus", "Opus"),
            ],
            default: "mp3",
        }],
        output: Output::File { suffix: "converted", ext: fmt },
        build: |j| vec![ffmpeg(j, args!["-vn"])],
    },
    Tool {
        id: "aud-compress",
        family: Family::Audio,
        name: "Compress audio",
        blurb: "MP3 at the bitrate you pick.",
        bins: &["ffmpeg"],
        accepts: AUD,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "bitrate",
            label: "Bitrate",
            options: &[("192k", "192 kbps"), ("128k", "128 kbps"), ("96k", "96 kbps"), ("64k", "64 kbps")],
            default: "128k",
        }],
        output: Output::File { suffix: "compressed", ext: |_| "mp3".into() },
        build: |j| vec![ffmpeg(j, args!["-vn", "-c:a", "libmp3lame", "-b:a", j.p("bitrate")])],
    },
    Tool {
        id: "aud-volume",
        family: Family::Audio,
        name: "Volume & normalize",
        blurb: "Broadcast loudness or a fixed gain.",
        bins: &["ffmpeg"],
        accepts: AV,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "af",
            label: "Adjust",
            options: &[
                ("loudnorm=I=-16:TP=-1.5:LRA=11", "Normalize (−16 LUFS)"),
                ("volume=6dB", "+6 dB"),
                ("volume=3dB", "+3 dB"),
                ("volume=-3dB", "−3 dB"),
                ("volume=-6dB", "−6 dB"),
            ],
            default: "loudnorm=I=-16:TP=-1.5:LRA=11",
        }],
        output: Output::File { suffix: "leveled", ext: same },
        build: |j| vec![ffmpeg(j, args!["-af", j.p("af"), "-c:v", "copy"])],
    },
    Tool {
        id: "aud-speed",
        family: Family::Audio,
        name: "Audio speed",
        blurb: "Faster or slower, pitch kept.",
        bins: &["ffmpeg"],
        accepts: AUD,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "factor",
            label: "Speed",
            options: &[("0.75", "0.75×"), ("1.25", "1.25×"), ("1.5", "1.5×"), ("2", "2×")],
            default: "1.25",
        }],
        output: Output::File { suffix: "speed", ext: same },
        build: |j| vec![ffmpeg(j, args!["-af", format!("atempo={}", j.p("factor"))])],
    },
    Tool {
        id: "aud-waveform",
        family: Family::Audio,
        name: "Waveform image",
        blurb: "The whole track as a PNG.",
        bins: &["ffmpeg"],
        accepts: AV,
        inputs: Inputs::One,
        params: &[],
        output: Output::File { suffix: "waveform", ext: |_| "png".into() },
        build: |j| {
            vec![ffmpeg(
                j,
                args!["-filter_complex", "showwavespic=s=1600x400:colors=#f2a33a", "-frames:v", "1"],
            )]
        },
    },
    Tool {
        id: "aud-trim",
        family: Family::Audio,
        name: "Trim audio",
        blurb: "Keep a start→end slice of a track.",
        bins: &["ffmpeg"],
        accepts: AUD,
        inputs: Inputs::One,
        params: TRIM,
        output: Output::File { suffix: "trimmed", ext: same },
        build: |j| {
            vec![Step::Run(
                "ffmpeg",
                args!["-hide_banner", "-nostdin", "-y", "-ss", j.p("start"), "-to", j.p("end"), "-i", j.input(), j.out()],
            )]
        },
    },
    Tool {
        id: "aud-split",
        family: Family::Audio,
        name: "Split audio",
        blurb: "Cut a long recording into equal parts.",
        bins: &["ffmpeg"],
        accepts: AUD,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "every",
            label: "Every",
            options: &[("60", "1 min"), ("300", "5 min"), ("600", "10 min"), ("1800", "30 min")],
            default: "300",
        }],
        output: Output::Dir("parts"),
        build: |j| vec![super::video::segment(j)],
    },
    Tool {
        id: "aud-merge",
        family: Family::Audio,
        name: "Merge audio",
        blurb: "Tracks back to back, in drop order.",
        bins: &["ffmpeg"],
        accepts: AUD,
        inputs: Inputs::Many,
        params: &[Param::Select {
            key: "format",
            label: "As",
            options: &[("mp3", "MP3"), ("m4a", "M4A"), ("wav", "WAV"), ("flac", "FLAC")],
            default: "mp3",
        }],
        output: Output::File { suffix: "merged", ext: fmt },
        build: |j| {
            let mut a = args!["-hide_banner", "-nostdin", "-y"];
            for p in &j.inputs {
                a.extend(args!["-i", s(p)]);
            }
            let pads: String = (0..j.inputs.len()).map(|i| format!("[{i}:a]")).collect();
            a.extend(args!["-filter_complex", format!("{pads}concat=n={}:v=0:a=1[a]", j.inputs.len()), "-map", "[a]", j.out()]);
            vec![Step::Run("ffmpeg", a)]
        },
    },
    Tool {
        id: "aud-audiogram",
        family: Family::Audio,
        name: "Audiogram",
        blurb: "Audio as a shareable video with a live waveform.",
        bins: &["ffmpeg"],
        accepts: AUD,
        inputs: Inputs::One,
        params: &[
            Param::Select {
                key: "size",
                label: "Format",
                options: &[("1080x1080", "Square"), ("1080x1920", "Story"), ("1920x1080", "Wide")],
                default: "1080x1080",
            },
            Param::Select {
                key: "color",
                label: "Wave",
                options: &[("#f2a33a", "Amber"), ("#3cc2b3", "Teal"), ("#ffffff", "White")],
                default: "#f2a33a",
            },
        ],
        output: Output::File { suffix: "audiogram", ext: |_| "mp4".into() },
        build: |j| {
            let (w, h) = j.p("size").split_once('x').unwrap_or(("1080", "1080"));
            let fc = format!(
                "color=c=#1d1914:s={w}x{h}:r=30[bg];[0:a]showwaves=s={w}x{}:mode=cline:rate=30:colors={}[wave];\
                 [bg][wave]overlay=0:(H-h)/2:shortest=1,format=yuv420p[v]",
                h.parse::<u32>().unwrap_or(1080) / 3,
                j.p("color")
            );
            vec![ffmpeg(j, args!["-filter_complex", fc, "-map", "[v]", "-map", "0:a", "-c:v", "libx264", "-c:a", "aac", "-shortest"])]
        },
    },
];
