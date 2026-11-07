use crate::cli::{Call, help, list, parse, wanted};
use crate::tools::{self, Param};
use std::path::PathBuf;

fn args(s: &[&str]) -> Vec<String> {
    s.iter().map(|a| a.to_string()).collect()
}

#[test]
fn the_app_opens_without_arguments_or_with_files() {
    assert!(!wanted(&[]));
    assert!(!wanted(&args(&["-psn_0_12345"])), "a Finder launch");
    let file = std::env::current_exe().unwrap();
    assert!(!wanted(&[file.to_string_lossy().into_owned()]), "a file to open");
    assert!(wanted(&args(&["list"])) && wanted(&args(&["invalid"])) && wanted(&args(&["--help"])));
}

#[test]
fn options_are_checked_against_the_tool() {
    let t = tools::find("pdf-pdfa").unwrap();
    let call = parse(t, &args(&["--level", "1", "a.pdf", "--level=3", "b.pdf"]), None).unwrap();
    let want =
        Call { params: [("level".to_string(), "3".to_string())].into(), files: vec![PathBuf::from("a.pdf"), PathBuf::from("b.pdf")] };
    assert_eq!(call, want, "the last value wins, files anywhere");
    assert!(parse(t, &args(&["--level", "9"]), None).is_err_and(|e| e.contains("2, 1, 3")));
    assert!(parse(t, &args(&["--color", "red"]), None).is_err_and(|e| e.contains("remise help pdf-pdfa")));
    assert!(parse(t, &args(&["--level"]), None).is_err_and(|e| e.contains("needs a value")));
    // After `--`, a name that looks like an option is a file.
    assert_eq!(parse(t, &args(&["--", "--odd.pdf"]), None).unwrap().files, [PathBuf::from("--odd.pdf")]);
}

#[test]
fn passwords_come_from_the_environment_only() {
    let t = tools::find("arc-create").unwrap();
    assert!(parse(t, &args(&["--password", "x", "a"]), None).is_err_and(|e| e.contains("REMISE_PASSWORD")));
    let call = parse(t, &args(&["a"]), Some("s3cret".into())).unwrap();
    assert_eq!(call.params.get("password").map(String::as_str), Some("s3cret"));
    // A tool without one doesn't get it.
    let pdfa = tools::find("pdf-pdfa").unwrap();
    assert!(!parse(pdfa, &args(&["a.pdf"]), Some("s3cret".into())).unwrap().params.contains_key("password"));
}

#[test]
fn help_and_list_cover_every_tool_and_option() {
    let listed = list();
    for t in tools::all() {
        assert!(listed.contains(&format!("  {:<20} {}", t.id, t.name)), "{} not listed", t.id);
        let text = help(t);
        for p in t.params {
            let shown = match p {
                Param::Text { key: "password", .. } => "REMISE_PASSWORD".to_string(),
                _ => format!("--{}", p.key()),
            };
            assert!(text.contains(&shown), "{}: {shown} missing from help", t.id);
        }
        assert!(text.contains(&format!("remise {}", t.id)), "{}: no example", t.id);
    }
}
