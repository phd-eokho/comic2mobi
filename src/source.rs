//! Gathers the page images of one book into its work directory.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::Chars;

use crate::image::{self, Image};
use crate::tool;

const ARCHIVE_EXTENSIONS: [&str; 6] = ["zip", "cbz", "rar", "cbr", "7z", "cb7"];

pub struct Page {
    /// File name of the image inside the work directory.
    pub file: String,
    /// Original file name without extension, shown in the table of contents.
    pub label: String,
    pub image: Image,
}

pub fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ARCHIVE_EXTENSIONS
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

/// Puts every image of `input` (a directory, or an archive unpacked with
/// 7-Zip) into `work` under a sequential name, in natural reading order.
pub fn collect(input: &Path, work: &Path, sevenzip: Option<&Path>) -> Result<Vec<Page>, String> {
    let extracted = work.join("src");
    let root = if input.is_dir() {
        input
    } else {
        extract(sevenzip.ok_or("7-Zip not found")?, input, &extracted)?;
        &extracted
    };

    let mut files = Vec::new();
    walk(root, &mut files).map_err(|e| format!("cannot read {}: {e}", root.display()))?;
    let mut files: Vec<(String, PathBuf)> = files
        .into_iter()
        .map(|path| {
            (
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
                path,
            )
        })
        .collect();
    files.sort_by(|a, b| natural_cmp(&a.0, &b.0));

    let mut pages = Vec::new();
    for (_, path) in files {
        let read_error = |e: io::Error| format!("cannot read {}: {e}", path.display());
        let Some(image) = image::probe(&path).map_err(read_error)? else {
            continue;
        };
        let file = format!("{:05}.{}", pages.len(), image.format.extension());
        let dest = work.join(&file);
        fs::hard_link(&path, &dest)
            .or_else(|_| fs::copy(&path, &dest).map(drop))
            .map_err(read_error)?;
        let label = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        pages.push(Page { file, label, image });
    }
    if pages.is_empty() {
        return Err("no images found".into());
    }
    Ok(pages)
}

fn extract(sevenzip: &Path, archive: &Path, dest: &Path) -> Result<(), String> {
    let mut output_dir = OsString::from("-o");
    output_dir.push(dest);
    let output = Command::new(sevenzip)
        .args(["x", "-y", "-bd"])
        .arg(output_dir)
        .arg("--")
        .arg(archive)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run {}: {e}", sevenzip.display()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "7-Zip could not extract the archive:\n{}",
            tool::tail(&output)
        ))
    }
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(&entry.path(), files)?;
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}

/// Orders names the way a reader expects: `page2` before `page10`.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut left, mut right) = (a.chars().peekable(), b.chars().peekable());
    loop {
        let ordering = match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(l), Some(r)) if l.is_ascii_digit() && r.is_ascii_digit() => {
                let (l, r) = (take_number(&mut left), take_number(&mut right));
                l.len().cmp(&r.len()).then_with(|| l.cmp(&r))
            }
            (Some(l), Some(r)) => {
                left.next();
                right.next();
                l.to_lowercase().cmp(r.to_lowercase())
            }
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
}

/// Consumes a run of digits and returns it without leading zeros.
fn take_number(chars: &mut Peekable<Chars>) -> String {
    let mut digits = String::new();
    while let Some(digit) = chars.next_if(char::is_ascii_digit) {
        if digit != '0' || !digits.is_empty() {
            digits.push(digit);
        }
    }
    digits
}
