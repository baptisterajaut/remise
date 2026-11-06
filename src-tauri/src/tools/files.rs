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
        params: &[Param::Select {
            key: "format",
            label: "Format",
            options: &[("zip", "ZIP"), ("7z", "7z")],
            default: "zip",
        }],
        output: Output::File { suffix: "archive", ext: fmt },
        build: |j| {
            let mut a = args!["a", format!("-t{}", j.p("format")), "-y", j.out()];
            a.extend(j.inputs.iter().map(|p| s(p)));
            vec![Step::Run("7z", a)]
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
        params: &[],
        output: Output::Dir("extracted"),
        build: |j| vec![Step::Run("7z", args!["x", "-y", format!("-o{}", j.out()), j.input()])],
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
        // CBA: no live preview / undo, the patterns are simple enough to eyeball
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
        blurb: "Set the modified date of everything you drop (UTC).",
        bins: &[],
        accepts: &[],
        inputs: Inputs::Many,
        params: &[Param::Text { key: "date", label: "Date", default: "2024-01-01 12:00", placeholder: "YYYY-MM-DD HH:MM" }],
        output: Output::None,
        build: |j| match parse_date(j.p("date")) {
            None => vec![Step::Fail(format!("`{}` isn't a YYYY-MM-DD HH:MM date", j.p("date")))],
            Some(when) => j.inputs.iter().map(|p| Step::Touch(p.clone(), when)).collect(),
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
];

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
