# Changelog

## 2.0.0 - 2026-11-14

Rewrite in Rust as a command-line tool for Linux and WSL. The pipeline is the
one from 1.x (images -> OPF/NCX/HTML -> kindlegen -> kindlestrip, one book per
worker); everything around it is new.

### Changed

- Command-line tool instead of a Windows Forms window; no Visual Studio, .NET
  or Windows required.
- kindlegen is an external program found through `KINDLEGEN` or `PATH`, no
  longer a binary embedded in the executable.
- A fixed pool of `--jobs` worker threads replaces one thread-pool item per
  input; results are reported as each book finishes.
- kindlestrip is on by default (`--no-strip` turns it off) and passes a book
  without SRCS sections through unchanged.
- Images are copied or hard-linked into a per-book scratch directory and
  referenced by relative name, so nothing depends on absolute source paths and
  two inputs can no longer overwrite each other's intermediate files.

### Added

- Archive inputs (`zip`, `cbz`, `rar`, `cbr`, `7z`, `cb7`), unpacked with the
  7-Zip command-line program.
- PNG, GIF and BMP pages, detected by content; non-image files are skipped.
- Natural page order (`2.jpg` before `10.jpg`).
- `--author` option.

### Fixed

- Pages taller or wider than the 768x1024 page box overflowed it; every image
  is now scaled to fit and centred.
- The OPF nested `<manifest>` and `<spine>` inside `<metadata>`, used item ids
  starting with a digit, and declared every file as `image/jpeg`.
- Titles and file names containing `&`, `<` or `"` produced invalid XML.
- Output is written under a temporary name and renamed, so a failed run cannot
  leave a truncated book.

### Removed

- The EPUB and TXT input types and the Cancel button, which were never
  implemented.
- Placeholder metadata (`ISBN`, `publisher`, `publication` date).

## 1.1.0 - 2013 (legacy)

ComicToMobi, a C# Windows Forms application (Visual Studio 2010, .NET
Framework 4.0 Client Profile, x86), in a solution named KindleConverter. Its
source is not part of this repository; this entry is the record of its design.

### Design

| File | Role |
| --- | --- |
| `ComicToMobi.cs` | Main window: drag-and-drop input list, output folder, Start, worker callback |
| `MobiProcessor.cs` | Builds the NCX, OPF and per-image HTML with LINQ to XML and runs kindlegen |
| `Kindlestrip.cs` | C# port of kindlestrip.py 1.35 (Paul Durrant, Kevin Hendricks; public domain) |
| `Program.cs` | Entry point and the global input list |

- **Input**: folders dropped onto a list view. `.zip` and `.rar` files were
  accepted by the list but never unpacked.
- **Per book**: one HTML page per image, sized from `System.Drawing`, inside a
  fixed 768x1024 fixed-layout comic package; then `kindlegen.exe -c1`; then,
  if "use kindlestrip" was ticked, removal of the SRCS sections.
- **Parallelism**: one `ThreadPool.QueueUserWorkItem` per input, each with a
  `ManualResetEvent`; the UI thread waited on all of them.
- **kindlegen**: Amazon KindleGen V2.8, embedded as a resource and written
  next to the executable on every run.
- **Planned but not wired in**: SevenZipSharp with `7z.dll` (archive
  extraction) and LibJpeg.NET (image re-encoding) sat in a `res` folder without
  being referenced by the project.

### Known defects

- The window froze during conversion, and workers reported through message
  boxes.
- Archive inputs failed, as described above.
- Images were referenced by absolute source path and all declared as JPEG;
  page order was the file system's.
- The Delete key removed the first rows of the list rather than the selected
  ones.
- With kindlestrip enabled, a book without SRCS sections failed with a null
  reference.
