//! Files tools.

use super::*;

pub static TOOLS: &[Tool] = &[
    Tool {
        id: "arc-create",
        family: Family::Files,
        name: "Create archive",
        blurb: "Zip or 7z everything you drop.",
        bins: &["7z"],
        accepts: &[],
        inputs: Inputs::Many,
        params: &[
            Param::Select { key: "format", label: "Format", options: &[("zip", "ZIP"), ("7z", "7z")], default: "zip" },
            Param::Text { key: "password", label: "Password", default: "", placeholder: "none" },
        ],
        output: Output::File { suffix: "archive", ext: fmt },
        // AES-256 either way. 7z also hides the file names; a zip shows them to anyone.
        build: |j| {
            if j.p("format") == "zip" && !j.p("password").is_ascii() {
                // 7-Zip's answer is E_INVALIDARG: zip never said how to encode the rest.
                return vec![Step::Fail("a zip password takes no accents or symbols beyond ASCII; 7z takes any".into())];
            }
            let mut a = args!["a", format!("-t{}", j.p("format")), "-y"];
            if !j.p("password").is_empty() {
                // `-p` alone: 7-Zip asks for it.
                a.extend(args!["-p", if j.p("format") == "7z" { "-mhe=on" } else { "-mem=AES256" }]);
            }
            a.push(j.out());
            a.extend(j.inputs.iter().map(|p| s(p)));
            seven_zip(a, j.p("password"))
        },
    },
    Tool {
        id: "arc-extract",
        family: Family::Files,
        name: "Extract archive",
        blurb: "zip, 7z, rar, tar, iso…",
        bins: &["7z"],
        accepts: ARCHIVES,
        inputs: Inputs::One,
        params: &[Param::Text { key: "password", label: "Password", default: "", placeholder: "if it has one" }],
        output: Output::Dir("extracted"),
        // 7-Zip only asks when the archive is locked.
        build: |j| seven_zip(args!["x", "-y", format!("-o{}", j.out()), j.input()], j.p("password")),
    },
    Tool {
        id: "batch-rename",
        family: Family::Files,
        name: "Batch rename",
        blurb: "{name} and {n} patterns, extension kept.",
        bins: &[],
        accepts: &[],
        inputs: Inputs::Many,
        params: &[
            Param::Text { key: "pattern", label: "Pattern", default: "{name}-{n}", placeholder: "holiday-{n}" },
            Param::Text { key: "start", label: "First number", default: "1", placeholder: "1" },
        ],
        output: Output::None,
        // CBA: no undo; the bench previews every new name before Run
        build: |j| {
            let pattern = j.p("pattern");
            if pattern.contains(['/', '\\']) || pattern.contains("..") || pattern.trim().is_empty() {
                return vec![Step::Fail("the pattern is a file name: no slashes, no ..".into())];
            }
            if j.inputs.len() > 1 && !pattern.contains("{n}") && !pattern.contains("{name}") {
                return vec![Step::Fail("with several files, put {n} or {name} in the pattern".into())];
            }
            let start: usize = j.p("start").trim().parse().unwrap_or(1);
            let width = (start + j.inputs.len()).saturating_sub(1).to_string().len().max(2);
            let renames = j
                .inputs
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let n = format!("{:0width$}", start + i);
                    let mut name = j.p("pattern").replace("{name}", &stem_of(p)).replace("{n}", &n);
                    let ext = p.extension().map(|e| e.to_string_lossy().into_owned());
                    if let Some(ext) = ext {
                        name = format!("{name}.{ext}");
                    }
                    Step::Rename(p.clone(), p.with_file_name(name))
                })
                .collect();
            all_or_nothing(renames)
        },
    },
    Tool {
        id: "file-dates",
        family: Family::Files,
        name: "Change file dates",
        blurb: "Set the modified date of everything you drop.",
        bins: &[],
        accepts: &[],
        inputs: Inputs::Many,
        params: &[Param::Text { key: "date", label: "Date", default: "2024-01-01 12:00", placeholder: "YYYY-MM-DD HH:MM" }],
        output: Output::None,
        // The date is wall-clock time where the user is: `tz`, minutes east of UTC on that date
        // (summer time included), comes from the webview, which knows the local time zone.
        build: |j| match parse_date(j.p("date")) {
            None => vec![Step::Fail(format!("`{}` isn't a YYYY-MM-DD HH:MM date", j.p("date")))],
            Some(utc) => {
                let east = std::time::Duration::from_secs(j.p("tz").trim().parse::<i64>().unwrap_or(0).unsigned_abs() * 60);
                let when = if j.p("tz").trim().starts_with('-') { utc + east } else { utc - east };
                j.inputs.iter().map(|p| Step::Touch(p.clone(), when)).collect()
            }
        },
    },
    Tool {
        id: "file-sort",
        family: Family::Files,
        name: "Sort & route",
        blurb: "File a messy folder: by kind, extension or month.",
        bins: &[],
        accepts: &[],
        inputs: Inputs::Many,
        params: &[Param::Select {
            key: "by",
            label: "Into folders by",
            options: &[("kind", "Kind"), ("ext", "Extension"), ("month", "Month modified")],
            default: "kind",
        }],
        output: Output::None,
        build: |_| vec![Step::Then(sort_plan)],
    },
    Tool {
        id: "file-checksum",
        family: Family::Files,
        name: "Checksum",
        blurb: "SHA-256: write a .sha256 next to each file, or check one.",
        bins: &[],
        accepts: &[],
        inputs: Inputs::One,
        params: &[Param::Select { key: "mode", label: "Mode", options: &[("make", "Write .sha256"), ("check", "Check")], default: "make" }],
        output: Output::None,
        build: |j| vec![Step::Then(if j.p("mode") == "check" { check_sum } else { write_sum })],
    },
];

/// `sha256sum`'s format, so the file checks with it too. Written aside, then moved next to the
/// file: an existing .sha256 is never overwritten.
fn write_sum(j: &Job) -> Result<Vec<Step>, String> {
    let file = &j.inputs[0];
    if ext_of(file) == "sha256" {
        return Err("this is already a checksum: use Check".into());
    }
    let hash = crate::fetch::sha256(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let name = file.file_name().ok_or("odd file name")?.to_string_lossy();
    let aside = j.tmp.join("sum");
    Ok(vec![Step::Write(aside.clone(), format!("{hash}  {name}\n")), Step::Rename(aside, sum_of(file))])
}

/// A dropped file is checked against the .sha256 next to it; a dropped .sha256, every file it
/// lists. Success leaves nothing behind; a mismatch fails, naming the files.
fn check_sum(j: &Job) -> Result<Vec<Step>, String> {
    let dropped = &j.inputs[0];
    let (list, only) = if ext_of(dropped) == "sha256" { (dropped.clone(), None) } else { (sum_of(dropped), Some(dropped)) };
    let text = std::fs::read_to_string(&list).map_err(|_| format!("no {} next to it", list.display()))?;
    let entries = parse_sums(&list, &text)?;
    // Lists name files as they were typed (`./a.txt`): compared as real paths.
    let real = |p: &PathBuf| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
    let wanted: Vec<_> = entries.iter().filter(|(_, f)| only.is_none_or(|o| real(o) == real(f))).collect();
    if wanted.is_empty() {
        return Err(format!("{} doesn't list this file", list.display()));
    }
    let bad: Vec<String> = wanted
        .into_iter()
        .filter(|(hash, f)| !crate::fetch::sha256(f).is_ok_and(|h| h.eq_ignore_ascii_case(hash)))
        .map(|(_, f)| f.file_name().unwrap_or_default().to_string_lossy().into_owned())
        .collect();
    if bad.is_empty() { Ok(vec![]) } else { Err(format!("doesn't match (changed, damaged or missing): {}", bad.join(", "))) }
}

fn sum_of(file: &std::path::Path) -> PathBuf {
    let mut name = file.as_os_str().to_owned();
    name.push(".sha256");
    PathBuf::from(name)
}

/// `HASH  name` (or ` *name`, binary mode) per line, as `sha256sum` writes; a bare hash, as many
/// download pages publish, is the file the list is named after.
pub fn parse_sums(list: &std::path::Path, text: &str) -> Result<Vec<(String, PathBuf)>, String> {
    let dir = list.parent().unwrap_or(std::path::Path::new(""));
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|line| {
            let (hash, name) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("{} isn't a SHA-256 list", list.display()));
            }
            let name = name.trim_start().trim_start_matches('*');
            let file = if name.is_empty() { list.with_extension("") } else { dir.join(name) };
            Ok((hash.to_string(), file))
        })
        .collect()
}

/// 7-Zip with the password on stdin, never argv, which anyone can read in `ps`. Always answered,
/// empty if need be: with nothing on stdin a locked archive gets "Break signaled" instead of
/// "Wrong password".
fn seven_zip(mut args: Vec<String>, pw: &str) -> Vec<Step> {
    if pw.contains(['\n', '\r']) {
        return vec![Step::Fail("a password can't span several lines".into())];
    }
    // Windows' 7z reads stdin in the OEM code page, so "été" wouldn't be the password another
    // tool typed. The Unix build has no such switch (and reads UTF-8).
    if cfg!(windows) {
        args.insert(1, "-sccUTF-8".into());
    }
    vec![Step::Feed("7z", args, format!("{pw}\n"))]
}

/// Renames are only started once every target is known to be free and distinct, so a batch never
/// stops half-way with some files renamed and others not.
fn all_or_nothing(steps: Vec<Step>) -> Vec<Step> {
    let mut seen = std::collections::HashSet::new();
    for step in &steps {
        if let Step::Rename(from, to) = step
            && (from != to && (to.exists() || !seen.insert(to.clone())))
        {
            return vec![Step::Fail(format!("{} already exists; nothing was renamed", to.display()))];
        }
    }
    steps
}

/// Folder a file goes to. `month` comes from the file's modified time, read at run time.
fn sort_plan(j: &Job) -> Result<Vec<Step>, String> {
    j.inputs
        .iter()
        .filter(|p| p.is_file())
        .map(|p| {
            let folder = match j.p("by") {
                "ext" => {
                    let e = ext_of(p).to_uppercase();
                    if e.is_empty() { "No extension".into() } else { e }
                }
                "month" => {
                    let modified = std::fs::metadata(p).and_then(|m| m.modified()).map_err(|e| e.to_string())?;
                    month_of(modified)
                }
                _ => kind_of(&ext_of(p)).into(),
            };
            let name = p.file_name().ok_or("odd file name")?;
            Ok(Step::Rename(p.clone(), p.with_file_name(folder).join(name)))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(all_or_nothing)
}

pub fn kind_of(ext: &str) -> &'static str {
    let is = |list: &[&str]| list.contains(&ext);
    if is(IMG) || is(&["svg", "raw", "cr2", "nef", "dng", "psd"]) {
        "Images"
    } else if is(VID) {
        "Videos"
    } else if is(AUD) {
        "Audio"
    } else if is(&["pdf", "doc", "docx", "odt", "rtf", "txt", "md", "xls", "xlsx", "ods", "csv", "ppt", "pptx", "odp", "epub"]) {
        "Documents"
    } else if is(ARCHIVES) {
        "Archives"
    } else {
        "Other"
    }
}

/// `YYYY-MM` (UTC) of a timestamp.
pub fn month_of(t: std::time::SystemTime) -> String {
    let days = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // Civil from days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}")
}
