//! EPUB → one HTML page that Chromium can print. An EPUB is a zip of XHTML chapters listed,
//! in reading order, by the OPF "spine"; we stitch the chapter bodies together.

use super::{chromium, s, Job, Step};
use std::path::{Path, PathBuf};

/// Value of `name="…"` (or single-quoted) inside one tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    [format!(" {name}=\""), format!(" {name}='")].iter().find_map(|pat| {
        let start = tag.find(pat.as_str())? + pat.len();
        let quote = pat.chars().last()?;
        let end = tag[start..].find(quote)? + start;
        Some(tag[start..end].replace("&amp;", "&"))
    })
}

/// Every `<name …>` opening tag, as text.
fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        if after.starts_with(|c: char| c.is_whitespace() || c == '/' || c == '>') {
            let end = after.find('>').unwrap_or(after.len());
            out.push(&after[..end]);
        }
        rest = after;
    }
    out
}

/// Path of the OPF package inside the book, from META-INF/container.xml.
pub fn rootfile(container: &str) -> Option<String> {
    tags(container, "rootfile").into_iter().find_map(|t| attr(t, "full-path"))
}

/// Chapter hrefs in reading order.
pub fn spine(opf: &str) -> Vec<String> {
    let items: Vec<(String, String)> = tags(opf, "item")
        .into_iter()
        .filter_map(|t| Some((attr(t, "id")?, attr(t, "href")?)))
        .collect();
    tags(opf, "itemref")
        .into_iter()
        .filter_map(|t| attr(t, "idref"))
        .filter_map(|id| items.iter().find(|(i, _)| *i == id).map(|(_, h)| h.replace("%20", " ")))
        .collect()
}

/// Inner HTML of `<body>`, or the whole text if there is none.
pub fn body(xhtml: &str) -> &str {
    let Some(open) = xhtml.find("<body") else { return xhtml };
    let start = xhtml[open..].find('>').map_or(open, |i| open + i + 1);
    let end = xhtml.rfind("</body>").unwrap_or(xhtml.len());
    &xhtml[start..end.max(start)]
}

/// Stylesheet `<link>` tags of a chapter.
fn stylesheets(xhtml: &str) -> Vec<String> {
    tags(xhtml, "link")
        .into_iter()
        .filter(|t| t.contains("stylesheet"))
        .map(|t| format!("<link{t}>"))
        .collect()
}

/// One printable page from the chapters, page-broken between chapters.
pub fn merge(chapters: &[String]) -> String {
    let mut css: Vec<String> = chapters.iter().flat_map(|c| stylesheets(c)).collect();
    css.dedup();
    let bodies: Vec<String> = chapters
        .iter()
        .map(|c| format!("<section style=\"break-after: page\">{}</section>", body(c)))
        .collect();
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">{}</head><body>{}</body></html>",
        css.join(""),
        bodies.join("\n")
    )
}

/// After `7z x` unpacked the book into tmp/book: stitch it and print it.
pub fn plan(j: &Job) -> Result<Vec<Step>, String> {
    let book = j.tmp.join("book");
    let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("not a readable EPUB ({}: {e})", p.display()));
    let opf_rel = rootfile(&read(&book.join("META-INF/container.xml"))?).ok_or("EPUB without a package file")?;
    let opf_path = book.join(&opf_rel);
    let opf_dir = opf_path.parent().unwrap_or(&book).to_path_buf();
    let hrefs = spine(&read(&opf_path)?);
    if hrefs.is_empty() {
        return Err("EPUB with an empty reading order".into());
    }
    let files: Vec<PathBuf> = hrefs.iter().map(|h| opf_dir.join(h)).collect();
    let chapters = files.iter().map(|f| read(f)).collect::<Result<Vec<_>, _>>()?;
    // Next to the first chapter, so its relative images and CSS still resolve.
    let page = files[0].parent().unwrap_or(&opf_dir).join("_remise_book.html");
    let print = chromium(j, s(&page), args!["--no-pdf-header-footer", format!("--print-to-pdf={}", j.out())]);
    Ok(vec![Step::Write(page, merge(&chapters)), print])
}
