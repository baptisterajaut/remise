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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
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
        multi: false,
        params: &[],
        output: Output::File { suffix: "waveform", ext: |_| "png".into() },
        build: |j| {
            vec![ffmpeg(
                j,
                args!["-filter_complex", "showwavespic=s=1600x400:colors=#f2a33a", "-frames:v", "1"],
            )]
        },
    },
];
