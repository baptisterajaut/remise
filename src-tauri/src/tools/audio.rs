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
            options: &[("mp3", "MP3"), ("m4a", "M4A"), ("wav", "WAV"), ("flac", "FLAC"), ("ogg", "OGG"), ("opus", "Opus")],
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
        build: |j| vec![ffmpeg(j, args!["-filter_complex", "showwavespic=s=1600x400:colors=#f2a33a", "-frames:v", "1"])],
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
    Tool {
        id: "aud-cleanup",
        family: Family::Audio,
        name: "Clean up recording",
        blurb: "Cuts the pauses and the hiss out of a voice memo, a lecture or a talking video.",
        bins: &["ffmpeg"],
        accepts: AV,
        inputs: Inputs::One,
        params: &[
            Param::Select {
                key: "pauses",
                label: "Cut pauses",
                options: &[("0.3", "Over 0.3 s"), ("0.5", "Over 0.5 s"), ("1", "Over 1 s"), ("off", "Keep them")],
                default: "0.5",
            },
            Param::Select {
                key: "level",
                label: "Silence is",
                options: &[("-40", "Below −40 dB (quiet room)"), ("-35", "Below −35 dB"), ("-30", "Below −30 dB (noisy room)")],
                default: "-35",
            },
            Param::Select { key: "denoise", label: "Hiss", options: &[("on", "Reduce"), ("off", "Leave")], default: "on" },
        ],
        output: Output::File { suffix: "clean", ext: same },
        build: |j| {
            if j.p("pauses") == "off" && j.p("denoise") == "off" {
                return vec![Step::Fail("nothing to do: cut the pauses, reduce the hiss, or both".into())];
            }
            media::probe_then(j, detect)
        },
    },
];

/// Silence left on each side of a cut, so words don't start or end clipped.
const MARGIN: f64 = 0.12;
/// Fade at each join, so a cut never clicks.
const FADE: f64 = 0.01;

fn denoise(j: &Job) -> &'static str {
    if j.p("denoise") == "on" { "afftdn," } else { "" }
}

/// Finds the pauses on the sound as it will come out (denoised first, when asked), into
/// tmp/silences.txt. The file name stays relative (the step runs in tmp): a Windows path's
/// `C:\` would need escaping inside a filter.
fn detect(j: &Job) -> Result<Vec<Step>, String> {
    let m = media::probed(j)?.remove(0);
    if m.audio.is_none() {
        return Err(format!("{} has no sound to clean up", j.input()));
    }
    if j.p("pauses") == "off" {
        return Ok(render(j, &m, &[]));
    }
    let af = format!("{}silencedetect=n={}dB:d={},ametadata=mode=print:file=silences.txt", denoise(j), j.p("level"), j.p("pauses"));
    let find = args!["-hide_banner", "-nostdin", "-y", "-i", j.input(), "-map", "0:a:0", "-af", af, "-f", "null", "-"];
    Ok(vec![Step::RunIn(j.tmp.clone(), "ffmpeg", find), Step::Then(cut)])
}

fn cut(j: &Job) -> Result<Vec<Step>, String> {
    let m = media::probed(j)?.remove(0);
    let duration = m.duration.ok_or_else(|| format!("can't tell how long {} is", j.input()))?;
    let keep = keeps(&j.read_tmp("silences.txt").unwrap_or_default(), duration, MARGIN);
    match keep.as_slice() {
        [] => Err("it's all silence (try a lower silence level)".into()),
        [(a, b)] if *a == 0.0 && *b == duration => Ok(render(j, &m, &[])),
        _ => Ok(render(j, &m, &keep)),
    }
}

/// The stretches to keep, from silencedetect's `lavfi.silence_start=`/`_end=` lines: each
/// pause shrinks by `margin` on both sides, except at the very start and end of the file.
/// A pause still running at the end has no `_end`.
pub fn keeps(silences: &str, duration: f64, margin: f64) -> Vec<(f64, f64)> {
    let (mut gaps, mut start) = (Vec::new(), None);
    for line in silences.lines().map(str::trim) {
        if let Some(v) = line.strip_prefix("lavfi.silence_start=") {
            start = v.parse::<f64>().ok();
        } else if let (Some(v), Some(s)) = (line.strip_prefix("lavfi.silence_end="), start) {
            gaps.extend(v.parse::<f64>().ok().map(|e| (s, e)));
            start = None;
        }
    }
    gaps.extend(start.map(|s| (s, duration)));
    let (mut keep, mut from) = (Vec::new(), 0.0);
    for (s, e) in gaps {
        let a = if s <= margin { 0.0 } else { s + margin };
        let b = if e >= duration - margin { duration } else { e - margin };
        if b <= a {
            continue;
        }
        if a > from {
            keep.push((from, a));
        }
        from = b;
    }
    if from < duration {
        keep.push((from, duration));
    }
    keep
}

/// No stretches: the picture is copied and only the sound is redone. Otherwise every stretch
/// is trimmed out of the picture and the sound and joined back: `concat` lines the two up at
/// each join (`select`/`aselect` drifted 0.45 s over 50 cuts). The graph goes in a file, a long
/// lecture makes hundreds of stretches.
fn render(j: &Job, m: &media::Media, keep: &[(f64, f64)]) -> Vec<Step> {
    let mut a = args!["-hide_banner", "-nostdin", "-y", "-i", j.input()];
    if keep.is_empty() {
        if j.p("denoise") == "off" {
            return vec![Step::Copy(j.inputs[0].clone(), j.out.clone())];
        }
        a.extend(args!["-c:v", "copy", "-af", "afftdn", j.out()]);
        return vec![Step::Run("ffmpeg", a)];
    }
    // An audio file's picture is cover art, not something to cut.
    let video = m.video.is_some() && VID.contains(&j.in_ext().as_str());
    let graph = j.tmp.join("cut.txt");
    a.extend(args!["-/filter_complex", s(&graph), "-map", "[a]"]);
    if video {
        a.extend(args!["-map", "[v]"]);
        a.extend(super::video::codecs(j));
    }
    a.push(j.out());
    vec![Step::Write(graph, cut_graph(denoise(j), keep, video)), Step::Run("ffmpeg", a)]
}

pub fn cut_graph(denoise: &str, keep: &[(f64, f64)], video: bool) -> String {
    let n = keep.len();
    let pads = |p: &str| (0..n).map(|i| format!("[{p}{i}]")).collect::<String>();
    let mut g = format!("[0:a:0]{denoise}asplit={n}{};", pads("a"));
    if video {
        g += &format!("[0:v:0]split={n}{};", pads("v"));
    }
    let mut joined = String::new();
    for (i, (from, to)) in keep.iter().enumerate() {
        let out = (to - from - FADE).max(0.0);
        g += &format!(
            "[a{i}]atrim=start={from:.3}:end={to:.3},asetpts=PTS-STARTPTS,afade=t=in:d={FADE},afade=t=out:st={out:.3}:d={FADE}[x{i}];"
        );
        if video {
            g += &format!("[v{i}]trim=start={from:.3}:end={to:.3},setpts=PTS-STARTPTS[w{i}];");
            joined += &format!("[w{i}]");
        }
        joined += &format!("[x{i}]");
    }
    let outs = if video { "[v][a]" } else { "[a]" };
    g + &format!("{joined}concat=n={n}:v={}:a=1{outs}", u8::from(video))
}
