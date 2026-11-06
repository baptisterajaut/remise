# Developing Remise

## How it works

```
src/                      the UI: plain HTML/CSS/JS, no framework, no bundler
src-tauri/src/
  tools/                  the registry, one file per family (pdf, docs, image, video, audio, create, files)
    mod.rs                types (Tool, Step, Job) and shared helpers
  runner.rs               runs a chain of tools: scratch dir, progress, never overwrites
  bins.rs                 finds binaries: user tools, then vendor/, then PATH and the usual places
  tests/                  unit tests, the real-binary smoke test, and the manifest dump
scripts/vendor.py         fetches the bundled binaries, pinned by URL + SHA-256, writes THIRD_PARTY.md
scripts/coverage.py       replays the Hangar coverage check (see COVERAGE.md)
tests/ui.mjs              headless walk-through of the UI with a mocked Tauri bridge
```

A tool is data: what it accepts, its parameters, and a pure function that turns a job (inputs, parameters, output path) into steps. A step is a command to run, a file to write, or a `Then` that decides the next steps from what earlier ones produced (a page size, a list of pages). Adding a tool means adding one `Tool { … }` entry to the right family file. The UI renders its parameters on its own, and the chain builder offers it wherever the previous step's output fits.

Commands are always spawned with an argument vector and never through a shell, because file names are user input. For the same reason, the UI builds every node with `textContent` and never uses `innerHTML`.

## Commands

```sh
pnpm install
pnpm tauri dev          # run the app
pnpm lint               # eslint (complexity ≤ 10) + clippy -D warnings (cognitive complexity ≤ 15)
pnpm test               # unit tests
pnpm smoke              # every tool against the real binaries on generated fixtures
pnpm test:ui            # headless walk-through of the UI (needs a Chromium)
python3 scripts/vendor.py linux-x64   # bundled binaries into vendor/ (also windows-x64, macos-arm64, macos-x64)
```

Set `REMISE_VENDOR=$PWD/vendor` to make the app or the smoke test use a vendored set, and `REMISE_KEEP=1` to keep the smoke test's outputs for a look.

## CI and releases

- **CI** runs lint and unit tests on Linux, Windows and macOS. It then runs the smoke test on each OS against the bundled binaries, plus the system packages where nothing relocatable exists upstream.
- **Releases**: pushing a `v*` tag builds the four installers (Windows x64, macOS Apple Silicon and Intel, Linux AppImage) into a GitHub release.

## Bundled programs and licences

| | Windows | macOS | Linux AppImage |
|---|---|---|---|
| FFmpeg, qpdf, 7-Zip, Pandoc, Typst, yt-dlp + QuickJS, waifu2x | bundled | bundled | bundled |
| Potrace | bundled | bundled on Intel | bundled |
| Ghostscript, Tesseract, ImageMagick | bundled | Homebrew | distro packages |

- **How Remise runs them**: as separate processes, never linked into the app. Every build includes `vendor/THIRD_PARTY.md`, which lists each program's version, licence, binary URL and corresponding source, along with the licence texts. Ghostscript is AGPL-3.0; FFmpeg (in these builds), Potrace and Pandoc are GPL; 7-Zip is LGPL; the rest are permissive.
- **Remise's own code** is MIT-licensed.
- **The bundled fonts**, Bricolage Grotesque and IBM Plex Mono, are under the SIL OFL 1.1.

**yt-dlp updates itself.** On first use, the bundled copy is copied into the per-user tools folder, which is writable. Each download first tries `yt-dlp -U` there; if the update fails, the download carries on with the version already there.

## Roadmap: fewer manual installs

An adversarial review measured these options; sizes and licences were checked against the release pages.

- **Ghostscript with built-in OCR on macOS and Linux.** Build the official 10.08.0 tarball in CI; it carries Tesseract and Leptonica, so `-sDEVICE=pdfocr24` replaces the separate Tesseract for PDF OCR. AGPL, already handled on Windows.
- **ImageMagick 7 on Linux.** Unpack the official AppImage (33 MB) with 7z, with no FUSE needed, and ship the same hardened `policy.xml`. This also fixes PDF writing on distros that still have IM 6.
- **Headless Chromium on first use.** `chrome-headless-shell` from Chrome for Testing (105–128 MB) for HTML → PDF/image on Macs and Linux without a browser; Windows already has Edge.
- **Transcription.** whisper.cpp (MIT, ~10 MB) plus `ggml-tiny-q5_1` (32 MB) on first use. Its macOS binaries need building in CI. Whistle/cactus is smaller, but its native source isn't published.
- **Background removal.** The `ort` crate plus ONNX Runtime and `u2netp` (4.6 MB, Apache-2.0) on first use, instead of the Python rembg.
- **Not worth bundling.** LibreOffice (220–375 MB); Pandoc already covers the text side of Office documents.

## Known limits

These are deliberate shortcuts, marked `CBA:` in the code (`grep -rn CBA:`):

- **Builds**: they are unsigned.
- **macOS**: Ghostscript, Tesseract and ImageMagick are not bundled, because their projects ship no relocatable macOS build.
- **Linux IM6**: on Ubuntu 22.04/24.04 and Debian 12, the distro's ImageMagick 6 refuses to write PDF by default (`policy.xml`), which breaks *Images to PDF*.
- **Join videos** copies streams without re-encoding, so the clips need matching codecs.
- **Insert clip** needs both videos to have an audio track.
- **Batch rename** has no live preview and no undo.
- **Running jobs** can't be cancelled yet.
- **Change file dates** reads the date as UTC.
- **Stamp** uses light tints instead of transparency, because `.setfillconstantalpha` errors out in Ghostscript 10.08.
- **Sign** expects A4 pages, and the signature drifts a little on other page sizes.
- **PDF to Ebook** keeps the text only.
- **AI Upscale** (waifu2x) needs a Vulkan graphics driver. Every recent GPU driver has one, and on Linux Mesa provides one even without a GPU (lavapipe). It works out of the box on Macs.
