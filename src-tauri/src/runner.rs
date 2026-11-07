//! Runs a chain of tools: step N's output is step N+1's input. A chain of one is a plain run.
//! Intermediate files live in a scratch dir; the final output lands next to the first input,
//! under a name that never overwrites anything.

use crate::bins::{command, resolve, which};
use crate::tools::{self, Inputs, Job, Output, Step, Tool, find, stem_of, with_defaults};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStderr, ChildStdout, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Jobs the user stopped: checked between steps, and every 100 ms while a program runs.
static STOPPED: Mutex<Vec<String>> = Mutex::new(Vec::new());
pub const STOPPED_MSG: &str = "Stopped.";

/// Stops `job`: kills the program it is running and skips its remaining steps. Results only
/// leave the scratch folder at the end, so a stopped job leaves no half-written file.
pub fn stop(job: &str) {
    STOPPED.lock().unwrap_or_else(|e| e.into_inner()).push(job.to_string());
}

/// Ctrl+C in a terminal: every job stops. An atomic, so a signal handler may set it.
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub fn interrupt() {
    INTERRUPTED.store(true, Ordering::Relaxed);
}

fn stopped(job: &str) -> bool {
    INTERRUPTED.load(Ordering::Relaxed) || STOPPED.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|j| j == job)
}

fn forget_stop(job: &str) {
    STOPPED.lock().unwrap_or_else(|e| e.into_inner()).retain(|j| j != job);
}

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

/// Rename, or copy + delete when scratch and destination sit on different filesystems. The free
/// name is picked again at the last moment: another run may have taken it in the meantime.
fn move_file(from: &Path, to: &Path) -> Result<PathBuf, String> {
    let to = &unique(to.to_path_buf());
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

/// The program and everything it started: its process group on Unix, its tree on Windows.
fn kill_tree(pid: u32) {
    #[cfg(unix)]
    let killer = which("kill").map(|k| (k, vec!["-KILL".to_string(), "--".into(), format!("-{pid}")]));
    #[cfg(windows)]
    let killer = which("taskkill").map(|k| (k, vec!["/T".to_string(), "/F".into(), "/PID".into(), pid.to_string()]));
    if let Some((path, args)) = killer {
        let _ = command(&path).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
}

/// `{bindir}`: data that ships next to a tool's binary (waifu2x models). A whole argument
/// `{which:NAME}`: where another program or a downloaded model is.
fn expand(arg: &str, bindir: &str) -> String {
    let arg = arg.replace("{bindir}", bindir);
    match arg.strip_prefix("{which:").and_then(|name| name.strip_suffix('}')) {
        Some(name) => which(name).map(|p| tools::s(&p)).unwrap_or(arg),
        None => arg,
    }
}

/// Runs one binary to completion. ffmpeg gets a real progress percentage.
fn exec(emit: Emit, p: &Progress, bin: &str, args: &[String], cwd: Option<&Path>, input: Option<&str>) -> Result<(), String> {
    let (path, args) = resolve(bin, args).ok_or_else(|| format!("{bin} is not installed"))?;
    let bindir = path.parent().map(tools::s).unwrap_or_default();
    let args: Vec<String> = args.iter().map(|a| expand(a, &bindir)).collect();
    let mut cmd = command(&path);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    if bin == "ffmpeg" {
        cmd.args(["-progress", "pipe:1", "-nostats"]);
    }
    // Its own process group, so Stop also reaches what it starts (soffice.bin, yt-dlp's ffmpeg).
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let stdin = if input.is_some() { Stdio::piped() } else { Stdio::null() };
    let mut child =
        cmd.args(&args).stdin(stdin).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("{bin}: {e}"))?;
    // A line or two, well under a pipe's buffer: written whole, then closed so the program sees
    // the end. A program that never reads it (no prompt came up) is fine.
    if let (Some(text), Some(mut pipe)) = (input, child.stdin.take()) {
        let _ = pipe.write_all(text.as_bytes());
    }
    // Stdout is read on this thread (the progress callback isn't Send), so Stop is watched on
    // another one, which only needs the pid to kill the program and what it started.
    let done = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (done, job, pid) = (done.clone(), p.job.to_string(), child.id());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                if stopped(&job) {
                    return kill_tree(pid);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };
    let duration = Arc::new(Mutex::new(None));
    let stderr = drain_stderr(child.stderr.take().unwrap(), duration.clone());
    let stdout = drain_stdout(child.stdout.take().unwrap(), |secs| {
        if let Some(d) = *duration.lock().unwrap() {
            emit(&Progress { pct: Some((secs / d).clamp(0.0, 1.0)), ..*p });
        }
    });
    let status = child.wait().map_err(|e| e.to_string());
    done.store(true, Ordering::Relaxed);
    let _ = watcher.join();
    if stopped(p.job) {
        return Err(STOPPED_MSG.into());
    }
    let status = status?;
    let stderr = stderr.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let msg = [last_lines(&stderr, 6), stdout].into_iter().find(|m| !m.is_empty());
    Err(msg.unwrap_or_else(|| format!("{bin} failed ({status})")))
}

/// Runs a binary and saves its stdout, for a later `Then` to read.
fn capture(bin: &str, args: &[String], into: &Path) -> Result<(), String> {
    let (path, args) = resolve(bin, args).ok_or_else(|| format!("{bin} is not installed"))?;
    let out = command(&path).args(&args).stdin(Stdio::null()).output().map_err(|e| format!("{bin}: {e}"))?;
    if !out.status.success() {
        return Err(last_lines(&String::from_utf8_lossy(&out.stderr), 6));
    }
    fs::write(into, &out.stdout).map_err(|e| e.to_string())
}

/// ffmpeg exits non-zero without an output file; the summary on stderr is all we want.
fn probe(input: &Path, into: &Path) -> Result<(), String> {
    let path = which("ffmpeg").ok_or("ffmpeg is not installed")?;
    let out = command(&path).args(["-hide_banner", "-i"]).arg(input).stdin(Stdio::null()).output().map_err(|e| format!("ffmpeg: {e}"))?;
    fs::write(into, &out.stderr).map_err(|e| e.to_string())
}

fn touch(path: &Path, when: std::time::SystemTime) -> Result<(), String> {
    let f = fs::File::options().write(true).open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.set_modified(when).map_err(|e| format!("{}: {e}", path.display()))
}

fn rename(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Err(format!("{} already exists", to.display()));
    }
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::rename(from, to).map_err(|e| e.to_string())
}

fn run_step(emit: Emit, p: &Progress, job: &Job, step: Step, produced: &mut Vec<PathBuf>) -> Result<(), String> {
    match step {
        Step::Run(bin, args) => exec(emit, p, bin, &args, None, None),
        Step::RunIn(dir, bin, args) => exec(emit, p, bin, &args, Some(&dir), None),
        Step::Feed(bin, args, input) => exec(emit, p, bin, &args, None, Some(&input)),
        Step::Capture { bin, args, into } => capture(bin, &args, &into),
        Step::Probe(input, into) => probe(&input, &into),
        Step::Then(next) => next(job)?.into_iter().try_for_each(|s| run_step(emit, p, job, s, produced)),
        Step::Fail(msg) => Err(msg),
        // A failure is fine, a Stop is not: the job ends there.
        Step::Try(step) => match run_step(emit, p, job, *step, produced) {
            Err(e) if e == STOPPED_MSG => Err(e),
            _ => Ok(()),
        },
        Step::Copy(from, to) => {
            if let Some(dir) = to.parent() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            fs::copy(&from, &to).map(|_| ()).map_err(|e| format!("{}: {e}", from.display()))
        }
        Step::Touch(path, when) => {
            touch(&path, when)?;
            produced.push(path);
            Ok(())
        }
        Step::Write(path, body) => fs::write(path, body).map_err(|e| e.to_string()),
        Step::Rename(from, to) => {
            rename(&from, &to)?;
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
        let outcome = (tool.build)(&job).into_iter().try_for_each(|s| {
            if stopped(self.job_id) {
                return Err(STOPPED_MSG.to_string());
            }
            run_step(self.emit, &p, &job, s, &mut produced)
        });
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

/// A fresh scratch dir with an unguessable name, created exclusively (owner-only on Unix): on a
/// shared /tmp nobody can pre-create it.
fn scratch_dir(job_id: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("remise-{}-{job_id}-{nanos:x}", std::process::id()));
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// `fallback` is where a generator (no input file) writes its result.
pub fn run_chain(emit: Emit, job_id: &str, steps: Vec<StepReq>, inputs: Vec<String>, fallback: &Path) -> Result<Vec<String>, String> {
    let generator = steps.first().and_then(|s| find(&s.tool)).is_some_and(|t| t.inputs == Inputs::None);
    if steps.is_empty() || (inputs.is_empty() && !generator) {
        return Err("nothing to do".into());
    }
    forget_stop(job_id);
    let mut current: Vec<PathBuf> = inputs.iter().map(PathBuf::from).collect();
    let (home, stem) = match current.first() {
        Some(first) => (first.parent().map(Path::to_path_buf).unwrap_or_default(), stem_of(first)),
        None => (fallback.to_path_buf(), "remise".to_string()),
    };
    let mut chain = Chain { emit, job_id, home, stem, scratch: scratch_dir(job_id)?, steps: steps.len(), suffixes: Vec::new() };
    let result = steps.into_iter().enumerate().try_for_each(|(i, req)| {
        current = chain.step(i, req, std::mem::take(&mut current))?;
        Ok::<(), String>(())
    });
    let _ = fs::remove_dir_all(&chain.scratch);
    forget_stop(job_id);
    result.map(|()| current.iter().map(|p| tools::s(p)).collect())
}
