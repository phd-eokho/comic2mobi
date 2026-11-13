//! Locating and reporting on the external programs (kindlegen, 7-Zip).

use std::env;
use std::path::PathBuf;
use std::process::Output;

/// Returns the first of `names` found on `PATH`, as an absolute path.
pub fn find(names: &[&str]) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for name in names {
        for dir in env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return candidate.canonicalize().ok();
            }
        }
    }
    None
}

const TAIL_LINES: usize = 8;

/// The last lines a failed program printed, for an error message.
pub fn tail(output: &Output) -> String {
    let text = [&output.stdout[..], &output.stderr[..]].concat();
    let text = String::from_utf8_lossy(&text);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n")
}
