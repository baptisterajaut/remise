//! What a video holds, read from the stream summary `ffmpeg -i` prints (Remise ships no
//! ffprobe): enough to tell whether clips can be joined as they are, and what to fill in.

use super::*;

#[derive(Debug, Default, PartialEq)]
pub struct Media {
    /// Codec, width, height, frames per second.
    pub video: Option<(String, u32, u32, Option<f64>)>,
    /// Codec, sample rate, channel layout.
    pub audio: Option<(String, String, String)>,
    pub duration: Option<f64>,
}

impl Media {
    /// Clips with the same signature can be joined without re-encoding.
    pub fn signature(&self) -> String {
        let v = self.video.as_ref().map(|(c, w, h, _)| format!("{c} {w}x{h}"));
        let a = self.audio.as_ref().map(|(c, r, l)| format!("{c} {r} {l}"));
        format!("{v:?}/{a:?}")
    }
}

pub fn parse(text: &str) -> Media {
    let mut m = Media::default();
    for line in text.lines().map(str::trim) {
        if m.duration.is_none() {
            m.duration = crate::runner::parse_duration(line);
        }
        let Some((_, desc)) = line.split_once(": Video: ").or_else(|| line.split_once(": Audio: ")) else {
            continue;
        };
        let parts: Vec<&str> = desc.split(',').map(str::trim).collect();
        let codec = parts[0].split_whitespace().next().unwrap_or_default().to_string();
        if line.contains(": Video: ") && m.video.is_none() {
            let size = parts
                .iter()
                .filter_map(|p| p.split_whitespace().next()?.split_once('x'))
                .find_map(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
            let fps = parts.iter().find_map(|p| p.strip_suffix(" fps")?.parse().ok());
            if let Some((w, h)) = size {
                m.video = Some((codec, w, h, fps));
            }
        } else if line.contains(": Audio: ") && m.audio.is_none() {
            let get = |i: usize| parts.get(i).copied().unwrap_or_default().to_string();
            m.audio = Some((codec, get(1), get(2)));
        }
    }
    m
}

/// `Probe` each input into tmp/probe-<i>.txt, then `plan`, which reads them with `probed`.
pub fn probe_then(j: &Job, plan: fn(&Job) -> Result<Vec<Step>, String>) -> Vec<Step> {
    let mut steps: Vec<Step> =
        j.inputs.iter().enumerate().map(|(i, p)| Step::Probe(p.clone(), j.tmp.join(format!("probe-{i}.txt")))).collect();
    steps.push(Step::Then(plan));
    steps
}

pub fn probed(j: &Job) -> Result<Vec<Media>, String> {
    (0..j.inputs.len()).map(|i| j.read_tmp(&format!("probe-{i}.txt")).map(|t| parse(&t))).collect()
}

/// Silence as an extra ffmpeg input, for a clip without sound.
pub fn silence(secs: f64) -> Vec<String> {
    args!["-f", "lavfi", "-t", format!("{secs:.3}"), "-i", "anullsrc=r=48000:cl=stereo"]
}
