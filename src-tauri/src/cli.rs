//! The same tools from a terminal: `remise <tool> [--option value]… <files>…`.
//! Progress goes to stderr and the paths written to stdout, one per line, so scripts can use
//! them. Exit status: 0 done, 1 a file failed (or a tool is missing), 2 a usage error.

use crate::runner::{self, Progress, StepReq};
use crate::tools::{self, Family, Inputs, Output, Param, Tool};
use crate::{bins, fetch};
use std::collections::{BTreeSet, HashMap};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

const USAGE: &str = "\
Remise: local file tools.

  remise                          open the app
  remise list                     every tool, by family
  remise help <tool>              what a tool takes, and its options
  remise <tool> [--option value]… <files>…
  remise download <name>          fetch what a tool needs once (a browser, a model…)

Results land next to each input, or in the current folder for tools that take no file; their
paths are printed one per line. Passwords come from REMISE_PASSWORD, never the command line.";

/// A terminal call, as opposed to opening the app: there are arguments, and the first is
/// neither a file to open nor the `-psn_…` that macOS Finder launches can add.
pub fn wanted(args: &[String]) -> bool {
    args.first().is_some_and(|a| !a.starts_with("-psn_") && !Path::new(a).exists())
}

/// The AppImage's AppRun moves into `$APPDIR/usr` before starting Remise, so relative paths and
/// results would land inside the app. The runtime keeps the folder it was started from in `OWD`
/// when it mounts the image; extracting (APPIMAGE_EXTRACT_AND_RUN) sets no `OWD`, but `chdir`
/// leaves the shell's `PWD` alone.
/// CBA: PWD is wrong when the caller changed folder without a shell (a script's subprocess
/// `cwd=`) and the image was extracted, not mounted; only `OWD` from the runtime is exact.
pub fn undo_apprun_chdir() {
    let Some(appdir) = std::env::var_os("APPDIR").filter(|_| std::env::var_os("APPIMAGE").is_some()) else { return };
    if !std::env::current_dir().is_ok_and(|d| d.starts_with(&appdir)) {
        return;
    }
    if let Some(dir) = ["OWD", "PWD"].into_iter().filter_map(std::env::var_os).map(PathBuf::from).find(|d| d.is_dir()) {
        let _ = std::env::set_current_dir(dir);
    }
}

pub fn main(args: &[String], package: &tauri::PackageInfo, identifier: &str) -> i32 {
    // The same folders the app gets from Tauri: resources beside (or inside) the executable,
    // downloads under the per-user data dir. REMISE_VENDOR / REMISE_USER_TOOLS still win.
    let resources = tauri::utils::platform::resource_dir(package, &tauri::Env::default()).ok().or_else(macos_resources);
    bins::set_vendor(resources.map(|d| d.join("vendor")));
    bins::set_user_tools(dirs::data_local_dir().map(|d| d.join(identifier).join("tools")));
    on_ctrl_c();
    match dispatch(args, &package.version.to_string()) {
        Ok(code) => code,
        Err(usage) => {
            eprintln!("{usage}");
            2
        }
    }
}

/// Tauri refuses a macOS executable path with a symlink in it (`/var` → `/private/var`, or a
/// link put in /usr/local/bin): the real path still finds Contents/Resources.
fn macos_resources() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    cfg!(target_os = "macos").then(|| exe.parent()?.join("../Resources").canonicalize().ok()).flatten()
}

/// Ok(exit code) once something ran; Err(message) for a usage error.
fn dispatch(args: &[String], version: &str) -> Result<i32, String> {
    let (first, rest) = (args[0].as_str(), &args[1..]);
    match first {
        "-h" | "--help" | "help" if rest.is_empty() => println!("{USAGE}"),
        "help" => println!("{}", help(tool(&rest[0])?)),
        "-V" | "--version" => println!("remise {version}"),
        "list" => print!("{}", list()),
        "download" => return download(rest),
        _ => return run(tool(first)?, rest),
    }
    Ok(0)
}

fn tool(id: &str) -> Result<&'static Tool, String> {
    tools::find(id).ok_or_else(|| format!("no tool or file named \"{id}\" (`remise list` shows the tools)"))
}

/// What a command line asks of a tool.
#[derive(Debug, PartialEq)]
pub struct Call {
    pub params: HashMap<String, String>,
    pub files: Vec<PathBuf>,
}

/// `--key value` or `--key=value`, checked against the tool's own options; anything else is a
/// file, and everything after `--` is too.
pub fn parse(t: &Tool, args: &[String], password: Option<String>) -> Result<Call, String> {
    let mut call = Call { params: HashMap::new(), files: vec![] };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let Some(opt) = arg.strip_prefix("--") else {
            call.files.push(PathBuf::from(arg));
            continue;
        };
        if opt.is_empty() {
            call.files.extend(it.by_ref().map(PathBuf::from));
            break;
        }
        let (key, value) = match opt.split_once('=') {
            Some((k, v)) => (k, v.to_string()),
            None => (opt, it.next().ok_or_else(|| format!("--{opt} needs a value"))?.clone()),
        };
        call.params.insert(key.to_string(), checked(t, key, value)?);
    }
    if let Some(pw) = password.filter(|_| has_password(t)) {
        call.params.insert("password".into(), pw);
    }
    Ok(call)
}

fn checked(t: &Tool, key: &str, value: String) -> Result<String, String> {
    if key == "password" && has_password(t) {
        return Err("put the password in REMISE_PASSWORD: a command line is visible to every program on this computer".into());
    }
    match t.params.iter().find(|p| p.key() == key && p.key() != "password") {
        None => Err(format!("{} has no --{key} (`remise help {}` lists its options)", t.id, t.id)),
        Some(Param::Select { options, .. }) if !options.iter().any(|(v, _)| *v == value) => {
            Err(format!("--{key} is one of: {}", options.iter().map(|(v, _)| *v).collect::<Vec<_>>().join(", ")))
        }
        Some(_) => Ok(value),
    }
}

fn has_password(t: &Tool) -> bool {
    t.params.iter().any(|p| p.key() == "password")
}

/// Which runs a call makes: each file in turn, all of them at once, or one with no file.
fn batches(t: &Tool, files: Vec<PathBuf>) -> Result<Vec<Vec<PathBuf>>, String> {
    if t.inputs == Inputs::None && !files.is_empty() {
        return Err(format!("{} makes a file from its options alone; it takes no file", t.id));
    }
    if let Some(gone) = files.iter().find(|f| !f.exists()) {
        return Err(format!("no such file: {}", gone.display()));
    }
    // Absolute, so the printed results are paths from anywhere.
    let files: Vec<PathBuf> = files.iter().map(|f| std::path::absolute(f).unwrap_or_else(|_| f.clone())).collect();
    match (t.inputs, files.is_empty()) {
        (Inputs::None, _) => Ok(vec![vec![]]),
        (_, true) => Err(format!("{} needs a file (`remise help {}`)", t.id, t.id)),
        (Inputs::One, false) => Ok(files.into_iter().map(|f| vec![f]).collect()),
        (Inputs::Many, false) => Ok(vec![files]),
    }
}

// CBA: file-dates reads its date as UTC here; the app sends the local time zone, which the
// webview knows. A `--tz` (minutes east of UTC) would be the upgrade.
fn run(t: &Tool, args: &[String]) -> Result<i32, String> {
    let password = std::env::var("REMISE_PASSWORD").ok();
    let hint = has_password(t) && password.is_none();
    let call = parse(t, args, password)?;
    let batches = batches(t, call.files)?;
    if let Some(missing) = missing(t) {
        eprintln!("{missing}");
        return Ok(1);
    }
    let here = std::env::current_dir().unwrap_or_default();
    let mut failed = false;
    for (n, inputs) in batches.into_iter().enumerate() {
        let label = inputs.first().and_then(|f| f.file_name()).map_or(t.name.into(), |f| f.to_string_lossy().into_owned());
        let steps = vec![StepReq { tool: t.id.into(), params: call.params.clone() }];
        let inputs = inputs.iter().map(|f| tools::s(f)).collect();
        let meter = Meter::new(&label);
        let result = runner::run_chain(&|p| meter.show(p), &format!("cli-{n}"), steps, inputs, &here);
        meter.done();
        match result {
            Ok(out) => out.iter().for_each(|o| println!("{o}")),
            Err(e) if e == runner::STOPPED_MSG => return Ok(130),
            Err(e) => {
                let hint = if hint { " (passwords come from REMISE_PASSWORD)" } else { "" };
                eprintln!("{label}: {e}{hint}");
                failed = true;
            }
        }
    }
    Ok(i32::from(failed))
}

/// The first program the tool needs and doesn't find, and how to get it.
fn missing(t: &Tool) -> Option<String> {
    let bin = t.bins.iter().find(|b| !bins::available(b))?;
    Some(match fetch::find(bin) {
        Some(d) => format!("{} needs {} ({} MB, once): run `remise download {bin}`", t.name, d.name, d.bytes.div_ceil(1_000_000)),
        None => format!("{} needs {bin}, which isn't installed", t.name),
    })
}

/// A percentage that rewrites itself on a terminal; a plain start line anywhere else (a log, a
/// pipe), where carriage returns would only make noise.
struct Meter<'a> {
    label: &'a str,
    tty: bool,
}

impl<'a> Meter<'a> {
    fn new(label: &'a str) -> Self {
        let tty = std::io::stderr().is_terminal();
        if tty {
            eprint!("{label}…");
        } else {
            eprintln!("{label}…");
        }
        Meter { label, tty }
    }

    fn show(&self, p: &Progress) {
        if let (true, Some(pct)) = (self.tty, p.pct) {
            eprint!("\r{}… {:>3.0}%", self.label, pct * 100.0);
        }
    }

    /// Blanks the line (no ANSI codes: older Windows consoles print them as text).
    fn done(&self) {
        if self.tty {
            eprint!("\r{:w$}\r", "", w = self.label.chars().count() + 8);
        }
    }
}

fn download(rest: &[String]) -> Result<i32, String> {
    let names: BTreeSet<&str> = tools::all().flat_map(|t| t.bins.iter()).filter_map(|b| fetch::find(b)).map(|d| d.bin).collect();
    let names = names.into_iter().collect::<Vec<_>>().join(", ");
    let [bin] = rest else { return Err(format!("usage: remise download <name>, one of: {names}")) };
    let d = fetch::find(bin).ok_or_else(|| format!("nothing to download named \"{bin}\"; one of: {names}"))?;
    let label = format!("{} ({} MB)", d.name, d.bytes.div_ceil(1_000_000));
    let meter = Meter::new(&label);
    let result = fetch::fetch(&|p| meter.show(p), d);
    meter.done();
    match result {
        Ok(path) => println!("{}", path.display()),
        Err(e) => {
            eprintln!("{label}: {e}");
            return Ok(1);
        }
    }
    Ok(0)
}

fn family(f: Family) -> &'static str {
    match f {
        Family::Pdf => "PDF",
        Family::Docs => "Docs",
        Family::Image => "Image",
        Family::Video => "Video",
        Family::Audio => "Audio",
        Family::Create => "Create",
        Family::Files => "Files",
    }
}

pub fn list() -> String {
    let mut out = String::new();
    let mut last = None;
    for t in tools::all() {
        if last != Some(t.family) {
            out += &format!("{}{}\n", if last.is_some() { "\n" } else { "" }, family(t.family));
            last = Some(t.family);
        }
        let needs = t.bins.iter().find(|b| !bins::available(b)).map(|b| match fetch::find(b) {
            Some(_) => format!("  [remise download {b}]"),
            None => format!("  [needs {b}]"),
        });
        out += &format!("  {:<20} {}: {}{}\n", t.id, t.name, t.blurb, needs.unwrap_or_default());
    }
    out
}

pub fn help(t: &Tool) -> String {
    let exts =
        if t.accepts.is_empty() { "any file".to_string() } else { t.accepts.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(" ") };
    let takes = match t.inputs {
        Inputs::One => format!("one file at a time ({exts}); give several to run them in turn"),
        Inputs::Many => format!("all the files you give, at once ({exts})"),
        Inputs::None => "no file: it makes one from its options".into(),
    };
    let writes = match (&t.output, t.inputs) {
        (Output::File { .. }, Inputs::None) => "a new file in the current folder",
        (Output::File { .. }, _) => "a new file next to the input",
        (Output::Dir(_), _) => "a folder next to the input",
        (Output::None, _) => "nothing new: it changes the files in place, or opens another app",
    };
    let mut out = format!("{}: {}\n{}\n\nTakes    {takes}\nWrites   {writes}\n", t.id, t.name, t.blurb);
    if !t.params.is_empty() {
        out += "\nOptions\n";
    }
    for p in t.params {
        out += &option(p);
    }
    let first = t.params.iter().find_map(|p| match p {
        Param::Select { key, options, .. } if *key != "password" => options.last().map(|(v, _)| format!(" --{key} {v}")),
        _ => None,
    });
    let file = match t.inputs {
        Inputs::None => String::new(),
        _ => format!(" file.{}", t.accepts.first().unwrap_or(&"txt")),
    };
    out + &format!("\nExample\n  remise {}{}{file}\n", t.id, first.unwrap_or_default())
}

fn option(p: &Param) -> String {
    match p {
        Param::Text { key: "password", label, .. } => format!("  {:<24} {label}, from the environment\n", "REMISE_PASSWORD"),
        Param::Select { key, label, options, default } => {
            let values = options.iter().map(|(v, _)| *v).collect::<Vec<_>>().join("|");
            let mut line = format!("  {:<24} {label}, default {default}\n", format!("--{key} {values}"));
            // Only when the names say more than the values ("2 = PDF/A-2b", not "zip = ZIP").
            if options.iter().any(|(v, name)| !v.eq_ignore_ascii_case(name)) {
                let names = options.iter().map(|(v, name)| format!("{v} = {name}")).collect::<Vec<_>>().join(", ");
                line += &format!("  {:<24} {names}\n", "");
            }
            line
        }
        Param::Text { key, label, default, placeholder } => {
            let default = if default.is_empty() { format!("e.g. {placeholder}") } else { format!("default \"{default}\"") };
            format!("  {:<24} {label}, {default}\n", format!("--{key} <text>"))
        }
    }
}

/// Ctrl+C stops the running program and its children (each runs in its own process group,
/// which the terminal's signal doesn't reach) the way the app's Stop button does. A second
/// Ctrl+C quits at once.
#[cfg(unix)]
fn on_ctrl_c() {
    extern "C" fn handler(_: libc::c_int) {
        runner::interrupt();
        // Both async-signal-safe: an atomic store and signal().
        unsafe { libc::signal(libc::SIGINT, libc::SIG_DFL) };
    }
    unsafe { libc::signal(libc::SIGINT, handler as extern "C" fn(libc::c_int) as libc::sighandler_t) };
}

/// Windows: Remise.exe runs behind remise-cli.exe, which takes its whole process tree down with
/// it on Ctrl+C (a job object): there is no console here to get the event from.
#[cfg(not(unix))]
fn on_ctrl_c() {}
