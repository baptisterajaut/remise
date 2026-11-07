// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Lossy: a non-UTF-8 path on Unix shouldn't panic before anything starts.
    let args: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    // Remise runs itself as a child process for background removal (see cutout.rs).
    if args.first().map(String::as_str) == Some("--cutout") {
        if let Err(e) = remise_lib::cutout(&args[1..]) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    remise_lib::start(&args)
}
