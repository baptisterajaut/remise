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
        multi: true,
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
        multi: false,
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
        multi: true,
        params: &[
            Param::Text { key: "pattern", label: "Pattern", default: "{name}-{n}", placeholder: "holiday-{n}" },
            Param::Text { key: "start", label: "First number", default: "1", placeholder: "1" },
        ],
        output: Output::None,
        // CBA: no live preview / undo, the patterns are simple enough to eyeball
        build: |j| {
            let start: usize = j.p("start").trim().parse().unwrap_or(1);
            let width = (start + j.inputs.len()).saturating_sub(1).to_string().len().max(2);
            j.inputs
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
                .collect()
        },
    },
];
