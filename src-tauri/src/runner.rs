//! Runs a chain of tools: step N's output is step N+1's input. A chain of one is a plain run.
//! Intermediate files live in a scratch dir; the final output lands next to the first input,
//! under a name that never overwrites anything.

use crate::bins::{command, resolve, which};
use crate::tools::{self, find, stem_of, with_defaults, Job, Output, Step, Tool};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{ChildStderr, ChildStdout, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
pub struct StepReq {
    pub tool: String,
    #[serde(default)]
    pub params: HashMap<String, String>,
}

#[derive(Serialize, Clone)]
pub struct Progress<'a> {
    pub job: &'a str,
    pub step: usize,
    pub steps: usize,
    pub pct: Option<f64>,
}

pub type Emit<'a> = &'a dyn Fn(&Progress);

/// `path`, or `name-2.ext`, `name-3.ext`… — never overwrite anything.
pub fn unique(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = stem_of(&path);
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..).map(|n| path.with_file_name(format!("{stem}-{n}{ext}"))).find(|p| !p.exists()).unwrap()
}

/// Rename, or copy + delete when scratch and destination sit on different filesystems.
fn move_file(from: &Path, to: &Path) -> Result<PathBuf, String> {
    if fs::rename(from, to).is_err() {
        fs::copy(from, to).map_err(|e| format!("can't write {}: {e}", to.display()))?;
        let _ = fs::remove_file(from);
    }
    Ok(to.to_path_buf())
}

/// ffmpeg's `  Duration: 00:01:02.50, start: …` banner line, in seconds.
pub fn parse_duration(line: &str) -> Option<f64> {
    let hms = line.trim().strip_prefix("Duration: ")?.split(',').next()?;
    let mut secs = 0.0;
    for part in hms.split(':') {
        secs = secs * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(secs).filter(|s| *s > 0.0)
}

fn last_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// Collects stderr on its own thread (a chatty tool can't deadlock on a full pipe) and
/// publishes ffmpeg's input duration as soon as it shows up.
fn drain_stderr(stderr: ChildStderr, duration: Arc<Mutex<Option<f64>>>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut all = String::new();
        for line in BufReader::new(stderr).split(b'\n').map_while(Result::ok) {
            let line = String::from_utf8_lossy(&line);
            if let Some(d) = parse_duration(&line) {
                duration.lock().unwrap().get_or_insert(d);
            }
            all.push_str(&line);
            all.push('\n');
        }
        all
    })
}

/// Reads stdout to the end: ffmpeg `-progress` lines become percentages, anything else is
/// kept (last few lines) for the error message.
fn drain_stdout(stdout: ChildStdout, on_progress: impl Fn(f64)) -> String {
    let mut tail = VecDeque::new();
    for line in BufReader::new(stdout).split(b'\n').map_while(Result::ok) {
        let line = String::from_utf8_lossy(&line).trim().to_string();
        if let Some(us) = line.strip_prefix("out_time_us=") {
            if let Ok(us) = us.parse::<f64>() {
                on_progress(us / 1e6);
            }
        } else if !line.is_empty() && !line.contains('=') {
            tail.push_back(line);
            if tail.len() > 6 {
                tail.pop_front();
            }
        }
    }
    Vec::from(tail).join("\n")
}

/// Runs one binary to completion. ffmpeg gets a real progress percentage.
// CBA: no cancel; keeping the Child in a job table + a `cancel` command if long video jobs need it
fn exec(emit: Emit, p: &Progress, bin: &str, args: &[String]) -> Result<(), String> {
    let (path, args) = resolve(bin, args).ok_or_else(|| format!("{bin} is not installed"))?;
    let mut cmd = command(&path);
    if bin == "ffmpeg" {
        cmd.args(["-progress", "pipe:1", "-nostats"]);
    }
    let mut child = cmd
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{bin}: {e}"))?;
    let duration = Arc::new(Mutex::new(None));
    let stderr = drain_stderr(child.stderr.take().unwrap(), duration.clone());
    let stdout = drain_stdout(child.stdout.take().unwrap(), |secs| {
        if let Some(d) = *duration.lock().unwrap() {
            emit(&Progress { pct: Some((secs / d).clamp(0.0, 1.0)), ..*p });
        }
    });
    let status = child.wait().map_err(|e| e.to_string())?;
    let stderr = stderr.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let msg = [last_lines(&stderr, 6), stdout].into_iter().find(|m| !m.is_empty());
    Err(msg.unwrap_or_else(|| format!("{bin} failed ({status})")))
}

fn list_dir(dir: &Path, ext: &str, out: &Path) -> Result<(), String> {
    let mut files: Vec<String> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|f| tools::ext_of(f) == ext)
        .map(|f| tools::s(&f))
        .collect();
    files.sort();
    fs::write(out, files.join("\n") + "\n").map_err(|e| e.to_string())
}

fn run_step(emit: Emit, p: &Progress, step: Step, produced: &mut Vec<PathBuf>) -> Result<(), String> {
    match step {
        Step::Run(bin, args) => exec(emit, p, bin, &args),
        Step::ListDir { dir, ext, out } => list_dir(&dir, ext, &out),
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

fn check_inputs(tool: &Tool, inputs: &[PathBuf], last: bool) -> Result<(), String> {
    if !last && !matches!(tool.output, Output::File { .. }) {
        return Err(format!("{} can't feed another step", tool.name));
    }
    let accepted = |f: &PathBuf| tool.accepts.is_empty() || tool.accepts.contains(&tools::ext_of(f).as_str());
    match inputs.iter().find(|f| !accepted(f)) {
        Some(bad) => Err(format!("{} doesn't take {}", tool.name, bad.display())),
        None => Ok(()),
    }
}

/// Where one step writes. For files, the tool writes to a neutral scratch name and the runner
/// moves it into `target` after: keeps `%`, spaces & co away from printf-happy CLIs and
/// never leaves half-written files next to the user's.
struct Placement {
    out: PathBuf,
    target: PathBuf,
}

struct Chain<'a> {
    emit: Emit<'a>,
    job_id: &'a str,
    home: PathBuf,
    stem: String,
    scratch: PathBuf,
    steps: usize,
    suffixes: Vec<&'static str>,
}

impl Chain<'_> {
    fn place(&mut self, tool: &Tool, job: &Job, i: usize) -> Result<Placement, String> {
        let dest = if i + 1 == self.steps { &self.home } else { &self.scratch };
        let none = PathBuf::new();
        match &tool.output {
            Output::File { suffix, ext } => {
                self.suffixes.push(suffix);
                let ext = ext(job);
                let target = unique(dest.join(format!("{}.{}.{ext}", self.stem, self.suffixes.join("."))));
                Ok(Placement { out: self.scratch.join(format!("out{i}.{ext}")), target })
            }
            Output::Dir(suffix) => {
                self.suffixes.push(suffix);
                let dir = unique(dest.join(format!("{}.{}", self.stem, self.suffixes.join("."))));
                fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                Ok(Placement { out: dir, target: none })
            }
            Output::None => Ok(Placement { out: none.clone(), target: none }),
        }
    }

    fn step(&mut self, i: usize, req: StepReq, inputs: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
        let tool = find(&req.tool).ok_or_else(|| format!("unknown tool {}", req.tool))?;
        check_inputs(tool, &inputs, i + 1 == self.steps)?;
        let tmp = self.scratch.join(format!("step{i}"));
        fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        let mut job = Job { inputs, out: PathBuf::new(), tmp, params: with_defaults(tool, req.params) };
        let place = self.place(tool, &job, i)?;
        job.out = place.out;

        let p = Progress { job: self.job_id, step: i, steps: self.steps, pct: None };
        (self.emit)(&p);
        let mut produced = Vec::new();
        let outcome = (tool.build)(&job).into_iter().try_for_each(|s| run_step(self.emit, &p, s, &mut produced));
        match (&tool.output, outcome) {
            (Output::Dir(_), Err(e)) => {
                let _ = fs::remove_dir_all(&job.out);
                Err(e)
            }
            (_, Err(e)) => Err(e),
            (Output::File { .. }, Ok(())) if !job.out.is_file() => Err(format!("{} finished but wrote nothing", tool.name)),
            (Output::File { .. }, Ok(())) => Ok(vec![move_file(&job.out, &place.target)?]),
            (Output::Dir(_), Ok(())) => Ok(vec![job.out]),
            (Output::None, Ok(())) => Ok(produced),
        }
    }
}

pub fn run_chain(emit: Emit, job_id: &str, steps: Vec<StepReq>, inputs: Vec<String>) -> Result<Vec<String>, String> {
    if steps.is_empty() || inputs.is_empty() {
        return Err("nothing to do".into());
    }
    let mut current: Vec<PathBuf> = inputs.iter().map(PathBuf::from).collect();
    let mut chain = Chain {
        emit,
        job_id,
        home: current[0].parent().map(Path::to_path_buf).unwrap_or_default(),
        stem: stem_of(&current[0]),
        scratch: std::env::temp_dir().join(format!("remise-{}-{job_id}", std::process::id())),
        steps: steps.len(),
        suffixes: Vec::new(),
    };
    let result = steps.into_iter().enumerate().try_for_each(|(i, req)| {
        current = chain.step(i, req, std::mem::take(&mut current))?;
        Ok::<(), String>(())
    });
    let _ = fs::remove_dir_all(&chain.scratch);
    result.map(|()| current.iter().map(|p| tools::s(p)).collect())
}
