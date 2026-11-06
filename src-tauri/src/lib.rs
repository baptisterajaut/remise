#![warn(clippy::cognitive_complexity, clippy::too_many_lines)]

mod tools;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter};
use tools::{find, stem_of, with_defaults, Family, Job, Output, Param, Step, TOOLS};

// ───────────────────────────── binaries ─────────────────────────────

/// Names a binary goes by across platforms.
fn aliases(bin: &str) -> Vec<&str> {
    match bin {
        "gs" => vec!["gs", "gswin64c", "gswin32c"],
        "7z" => vec!["7z", "7zz", "7za"],
        other => vec![other],
    }
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> =
        std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    // Apps launched from Finder don't inherit the shell PATH, so Homebrew would be invisible.
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"].map(PathBuf::from));
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/bin"));
    }
    // Windows installers for these two don't touch PATH.
    for pf in ["ProgramFiles", "ProgramFiles(x86)"].iter().filter_map(std::env::var_os) {
        let pf = PathBuf::from(pf);
        dirs.push(pf.join("7-Zip"));
        if let Ok(rd) = fs::read_dir(pf.join("gs")) {
            dirs.extend(rd.flatten().map(|e| e.path().join("bin")));
        }
    }
    dirs
}

fn which(bin: &str) -> Option<PathBuf> {
    let dirs = search_dirs();
    for name in aliases(bin) {
        for d in &dirs {
            for cand in [d.join(name), d.join(format!("{name}.exe"))] {
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
    }
    None
}

fn command(path: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

// ───────────────────────────── manifest ─────────────────────────────

#[derive(Serialize)]
struct ToolInfo {
    id: &'static str,
    family: Family,
    name: &'static str,
    blurb: &'static str,
    accepts: &'static [&'static str],
    multi: bool,
    params: &'static [Param],
    output: &'static str,
    missing: Vec<&'static str>,
}

#[tauri::command]
fn list_tools() -> Vec<ToolInfo> {
    let mut found: HashMap<&str, bool> = HashMap::new();
    TOOLS
        .iter()
        .map(|t| ToolInfo {
            id: t.id,
            family: t.family,
            name: t.name,
            blurb: t.blurb,
            accepts: t.accepts,
            multi: t.multi,
            params: t.params,
            output: match t.output {
                Output::File { .. } => "file",
                Output::Dir(_) => "dir",
                Output::None => "none",
            },
            missing: t
                .bins
                .iter()
                .copied()
                .filter(|b| !*found.entry(b).or_insert_with(|| which(b).is_some()))
                .collect(),
        })
        .collect()
}

// ───────────────────────────── runner ─────────────────────────────

#[derive(Deserialize)]
struct StepReq {
    tool: String,
    #[serde(default)]
    params: HashMap<String, String>,
}

type Emit<'a> = &'a dyn Fn(&Progress);

#[derive(Serialize, Clone)]
struct Progress<'a> {
    job: &'a str,
    step: usize,
    steps: usize,
    pct: Option<f64>,
}

/// `path`, or `name-2.ext`, `name-3.ext`… — never overwrite anything.
fn unique(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = stem_of(&path);
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| path.with_file_name(format!("{stem}-{n}{ext}")))
        .find(|p| !p.exists())
        .unwrap()
}

/// Rename, or copy + delete when scratch and destination sit on different filesystems.
fn move_file(from: &Path, to: &Path) -> Result<PathBuf, String> {
    if fs::rename(from, to).is_err() {
        fs::copy(from, to).map_err(|e| format!("can't write {}: {e}", to.display()))?;
        let _ = fs::remove_file(from);
    }
    Ok(to.to_path_buf())
}

fn tail(lines: &VecDeque<String>) -> String {
    lines.iter().map(String::as_str).collect::<Vec<_>>().join("\n")
}

fn probe_duration(input: &str) -> Option<f64> {
    let out = command(&which("ffprobe")?)
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1", input])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok().filter(|d: &f64| *d > 0.0)
}

/// Runs one binary to completion. ffmpeg gets a real progress percentage.
fn exec(emit: Emit, p: &Progress, bin: &str, args: &[String]) -> Result<(), String> {
    let path = which(bin).ok_or_else(|| format!("{bin} is not installed"))?;
    let mut cmd = command(&path);
    let duration = if bin == "ffmpeg" {
        cmd.args(["-progress", "pipe:1", "-nostats"]);
        args.iter().position(|a| a == "-i").and_then(|i| probe_duration(&args[i + 1]))
    } else {
        None
    };
    let mut child = cmd
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{bin}: {e}"))?;

    // Drain stderr on its own thread so a chatty tool can't deadlock on a full pipe.
    let mut stderr = child.stderr.take().unwrap();
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });

    let mut last_out = VecDeque::new();
    for line in BufReader::new(child.stdout.take().unwrap()).split(b'\n').map_while(Result::ok) {
        let line = String::from_utf8_lossy(&line).trim().to_string();
        if let (Some(d), Some(us)) = (duration, line.strip_prefix("out_time_us=")) {
            if let Ok(us) = us.parse::<f64>() {
                let pct = Some((us / 1e6 / d).clamp(0.0, 1.0));
                emit(&Progress { pct, ..*p });
            }
        } else if !line.is_empty() && duration.is_none() {
            last_out.push_back(line);
            if last_out.len() > 6 {
                last_out.pop_front();
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let stderr = err_reader.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let mut last_err: VecDeque<String> =
        stderr.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    while last_err.len() > 6 {
        last_err.pop_front();
    }
    let msg = if last_err.is_empty() { tail(&last_out) } else { tail(&last_err) };
    Err(if msg.is_empty() { format!("{bin} failed ({status})") } else { msg })
}

fn run_step(emit: Emit, p: &Progress, step: Step, produced: &mut Vec<PathBuf>) -> Result<(), String> {
    match step {
        Step::Run(bin, args) => exec(emit, p, bin, &args),
        Step::ListDir { dir, ext, out } => {
            let mut files: Vec<String> = fs::read_dir(&dir)
                .map_err(|e| e.to_string())?
                .flatten()
                .map(|e| e.path())
                .filter(|f| tools::ext_of(f) == ext)
                .map(|f| tools::s(&f))
                .collect();
            files.sort();
            fs::write(out, files.join("\n") + "\n").map_err(|e| e.to_string())
        }
        Step::Write(path, body) => fs::write(path, body).map_err(|e| e.to_string()),
        Step::Rename(from, to) => {
            if to.exists() {
                return Err(format!("{} already exists", to.display()));
            }
            fs::rename(&from, &to).map_err(|e| e.to_string())?;
            produced.push(to);
            Ok(())
        }
        Step::Launch(bin, args) => {
            let path = which(bin).ok_or_else(|| format!("{bin} is not installed"))?;
            command(&path).args(args).spawn().map(|_| ()).map_err(|e| format!("{bin}: {e}"))
        }
    }
}

/// Runs a chain of tools: step N's output is step N+1's input. A chain of one is a plain run.
/// Intermediate files live in a temp dir; the final output lands next to the first input.
fn run_chain(emit: Emit, job_id: &str, steps: Vec<StepReq>, inputs: Vec<String>) -> Result<Vec<String>, String> {
    if steps.is_empty() || inputs.is_empty() {
        return Err("nothing to do".into());
    }
    let original: Vec<PathBuf> = inputs.iter().map(PathBuf::from).collect();
    let home = original[0].parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = stem_of(&original[0]);
    let scratch = std::env::temp_dir().join(format!("remise-{}-{}", std::process::id(), job_id));
    let n = steps.len();
    let mut current = original.clone();
    let mut suffixes: Vec<&str> = Vec::new();

    let result = (|| {
        for (i, req) in steps.into_iter().enumerate() {
            let tool = find(&req.tool).ok_or_else(|| format!("unknown tool {}", req.tool))?;
            let last = i + 1 == n;
            if !last && !matches!(tool.output, Output::File { .. }) {
                return Err(format!("{} can't feed another step", tool.name));
            }
            if let Some(bad) = current
                .iter()
                .find(|f| !tool.accepts.is_empty() && !tool.accepts.contains(&tools::ext_of(f).as_str()))
            {
                return Err(format!("{} doesn't take {}", tool.name, bad.display()));
            }
            let tmp = scratch.join(format!("step{i}"));
            fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
            let mut job = Job {
                inputs: current.clone(),
                out: PathBuf::new(),
                tmp,
                params: with_defaults(tool, req.params),
            };
            let dest = if last { home.clone() } else { scratch.clone() };
            let mut target = PathBuf::new();
            job.out = match &tool.output {
                // Tools write to a neutral scratch name; the runner moves it into place after.
                // Keeps `%`, spaces & co away from printf-happy CLIs, and never leaves half files.
                Output::File { suffix, ext } => {
                    suffixes.push(suffix);
                    let ext = ext(&job);
                    target = unique(dest.join(format!("{stem}.{}.{ext}", suffixes.join("."))));
                    scratch.join(format!("out{i}.{ext}"))
                }
                Output::Dir(suffix) => {
                    suffixes.push(suffix);
                    let d = unique(dest.join(format!("{stem}.{}", suffixes.join("."))));
                    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
                    d
                }
                Output::None => PathBuf::new(),
            };

            let p = Progress { job: job_id, step: i, steps: n, pct: None };
            emit(&p);
            let mut produced = Vec::new();
            let outcome = (tool.build)(&job).into_iter().try_for_each(|s| run_step(emit, &p, s, &mut produced));
            match (&tool.output, outcome) {
                (Output::Dir(_), Err(e)) => {
                    let _ = fs::remove_dir_all(&job.out);
                    return Err(e);
                }
                (_, Err(e)) => return Err(e),
                (Output::File { .. }, Ok(())) if !job.out.is_file() => {
                    return Err(format!("{} finished but wrote nothing", tool.name));
                }
                (Output::File { .. }, Ok(())) => current = vec![move_file(&job.out, &target)?],
                (Output::None, Ok(())) => current = produced,
                (_, Ok(())) => current = vec![job.out],
            }
        }
        Ok(current.iter().map(|p| tools::s(p)).collect())
    })();
    let _ = fs::remove_dir_all(&scratch);
    result
}

#[tauri::command]
async fn run(app: AppHandle, job: String, steps: Vec<StepReq>, inputs: Vec<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |p: &Progress| {
            let _ = app.emit("progress", p);
        };
        run_chain(&emit, &job, steps, inputs)
    })
        .await
        .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run_app() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![list_tools, run])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Every tool against real binaries on generated fixtures: `cargo test -- --ignored smoke`.
#[cfg(test)]
mod smoke {
    use super::*;

    fn sh(bin: &str, args: &[&str]) {
        let ok = command(&which(bin).unwrap()).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status();
        assert!(ok.unwrap().success(), "fixture {bin} {args:?}");
    }

    fn go(tool: &str, inputs: &[&Path], params: &[(&str, &str)]) -> Result<Vec<String>, String> {
        let req = StepReq { tool: tool.into(), params: params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() };
        let inputs = inputs.iter().map(|p| tools::s(p)).collect();
        run_chain(&|_| {}, tool, vec![req], inputs)
    }

    #[test]
    #[ignore]
    fn smoke_every_tool() {
        let d = std::env::temp_dir().join(format!("remise-smoke-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let f = |n: &str| d.join(n);
        let (png, png2, pdf, mp4, mp4b, wav) = (f("a 50%.png"), f("b.png"), f("doc.pdf"), f("clip.mp4"), f("clip2.mp4"), f("tone.wav"));
        sh("magick", &["-size", "640x400", "xc:white", "-fill", "black", "-pointsize", "48", "-annotate", "+40+200", "Hello Remise", png.to_str().unwrap()]);
        sh("magick", &["-size", "320x200", "gradient:orange-navy", png2.to_str().unwrap()]);
        sh("magick", &[png.to_str().unwrap(), png2.to_str().unwrap(), pdf.to_str().unwrap()]);
        for out in [&mp4, &mp4b] {
            sh("ffmpeg", &["-y", "-f", "lavfi", "-i", "testsrc=s=320x240:d=3", "-f", "lavfi", "-i", "sine=d=3", "-shortest", "-pix_fmt", "yuv420p", out.to_str().unwrap()]);
        }
        sh("ffmpeg", &["-y", "-f", "lavfi", "-i", "sine=d=2", wav.to_str().unwrap()]);

        let mut failures = vec![];
        let mut check = |tool: &str, inputs: &[&Path], params: &[(&str, &str)]| match go(tool, inputs, params) {
            Ok(out) => {
                assert!(!out.is_empty() && out.iter().all(|o| Path::new(o).exists()), "{tool}: {out:?}");
                println!("ok   {tool:20} -> {}", out.join(", "));
            }
            Err(e) => failures.push(format!("{tool}: {e}")),
        };
        for t in TOOLS {
            match t.id {
                "pdf-studio" | "img-remove-bg" | "batch-rename" | "pdf-unlock" => continue,
                "pdf-encrypt" => check(t.id, &[&pdf], &[("password", "s3cret")]),
                "pdf-extract" => check(t.id, &[&pdf], &[("pages", "2")]),
                "vid-trim" => check(t.id, &[&mp4], &[("start", "00:00:01"), ("end", "00:00:02")]),
                "arc-extract" => {}
                id if t.multi && t.family == Family::Video => check(id, &[&mp4, &mp4b], &[]),
                id if t.multi && t.family == Family::Files => check(id, &[&png, &pdf], &[]),
                id if t.multi && t.accepts.contains(&"pdf") => check(id, &[&pdf, &pdf], &[]),
                id if t.multi => check(id, &[&png, &png2], &[]),
                id if t.accepts.contains(&"pdf") => check(id, &[&pdf], &[]),
                id if t.accepts.contains(&"png") => check(id, &[&png], &[]),
                id if t.family == Family::Audio && t.accepts.contains(&"wav") => check(id, &[&wav], &[]),
                id => check(id, &[&mp4], &[]),
            }
        }
        // Round trips that need a previous output.
        let zip = go("arc-create", &[&png, &pdf], &[]).unwrap();
        check("arc-extract", &[Path::new(&zip[0])], &[]);
        let locked = go("pdf-encrypt", &[&pdf], &[("password", "pw")]).unwrap();
        check("pdf-unlock", &[Path::new(&locked[0])], &[("password", "pw")]);
        // A real chain: compress -> grayscale -> web, one output next to the input.
        let steps = ["pdf-compress", "pdf-grayscale", "pdf-web"].map(|t| StepReq { tool: t.into(), params: HashMap::new() });
        let out = run_chain(&|_| {}, "chain", steps.into(), vec![tools::s(&pdf)]).unwrap();
        assert!(out[0].ends_with("doc.compressed.gray.web.pdf"), "{out:?}");
        // Rename last: it moves the fixtures.
        let (r1, r2) = (f("r1.txt"), f("r2.txt"));
        fs::write(&r1, "").unwrap();
        fs::write(&r2, "").unwrap();
        check("batch-rename", &[&r1, &r2], &[("pattern", "note-{n}")]);
        assert!(f("note-01.txt").exists() && f("note-02.txt").exists());

        assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
        let _ = fs::remove_dir_all(&d);
    }
}
