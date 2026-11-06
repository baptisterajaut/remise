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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
            let vf = format!("fps={},scale={}:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse", j.p("fps"), j.p("width"));
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
        inputs: Inputs::One,
        params: TRIM,
        output: Output::File { suffix: "trimmed", ext: same },
        build: |j| {
            // Input-side seek + re-encode: fast and frame accurate.
            vec![Step::Run(
                "ffmpeg",
                args!["-hide_banner", "-nostdin", "-y", "-ss", j.p("start"), "-to", j.p("end"), "-i", j.input(), j.out()],
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
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
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "vf",
            label: "Action",
            options: &[("transpose=1", "90° right"), ("transpose=2", "90° left"), ("transpose=1,transpose=1", "180°"), ("hflip", "Mirror")],
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
        inputs: Inputs::Many,
        params: &[],
        output: Output::File { suffix: "joined", ext: same },
        // CBA: concat demuxer + stream copy, needs matching codecs; concat filter with re-encode if mixed sources matter
        build: |j| {
            let list = j.tmp.join("list.txt");
            let body: String = j.inputs.iter().map(|p| format!("file '{}'\n", s(p).replace('\'', "'\\''"))).collect();
            vec![
                Step::Write(list.clone(), body),
                Step::Run(
                    "ffmpeg",
                    args!["-hide_banner", "-nostdin", "-y", "-f", "concat", "-safe", "0", "-i", s(&list), "-c", "copy", j.out()],
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
        inputs: Inputs::One,
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
    Tool {
        id: "vid-split",
        family: Family::Video,
        name: "Split video",
        blurb: "Cut into equal parts, no re-encode.",
        bins: &["ffmpeg"],
        accepts: VID,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "every",
            label: "Every",
            options: &[("30", "30 s"), ("60", "1 min"), ("300", "5 min"), ("600", "10 min")],
            default: "60",
        }],
        output: Output::Dir("parts"),
        build: |j| vec![segment(j)],
    },
    Tool {
        id: "vid-crop",
        family: Family::Video,
        name: "Crop video",
        blurb: "Centre-crop to vertical, square or widescreen.",
        bins: &["ffmpeg"],
        accepts: VID,
        inputs: Inputs::One,
        params: &[Param::Select {
            key: "ratio",
            label: "Ratio",
            options: &[("9/16", "9:16 vertical"), ("1/1", "1:1 square"), ("4/5", "4:5"), ("16/9", "16:9")],
            default: "9/16",
        }],
        output: Output::File { suffix: "cropped", ext: same },
        build: |j| {
            let r = j.p("ratio");
            let vf = format!("crop='min(iw,ih*{r})':'min(ih,iw/({r}))',setsar=1");
            vec![ffmpeg(j, args!["-vf", vf, "-c:a", "copy"])]
        },
    },
    Tool {
        id: "vid-watermark",
        family: Family::Video,
        name: "Watermark video",
        blurb: "Drop a video and a logo image.",
        bins: &["ffmpeg"],
        accepts: &["mp4", "mov", "mkv", "webm", "avi", "m4v", "png", "jpg", "jpeg", "webp"],
        inputs: Inputs::Many,
        params: &[
            Param::Select {
                key: "pos",
                label: "Corner",
                options: &[
                    ("W-w-24:H-h-24", "Bottom right"),
                    ("24:H-h-24", "Bottom left"),
                    ("W-w-24:24", "Top right"),
                    ("24:24", "Top left"),
                ],
                default: "W-w-24:H-h-24",
            },
            Param::Select {
                key: "width",
                label: "Logo width",
                options: &[("120", "120 px"), ("200", "200 px"), ("320", "320 px")],
                default: "200",
            },
        ],
        output: Output::File { suffix: "watermarked", ext: |j| j.pick(VID).map(|v| ext_of(Path::new(&v))).unwrap_or_else(|| "mp4".into()) },
        build: |j| {
            let (video, logo) = match pair(j, VID, IMG, "a video and a logo image") {
                Ok(p) => p,
                Err(fail) => return vec![fail],
            };
            let fc = format!("[1:v]scale={}:-1[l];[0:v][l]overlay={}", j.p("width"), j.p("pos"));
            vec![Step::Run(
                "ffmpeg",
                args!["-hide_banner", "-nostdin", "-y", "-i", video, "-i", logo, "-filter_complex", fc, "-c:a", "copy", j.out()],
            )]
        },
    },
    Tool {
        id: "vid-caption",
        family: Family::Video,
        name: "Caption",
        blurb: "Burn subtitles into the picture: drop a video and its .srt.",
        bins: &["ffmpeg"],
        accepts: &["mp4", "mov", "mkv", "webm", "avi", "m4v", "srt", "vtt", "ass"],
        inputs: Inputs::Many,
        params: &[Param::Select {
            key: "size",
            label: "Text size",
            options: &[("18", "Small"), ("24", "Medium"), ("32", "Large")],
            default: "24",
        }],
        output: Output::File { suffix: "captioned", ext: |_| "mp4".into() },
        build: |j| {
            let (video, subs) = match pair(j, VID, &["srt", "vtt", "ass"], "a video and a subtitle file (.srt)") {
                Ok(p) => p,
                Err(fail) => return vec![fail],
            };
            // The subtitles filter wants its path escaped twice over; a copy with a fixed name, read
            // from inside scratch, sidesteps quotes, colons and backslashes in the real path.
            let name = format!("subs.{}", ext_of(Path::new(&subs)));
            let vf = format!("subtitles={name}:force_style='FontSize={},Outline=1'", j.p("size"));
            vec![
                Step::Copy(PathBuf::from(subs), j.tmp.join(&name)),
                Step::RunIn(
                    j.tmp.clone(),
                    "ffmpeg",
                    args![
                        "-hide_banner",
                        "-nostdin",
                        "-y",
                        "-i",
                        video,
                        "-vf",
                        vf,
                        "-c:v",
                        "libx264",
                        "-crf",
                        "20",
                        "-pix_fmt",
                        "yuv420p",
                        "-c:a",
                        "copy",
                        j.out()
                    ],
                ),
            ]
        },
    },
    Tool {
        id: "vid-transcribe",
        family: Family::Video,
        name: "Transcribe",
        blurb: "Speech to subtitles with Whisper, on this machine.",
        bins: &["ffmpeg", "whisper-cli", "whisper-model"],
        accepts: AV,
        inputs: Inputs::One,
        params: &[
            Param::Select {
                key: "lang",
                label: "Language",
                options: &[
                    ("auto", "Detect"),
                    ("en", "English"),
                    ("fr", "French"),
                    ("es", "Spanish"),
                    ("de", "German"),
                    ("it", "Italian"),
                    ("pt", "Portuguese"),
                    ("nl", "Dutch"),
                ],
                default: "auto",
            },
            Param::Select { key: "format", label: "As", options: &[("srt", "SRT"), ("txt", "Text"), ("vtt", "WebVTT")], default: "srt" },
        ],
        output: Output::File { suffix: "transcript", ext: fmt },
        // whisper.cpp reads 16 kHz mono WAV, and adds the extension to `-of` itself.
        build: |j| {
            let wav = j.tmp("speech.wav");
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
            vec![
                Step::Run("ffmpeg", args!["-i", j.input(), "-vn", "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le", wav.clone()]),
                Step::Run(
                    "whisper-cli",
                    args![
                        "-m",
                        "{which:whisper-model}",
                        "-f",
                        wav,
                        "-l",
                        j.p("lang"),
                        format!("-o{}", j.p("format")),
                        "-of",
                        s(&j.out.with_extension("")),
                        "-t",
                        threads.to_string(),
                        "-np"
                    ],
                ),
            ]
        },
    },
    Tool {
        id: "vid-insert",
        family: Family::Video,
        name: "Insert clip",
        blurb: "Drop the main video first, then the clip to splice in at a time.",
        bins: &["ffmpeg"],
        accepts: VID,
        inputs: Inputs::Many,
        params: &[Param::Text { key: "at", label: "Insert at (seconds)", default: "5", placeholder: "12.5" }],
        output: Output::File { suffix: "inserted", ext: |_| "mp4".into() },
        // CBA: both clips need an audio track, and the clip is forced to 25 fps to match
        build: |j| {
            let [main, clip] = &j.inputs[..] else {
                return vec![Step::Fail("drop the main video, then one clip".into())];
            };
            let Ok(at) = j.p("at").trim().parse::<f64>() else {
                return vec![Step::Fail("the insert time is a number of seconds".into())];
            };
            let fc = format!(
                "[0:v]split=3[m1][m2][mr];[0:a]asplit[a1][a2];\
                 [m1]trim=0:{at},setpts=PTS-STARTPTS,setsar=1[v0];[a1]atrim=0:{at},asetpts=PTS-STARTPTS[x0];\
                 [m2]trim=start={at},setpts=PTS-STARTPTS,setsar=1[v2];[a2]atrim=start={at},asetpts=PTS-STARTPTS[x2];\
                 [1:v][mr]scale=rw:rh,setsar=1,fps=25[v1];[1:a]aresample=48000[x1];\
                 [v0][x0][v1][x1][v2][x2]concat=n=3:v=1:a=1[v][a]"
            );
            vec![Step::Run(
                "ffmpeg",
                args![
                    "-hide_banner",
                    "-nostdin",
                    "-y",
                    "-i",
                    s(main),
                    "-i",
                    s(clip),
                    "-filter_complex",
                    fc,
                    "-map",
                    "[v]",
                    "-map",
                    "[a]",
                    "-c:v",
                    "libx264",
                    "-crf",
                    "20",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    j.out()
                ],
            )]
        },
    },
    Tool {
        id: "vid-add-audio",
        family: Family::Video,
        name: "Add audio",
        blurb: "Drop a video and a track: replace the sound or mix it in.",
        bins: &["ffmpeg"],
        accepts: AV,
        inputs: Inputs::Many,
        params: &[Param::Select {
            key: "mode",
            label: "Track",
            options: &[("replace", "Replace"), ("mix", "Mix with original")],
            default: "replace",
        }],
        output: Output::File { suffix: "soundtrack", ext: |_| "mp4".into() },
        build: |j| {
            let (video, audio) = match pair(j, VID, AUD, "a video and an audio file") {
                Ok(p) => p,
                Err(fail) => return vec![fail],
            };
            let map = if j.p("mode") == "mix" {
                args!["-filter_complex", "[0:a][1:a]amix=inputs=2:duration=first[a]", "-map", "0:v", "-map", "[a]"]
            } else {
                args!["-map", "0:v", "-map", "1:a", "-shortest"]
            };
            let mut a = args!["-hide_banner", "-nostdin", "-y", "-i", video, "-i", audio];
            a.extend(map);
            a.extend(args!["-c:v", "copy", "-c:a", "aac", j.out()]);
            vec![Step::Run("ffmpeg", a)]
        },
    },
    Tool {
        id: "vid-download",
        family: Family::Video,
        name: "Video downloader",
        blurb: "Save a video from a link with yt-dlp (optional install).",
        bins: &["yt-dlp", "ffmpeg"],
        accepts: &[],
        inputs: Inputs::None,
        params: &[
            Param::Text { key: "url", label: "Link", default: "", placeholder: "https://…" },
            Param::Select {
                key: "quality",
                label: "Quality",
                options: &[("bv*[height<=1080]+ba/b", "Up to 1080p"), ("bv*+ba/b", "Best"), ("ba/b", "Audio only")],
                default: "bv*[height<=1080]+ba/b",
            },
        ],
        output: Output::Dir("downloads"),
        build: |j| {
            let url = j.p("url").trim();
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return vec![Step::Fail("paste a link starting with https://".into())];
            }
            let template = format!("{}/%(title).80s.%(ext)s", s(&j.out).replace('%', "%%"));
            vec![
                Step::Then(fresh_ytdlp),
                // QuickJS (bundled) runs YouTube's player code; deno/node are used instead if present.
                Step::Run("yt-dlp", args!["--no-playlist", "--js-runtimes", "quickjs", "-f", j.p("quality"), "-o", template, "--", url]),
            ]
        },
    },
];

/// Sites change weekly and the bundled yt-dlp is frozen at build time, inside a read-only app.
/// First use copies it to the per-user tools dir, where `-U` can replace it; every run then tries
/// an update and carries on with whatever it has if that fails (offline, distro-packaged yt-dlp).
fn fresh_ytdlp(_: &Job) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    if let (Some(current), Some(user)) = (crate::bins::which("yt-dlp"), crate::bins::user_tools())
        && crate::bins::is_bundled(&current)
    {
        // Copy under a temporary name, then rename: a half-copied yt-dlp must never look installed.
        let name = current.file_name().ok_or("odd yt-dlp path")?;
        let target = user.join("yt-dlp").join(name);
        let part = target.with_extension("part");
        steps.push(Step::Try(Box::new(Step::Copy(current.clone(), part.clone()))));
        steps.push(Step::Try(Box::new(Step::Rename(part, target))));
    }
    steps.push(Step::Try(Box::new(Step::Run("yt-dlp", args!["-U"]))));
    Ok(steps)
}

/// Equal-length parts with the segment muxer, stream copy.
pub fn segment(j: &Job) -> Step {
    Step::Run(
        "ffmpeg",
        args![
            "-hide_banner",
            "-nostdin",
            "-y",
            "-i",
            j.input(),
            "-map",
            "0",
            "-c",
            "copy",
            "-f",
            "segment",
            "-segment_time",
            j.p("every"),
            "-reset_timestamps",
            "1",
            j.out_pattern(&format!("-%03d.{}", j.in_ext()))
        ],
    )
}
