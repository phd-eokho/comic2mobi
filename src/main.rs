//! comic2mobi: converts folders or archives of page images to Kindle MOBI
//! books, several books at a time.

mod book;
mod image;
mod kindlestrip;
mod opf;
mod source;
mod tool;

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use book::ScratchDir;

const USAGE: &str = "\
Usage: comic2mobi [OPTIONS] <INPUT>...

Converts each INPUT to <output>/<name>.mobi. An INPUT is a directory of page
images (jpg, png, gif, bmp) or an archive of them (zip, cbz, rar, cbr, 7z, cb7).

Options:
  -o, --output <DIR>   Directory for the .mobi files [default: .]
  -j, --jobs <N>       Books converted in parallel [default: CPU count]
  -a, --author <NAME>  Author stored in the books [default: Unknown]
      --no-strip       Keep the source copy kindlegen embeds (larger files)
  -h, --help           Print this help
  -V, --version        Print the version

Requires kindlegen (on PATH, or set KINDLEGEN to its path) and, for archives,
7-Zip (7zz, 7z or 7za on PATH).
";

const EXIT_USAGE: u8 = 2;

struct Args {
    inputs: Vec<PathBuf>,
    output: PathBuf,
    jobs: usize,
    author: String,
    strip: bool,
}

/// What became of one input: its index, the finished book or the reason it
/// failed, and how long that took.
type Outcome = (usize, Result<PathBuf, String>, Duration);

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("comic2mobi: {message}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Returns whether every book converted.
fn run() -> Result<bool, String> {
    let Some(args) = parse_args()? else {
        return Ok(true);
    };
    let inputs = resolve_inputs(&args.inputs)?;
    let kindlegen = find_kindlegen()?;
    let sevenzip = tool::find(&["7zz", "7z", "7za"]);
    if sevenzip.is_none() && inputs.iter().any(|input| !input.is_dir()) {
        return Err("7-Zip not found; archives need 7zz, 7z or 7za on PATH".into());
    }
    fs::create_dir_all(&args.output)
        .map_err(|e| format!("cannot create {}: {e}", args.output.display()))?;
    let options = book::Options {
        output: args.output,
        author: args.author,
        strip: args.strip,
        kindlegen,
        sevenzip,
    };
    let scratch = ScratchDir::create(env::temp_dir().join(format!("comic2mobi-{}", process::id())))
        .map_err(|e| format!("cannot create a work directory: {e}"))?;

    // Workers claim books through the shared cursor and send each outcome
    // back; this thread only reports.
    let next = AtomicUsize::new(0);
    let (outcomes, finished) = mpsc::sync_channel(inputs.len());
    let failures = thread::scope(|scope| {
        for _ in 0..args.jobs.min(inputs.len()) {
            let (inputs, next, scratch, options, outcomes) =
                (&inputs, &next, scratch.path(), &options, outcomes.clone());
            scope.spawn(move || convert_books(inputs, next, scratch, options, outcomes));
        }
        drop(outcomes);
        finished
            .iter()
            .filter(|outcome| !report(&inputs, outcome))
            .count()
    });
    Ok(failures == 0)
}

fn convert_books(
    inputs: &[PathBuf],
    next: &AtomicUsize,
    scratch: &Path,
    options: &book::Options,
    outcomes: SyncSender<Outcome>,
) {
    loop {
        let index = next.fetch_add(1, Ordering::Relaxed);
        let Some(input) = inputs.get(index) else {
            return;
        };
        let started = Instant::now();
        let result = book::convert(input, scratch.join(format!("{index:03}")), options);
        if outcomes.send((index, result, started.elapsed())).is_err() {
            return;
        }
    }
}

/// Prints one outcome and returns whether the book converted.
fn report(inputs: &[PathBuf], (index, result, elapsed): &Outcome) -> bool {
    match result {
        Ok(dest) => println!("ok    {} ({:.1}s)", dest.display(), elapsed.as_secs_f32()),
        Err(message) => eprintln!("FAIL  {}: {message}", inputs[*index].display()),
    }
    result.is_ok()
}

/// Makes the inputs absolute and rejects what cannot be converted, including
/// two inputs that would produce the same output file.
fn resolve_inputs(raw: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut titles = HashSet::with_capacity(raw.len());
    raw.iter()
        .map(|path| {
            let input = path
                .canonicalize()
                .map_err(|e| format!("{}: {e}", path.display()))?;
            if !input.is_dir() && !source::is_archive(&input) {
                return Err(format!(
                    "{}: not a directory or a supported archive",
                    path.display()
                ));
            }
            let title = book::title(&input)
                .ok_or_else(|| format!("{}: cannot derive a book name", path.display()))?;
            if !titles.insert(title) {
                return Err(format!(
                    "{}: another input has the same name",
                    path.display()
                ));
            }
            Ok(input)
        })
        .collect()
}

fn find_kindlegen() -> Result<PathBuf, String> {
    let Some(path) = env::var_os("KINDLEGEN") else {
        return tool::find(&["kindlegen", "kindlegen.exe"]).ok_or_else(|| {
            "kindlegen not found; put it on PATH or set KINDLEGEN to its path".into()
        });
    };
    PathBuf::from(path)
        .canonicalize()
        .ok()
        .filter(|path| path.is_file())
        .ok_or_else(|| "KINDLEGEN does not point to a file".into())
}

/// Returns `None` when the invocation was only a request for help or version.
fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        inputs: Vec::new(),
        output: PathBuf::from("."),
        jobs: thread::available_parallelism().map_or(1, usize::from),
        author: "Unknown".into(),
        strip: true,
    };
    let mut raw = env::args_os().skip(1);
    while let Some(arg) = raw.next() {
        let mut value = || {
            raw.next()
                .ok_or_else(|| format!("{} needs a value", arg.display()))
        };
        match arg.to_str() {
            Some("-h" | "--help") => {
                print!("{USAGE}");
                return Ok(None);
            }
            Some("-V" | "--version") => {
                println!("comic2mobi {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            Some("-o" | "--output") => args.output = PathBuf::from(value()?),
            Some("-j" | "--jobs") => {
                args.jobs = value()?
                    .to_str()
                    .and_then(|jobs| jobs.parse().ok())
                    .filter(|&jobs| jobs > 0)
                    .ok_or("--jobs needs a positive number")?;
            }
            Some("-a" | "--author") => {
                args.author = value()?
                    .into_string()
                    .map_err(|_| "--author must be valid UTF-8")?;
            }
            Some("--no-strip") => args.strip = false,
            Some(option) if option.starts_with('-') => {
                return Err(format!("unknown option {option}\n\n{USAGE}"));
            }
            _ => args.inputs.push(PathBuf::from(arg)),
        }
    }
    if args.inputs.is_empty() {
        return Err(format!("no input given\n\n{USAGE}"));
    }
    Ok(Some(args))
}
