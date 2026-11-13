//! Converts one book: images -> OPF/NCX/HTML -> kindlegen -> kindlestrip.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::source::Page;
use crate::{kindlestrip, opf, source, tool};

const MOBI_NAME: &str = "book.mobi";

pub struct Options {
    pub output: PathBuf,
    pub author: String,
    pub strip: bool,
    pub kindlegen: PathBuf,
    pub sevenzip: Option<PathBuf>,
}

/// A directory that is deleted, with its contents, when the value is dropped.
pub struct ScratchDir(PathBuf);

impl ScratchDir {
    pub fn create(path: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// An unfinished output file, deleted on drop unless it was renamed away.
struct PartialFile(PathBuf);

impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// The book title, which also names the output file: the directory name, or
/// the archive name without its extension.
pub fn title(input: &Path) -> Option<String> {
    let name = if input.is_dir() {
        input.file_name()
    } else {
        input.file_stem()
    };
    Some(name?.to_string_lossy().into_owned())
}

/// Builds `<output>/<title>.mobi` from `input`, using `work` as a private
/// scratch directory. Returns the path of the finished book.
pub fn convert(input: &Path, work: PathBuf, options: &Options) -> Result<PathBuf, String> {
    let title = title(input).ok_or("cannot derive a book name from the path")?;
    let work =
        ScratchDir::create(work).map_err(|e| format!("cannot create a work directory: {e}"))?;
    let work = work.path();

    let pages = source::collect(input, work, options.sevenzip.as_deref())?;
    write_sources(work, &title, &options.author, &pages)
        .map_err(|e| format!("cannot write to {}: {e}", work.display()))?;
    let mobi = run_kindlegen(&options.kindlegen, work)?;

    // Written under a temporary name first, so a failure never leaves a
    // truncated book or destroys one from an earlier run.
    let dest = options.output.join(format!("{title}.mobi"));
    let partial = PartialFile(options.output.join(format!("{title}.mobi.part")));
    let saved = if options.strip {
        save_stripped(&mobi, &partial.0)
    } else {
        fs::copy(&mobi, &partial.0).map(drop)
    };
    saved
        .and_then(|()| fs::rename(&partial.0, &dest))
        .map_err(|e| format!("cannot write {}: {e}", dest.display()))?;
    Ok(dest)
}

fn write_sources(work: &Path, title: &str, author: &str, pages: &[Page]) -> io::Result<()> {
    fs::write(work.join(opf::OPF_NAME), opf::opf(title, author, pages))?;
    fs::write(work.join(opf::NCX_NAME), opf::ncx(title, author, pages))?;
    for (index, page) in pages.iter().enumerate() {
        fs::write(work.join(opf::page_name(index)), opf::page_html(page))?;
    }
    Ok(())
}

/// Runs inside the work directory with plain relative names, so the same call
/// works for a Linux kindlegen and for kindlegen.exe under WSL.
fn run_kindlegen(kindlegen: &Path, work: &Path) -> Result<PathBuf, String> {
    let output = Command::new(kindlegen)
        .current_dir(work)
        .args([opf::OPF_NAME, "-c1", "-o", MOBI_NAME])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run {}: {e}", kindlegen.display()))?;
    let mobi = work.join(MOBI_NAME);
    // kindlegen also exits non-zero for mere warnings; the file is what counts.
    if !mobi.is_file() {
        return Err(format!(
            "kindlegen produced no book:\n{}",
            tool::tail(&output)
        ));
    }
    Ok(mobi)
}

fn save_stripped(mobi: &Path, dest: &Path) -> io::Result<()> {
    let data = fs::read(mobi)?;
    let stripped =
        kindlestrip::strip(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let Some(stripped) = stripped else {
        return fs::write(dest, &data);
    };
    let mut file = File::create(dest)?;
    file.write_all(&stripped.head)?;
    stripped
        .body
        .iter()
        .try_for_each(|part| file.write_all(part))
}
