#![warn(clippy::cognitive_complexity, clippy::too_many_lines)]

mod bins;
mod cli;
mod cutout;
mod fetch;
mod runner;
mod tools;

#[cfg(test)]
mod tests;

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tools::{Family, Inputs, Output, Param};

pub use cutout::main as cutout;

#[derive(Serialize)]
struct ToolInfo {
    id: &'static str,
    family: Family,
    name: &'static str,
    blurb: &'static str,
    accepts: &'static [&'static str],
    multi: bool,
    /// No input: params only (gradient, QR code…).
    generator: bool,
    params: &'static [Param],
    output: &'static str,
    /// Extension of the file it writes with default params, so the chain builder knows what can follow.
    produces: Option<String>,
    missing: Vec<&'static str>,
}

fn produces(t: &tools::Tool) -> Option<String> {
    let Output::File { ext, .. } = &t.output else {
        return None;
    };
    let sample = format!("input.{}", t.accepts.first().unwrap_or(&"bin"));
    let job = tools::Job {
        inputs: vec![PathBuf::from(sample)],
        out: PathBuf::new(),
        tmp: PathBuf::new(),
        params: tools::with_defaults(t, HashMap::new()),
    };
    Some(ext(&job))
}

#[tauri::command]
fn list_tools() -> Vec<ToolInfo> {
    let mut found: HashMap<&str, bool> = HashMap::new();
    tools::all()
        .map(|t| ToolInfo {
            id: t.id,
            family: t.family,
            name: t.name,
            blurb: t.blurb,
            accepts: t.accepts,
            multi: t.inputs == Inputs::Many,
            generator: t.inputs == Inputs::None,
            params: t.params,
            output: match t.output {
                Output::File { .. } => "file",
                Output::Dir(_) => "dir",
                Output::None => "none",
            },
            produces: produces(t),
            missing: t.bins.iter().copied().filter(|b| !*found.entry(b).or_insert_with(|| bins::available(b))).collect(),
        })
        .collect()
}

#[tauri::command]
async fn run(app: AppHandle, job: String, steps: Vec<runner::StepReq>, inputs: Vec<String>) -> Result<Vec<String>, String> {
    // The id ends up in a scratch path that is later deleted: keep it boring.
    if job.is_empty() || job.len() > 32 || !job.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("bad job id".into());
    }
    // Generators have no input file to sit next to: their results go to Downloads.
    let fallback = app.path().download_dir().or_else(|_| app.path().home_dir()).unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |p: &runner::Progress| {
            let _ = app.emit("progress", p);
        };
        runner::run_chain(&emit, &job, steps, inputs, &fallback)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What Batch rename or Sort & route would do with these files, before it does it.
#[tauri::command]
fn preview(tool: String, params: HashMap<String, String>, inputs: Vec<String>) -> Result<Vec<(String, String)>, String> {
    let t = tools::find(&tool).ok_or("unknown tool")?;
    let job = tools::Job {
        inputs: inputs.iter().map(PathBuf::from).collect(),
        out: PathBuf::new(),
        tmp: std::env::temp_dir(),
        params: tools::with_defaults(t, params),
    };
    tools::planned_renames(&job, (t.build)(&job))
}

/// Stops a running job (the bench's Stop button).
#[tauri::command]
fn stop(job: String) {
    runner::stop(&job);
}

#[derive(Serialize)]
struct DownloadInfo {
    name: &'static str,
    mb: u64,
    /// "downloaded" (by Remise, can be removed), "present" (bundled or installed on the system),
    /// or "missing".
    state: &'static str,
}

/// What a striped tool can fetch itself instead of asking for an install, by bin name.
#[tauri::command]
fn downloads() -> HashMap<&'static str, DownloadInfo> {
    let state = |b| match (fetch::installed(b).is_some(), bins::available(b)) {
        (true, _) => "downloaded",
        (false, true) => "present",
        _ => "missing",
    };
    tools::all()
        .flat_map(|t| t.bins.iter())
        .filter_map(|b| fetch::find(b))
        .map(|d| (d.bin, DownloadInfo { name: d.name, mb: d.bytes.div_ceil(1_000_000), state: state(d.bin) }))
        .collect()
}

/// Deletes a download (Settings → Downloads). Only names from the download list are accepted.
#[tauri::command]
async fn remove_download(bin: String) -> Result<(), String> {
    let d = fetch::find(&bin).ok_or("nothing downloaded under that name")?;
    tauri::async_runtime::spawn_blocking(move || fetch::remove(d)).await.map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct About {
    version: String,
    /// Name and version, licence: the programs in `vendor/`, from its THIRD_PARTY.md.
    bundled: Vec<(String, String)>,
    /// That file, with the source links and where the licence texts are.
    credits: Option<String>,
    /// Where the command line lives, as a terminal would type it.
    command: Option<String>,
}

#[tauri::command]
fn about(app: AppHandle) -> About {
    let credits = bins::vendor().map(|v| v.join("THIRD_PARTY.md")).filter(|p| p.is_file());
    let md = credits.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    About {
        version: app.package_info().version.to_string(),
        bundled: third_party(&md),
        credits: credits.map(|p| tools::s(&p)),
        command: command_path().map(|p| tools::s(&p)),
    }
}

/// `## Name Version` headings and their `- Licence:` line.
fn third_party(md: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in md.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            out.push((name.trim().into(), String::new()));
        } else if let (Some(licence), Some(last)) = (line.strip_prefix("- Licence: "), out.last_mut()) {
            last.1 = licence.trim().into();
        }
    }
    out
}

/// Windows: bin\remise.exe from the installer (remise-cli.exe in a build). Linux: the AppImage
/// itself when run from one. Otherwise the app's own executable, which is the command line.
fn command_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    if cfg!(windows) {
        let dir = exe.parent()?;
        return [dir.join("bin").join("remise.exe"), dir.join("remise-cli.exe")].into_iter().find(|p| p.is_file());
    }
    Some(std::env::var_os("APPIMAGE").map(PathBuf::from).unwrap_or(exe))
}

/// Downloads what `bin` needs; progress comes as job `dl-<bin>`.
#[tauri::command]
async fn download(app: AppHandle, bin: String) -> Result<(), String> {
    let d = fetch::find(&bin).ok_or("nothing to download for that")?;
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |p: &runner::Progress| {
            let _ = app.emit("progress", p);
        };
        fetch::fetch(&emit, d).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The app, or the command line when there are arguments that aren't files to open.
pub fn start(args: &[String]) {
    let context = tauri::generate_context!();
    if cli::wanted(args) {
        std::process::exit(cli::main(args, context.package_info(), &context.config().identifier));
    }
    run_app(context)
}

fn run_app(context: tauri::Context) {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            bins::set_vendor(app.path().resource_dir().ok().map(|d| d.join("vendor")));
            bins::set_user_tools(app.path().app_local_data_dir().ok().map(|d| d.join("tools")));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_tools, run, stop, preview, downloads, download, remove_download, about])
        .run(context)
        .expect("error while running tauri application");
}
