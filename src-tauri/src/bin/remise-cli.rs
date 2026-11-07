//! The `remise` command on Windows (built as remise-cli.exe, installed as bin\remise.exe): the
//! console front of Remise.exe. Remise.exe is a GUI program, so a terminal neither shows its output
//! nor waits for it. This one runs it with the terminal's own stdin/stdout/stderr, waits, and exits
//! with its status.

use std::path::{Path, PathBuf};
use std::process::{Command, exit};

fn main() {
    let Some(app) = app() else {
        eprintln!("can't find Remise next to this program or one folder up");
        exit(1);
    };
    let mut child = match Command::new(&app).args(std::env::args_os().skip(1)).spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("can't start {}: {e}", app.display());
            exit(1);
        }
    };
    #[cfg(windows)]
    let _job = tie_to_this_process(&child);
    exit(child.wait().ok().and_then(|s| s.code()).unwrap_or(1));
}

/// Built, the app sits next to this program; installed, one folder up. Windows ignores case, so
/// bin\remise.exe must not take itself for the app (it would start itself forever).
fn app() -> Option<PathBuf> {
    let me = std::env::current_exe().unwrap_or_default();
    let name = if cfg!(windows) { "Remise.exe" } else { "remise" };
    let dir = me.parent().unwrap_or(Path::new("."));
    let lower = |p: &Path| p.to_string_lossy().to_lowercase();
    [Some(dir), dir.parent()].into_iter().flatten().map(|d| d.join(name)).find(|p| p.is_file() && lower(p) != lower(&me))
}

/// Remise.exe and the programs it starts end with this process: Ctrl+C or a closed terminal ends
/// this one, the job object's last handle closes, and Windows kills the rest.
#[cfg(windows)]
fn tie_to_this_process(child: &std::process::Child) -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::*;
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return None;
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = std::mem::size_of_val(&limits) as u32;
        SetInformationJobObject(job, JobObjectExtendedLimitInformation, (&raw const limits).cast(), size);
        AssignProcessToJobObject(job, child.as_raw_handle());
        Some(job)
    }
}
