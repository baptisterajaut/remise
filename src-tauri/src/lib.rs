#![warn(clippy::cognitive_complexity, clippy::too_many_lines)]

mod bins;
mod runner;
mod tools;

#[cfg(test)]
mod tests;

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tools::{Family, Inputs, Output, Param};

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
    let Output::File { ext, .. } = &t.output else { return None };
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
            missing: t
                .bins
                .iter()
                .copied()
                .filter(|b| !*found.entry(b).or_insert_with(|| bins::available(b)))
                .collect(),
        })
        .collect()
}

#[tauri::command]
async fn run(app: AppHandle, job: String, steps: Vec<runner::StepReq>, inputs: Vec<String>) -> Result<Vec<String>, String> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run_app() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            bins::set_vendor(app.path().resource_dir().ok().map(|d| d.join("vendor")));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_tools, run])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
