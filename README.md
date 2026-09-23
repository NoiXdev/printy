<p align="center">
  <img src="assets/logo/printy-logo.svg" alt="" width="96" height="96">
</p>

<h1 align="center">Printy</h1>

<p align="center">
  Watches folders and prints what lands in them — so nobody has to open a
  scan, hit <em>Print</em>, pick the tray and close it again, forty times a day.
</p>

<p align="center">
  <a href="https://github.com/NoiXdev/printy/actions/workflows/ci.yml"><img src="https://github.com/NoiXdev/printy/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/Windows-10%2B-black" alt="Windows 10 or later">
  <img src="https://img.shields.io/badge/macOS-13%2B-black" alt="macOS 13 or later">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT licence"></a>
  <a href="https://docs.noix.dev/printy/"><img src="https://img.shields.io/badge/docs-docs.noix.dev-informational" alt="Documentation"></a>
</p>

---

A scanner drops PDFs into a folder. An ERP export writes delivery notes. A label
generator produces one file per parcel. Somebody then opens each file, prints
it, and closes it — all day, and they are the bottleneck the moment they step
away from the desk.

Printy sits between the folder and the printer. You describe a folder once —
which printer, how many copies, duplex or not, colour or greyscale, and what
should happen to the file afterwards — and from then on anything that appears
there is printed on its own.

**[Read the documentation →](https://docs.noix.dev/printy/)**

## Install

**Windows** — take the installer from the
[latest release](https://github.com/NoiXdev/printy/releases/latest). The `.exe`
(NSIS) and `.msi` do the same thing; pick whichever your deployment prefers. A
portable `.zip` is published alongside them for machines where nothing may be
installed — unpack it and keep the two files together, the app needs the library
that ships beside it.

**macOS**

```bash
brew tap NoiXdev/tap
brew install --cask printy
```

Or open the `.dmg` from the
[latest release](https://github.com/NoiXdev/printy/releases/latest) and drag
Printy into Applications.

> **macOS prints through the system print service, and one guarantee is weaker
> there.** With *fit to page* turned **off**, an oversized page is cropped
> rather than shrunk, because the system offers no "shrink but never enlarge"
> mode. On Windows, Printy scales the page itself and never crops. If you print
> documents larger than your paper, leave *fit to page* on — or use Windows.

## What it does

- **Watches local folders.** Each folder has its own check interval, its own
  printer and its own settings. A folder can be paused without touching the
  others; one switch pauses everything.
- **Prints PDF, JPG, PNG and TIFF.** Per folder you choose the printer, the
  number of copies, single- or double-sided, colour or greyscale, and whether
  pages are scaled to fill the paper.
- **Waits until a file is finished.** A file still being written is left alone
  until it stops changing, so a half-transferred scan is never printed.
- **Decides what happens afterwards.** Move the file into a `printed`
  subfolder, leave it where it is and remember it, or delete it. Files that
  could not be printed go to `failed` instead, so the two are never confused.
- **Survives a printer being off.** A printer that is unreachable holds the
  queue instead of failing through it — nothing is lost and nothing burns its
  retries. When it comes back, printing resumes on its own. A file that is
  genuinely broken gives up after three attempts and lets the queue continue.
- **Stays out of the way.** It lives in the system tray, can start with the
  computer, and can start minimised. Notifications are all events, failures
  only, or off.
- **Keeps a history.** Every job with its folder, printer, time and outcome,
  filterable down to just the failures, and each one can be printed again.
- **Moves between machines.** Settings and folders export to a single file and
  import on the next machine. A folder whose path or printer does not exist
  there is imported switched off and named, rather than silently failing later.

## What it deliberately does not do

Knowing the edges up front saves a disappointing evaluation:

- **Local paths only.** Network shares and UNC paths are not supported, and
  neither are cloud-sync folders — a file that syncs in pieces looks finished
  more than once.
- **No subfolders.** A watched folder's subdirectories are not scanned. This is
  what keeps `printed` and `failed` from being picked up again.
- **No Office formats.** DOCX and XLSX are not printed; export to PDF first.
- **No tray or paper-size selection.** Printy uses the printer's own defaults.
- **One user, one machine.** It is a desktop application, not a print server.

## Development

Requires [Node.js](https://nodejs.org/) 22, a [Rust toolchain](https://rustup.rs/)
and the Tauri prerequisites for your platform.

```bash
git clone https://github.com/NoiXdev/printy.git
cd printy
npm install
npm run tauri dev
```

Tests — both halves run in CI on every pull request:

```bash
npm test                      # frontend
cd src-tauri && cargo test    # engine
```

PDF rendering on Windows uses a native library that is **not** in the
repository: it is an 8 MB third-party binary, fetched and verified by the build
workflow. Without it the app starts normally and then fails on the first PDF,
which is why the workflow checks it rather than trusting the download. To run a
Windows build locally, put `pdfium.dll` in `src-tauri/resources/`, or point
Printy at an existing copy under *Einstellungen*. macOS needs none of this.

The printing path itself cannot be unit-tested — Win32 handles against a real
spooler cannot be mocked honestly. [`docs/WINDOWS-VERIFICATION.md`](docs/WINDOWS-VERIFICATION.md)
is the checklist to run on a Windows machine before a release.

Interface strings are German; code, comments and documentation are English.

## Licence

MIT — see [LICENSE](LICENSE). A tool from [noix.dev](https://noix.dev).
