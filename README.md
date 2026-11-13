# comic2mobi

Converts folders or archives of comic page images into Kindle MOBI books,
several books in parallel.

```
comic2mobi -o ~/kindle "Some Comic vol.1/" "Some Comic vol.2.cbz" other.rar
```

Each input becomes `<output>/<name>.mobi`: a fixed-layout comic with one page
per image, in natural file-name order (`page2` before `page10`).

## Requirements

- **kindlegen**, Amazon's (discontinued) MOBI compiler. It is not included.
  Put it on `PATH` or point `KINDLEGEN` at it. Both the Linux binary and the
  Windows `kindlegen.exe` work; under WSL the latter runs through Windows
  interop. A current `kindlegen.exe` ships inside Kindle Previewer 3, typically
  at `%LOCALAPPDATA%\Amazon\Kindle Previewer 3\lib\fc\bin\kindlegen.exe`.
- **7-Zip** (`7zz`, `7z` or `7za` on `PATH`), only for archive inputs:
  `sudo apt install 7zip`. `.rar`/`.cbr` need a 7-Zip build with RAR support.
- A Rust toolchain to build: `cargo build --release`, or
  `cargo install --path .`. There are no crate dependencies.

## Usage

```
Usage: comic2mobi [OPTIONS] <INPUT>...

Options:
  -o, --output <DIR>   Directory for the .mobi files [default: .]
  -j, --jobs <N>       Books converted in parallel [default: CPU count]
  -a, --author <NAME>  Author stored in the books [default: Unknown]
      --no-strip       Keep the source copy kindlegen embeds (larger files)
  -h, --help           Print this help
  -V, --version        Print the version
```

An input is a directory (searched recursively) or a `.zip`, `.cbz`, `.rar`,
`.cbr`, `.7z` or `.cb7` archive. JPEG, PNG, GIF and BMP pages are recognised by
content; every other file is ignored. The exit status is 0 when every book
converted, 1 when some failed and 2 for a usage or setup error.

Under WSL with `kindlegen.exe`, the work directory is read by a Windows
process. It lives under `$TMPDIR` (default `/tmp`); pointing `TMPDIR` at a
Windows drive, e.g. `TMPDIR=/mnt/c/Temp`, speeds up large books.

## How it works

For every book, in its own scratch directory:

1. **Collect** (`source.rs`, `image.rs`): unpack the archive with 7-Zip or walk
   the directory, sort naturally, and read each image's type and pixel size
   from its header.
2. **Describe** (`opf.rs`): write an OPF package, an NCX table of contents and
   one HTML page per image, each image scaled to fit a 768x1024 page.
3. **Compile** (`book.rs`): run `kindlegen`, which produces a combined
   MOBI7 + KF8 file.
4. **Strip** (`kindlestrip.rs`): remove the SRCS sections, kindlegen's embedded
   copy of the sources, which is often most of the file.

Books are independent, so `main.rs` runs up to `--jobs` of these pipelines at
once on a pool of worker threads.

## History

Version 2 is a rewrite of ComicToMobi 1.x, a 2013 Windows Forms application.
[CHANGELOG.md](CHANGELOG.md) records that design and what changed.
