# Developing Remise

## How it works

```
src/                      the UI: plain HTML/CSS/JS, no framework, no bundler
src-tauri/src/
  tools/                  the registry, one file per family (pdf, docs, image, video, audio, create, files)
    mod.rs                types (Tool, Step, Job) and shared helpers
  runner.rs               runs a chain of tools: scratch dir, progress, never overwrites
  bins.rs                 finds binaries: downloads and user tools, then vendor/, then PATH and the usual places
  fetch.rs                first-use downloads (models, Chromium, PdfCraft, LibreOffice): pinned, checked, unpacked aside
  cutout.rs               `remise --cutout`: background removal with ONNX Runtime, run as a child process
  tests/                  unit tests, the real-binary smoke test, and the manifest dump
scripts/vendor.py         fetches the bundled binaries, pinned by URL + SHA-256, writes THIRD_PARTY.md
scripts/build-deps.sh     builds what upstream ships no relocatable binary of (run by the deps workflow)
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

Set `REMISE_VENDOR=$PWD/vendor` to make the app or the smoke test use a vendored set, and `REMISE_KEEP=1` to keep the smoke test's outputs for a look. The smoke test also exercises the first-use downloads when `REMISE_USER_TOOLS` points at a scratch folder, and background removal when `REMISE_SELF` points at a built `remise`.

## CI and releases

- **CI** runs lint and unit tests on Linux, Windows and macOS. It then runs the smoke test on Windows, Linux, and both kinds of Mac against the bundled binaries only, followed by the first-use downloads and the tools that use them. Those downloads are cached per OS under a key derived from `fetch.rs`, which holds every pinned URL and SHA-256: a changed pin downloads afresh, otherwise nothing is fetched again.
- **Deps**: `.github/workflows/deps.yml` builds from pinned sources what has no relocatable upstream binary (`scripts/build-deps.sh`), checks that the result links nothing but the system's C/C++ runtime, and, when run by hand, attaches it to the `deps` pre-release. `vendor.py` then pins those files by SHA-256 like any other download; an asset is never replaced, so a pin can't go stale.
- **Releases**: pushing a `v*` tag builds the four installers (Windows x64, macOS Apple Silicon and Intel, Linux AppImage) into a draft GitHub release.

## Bundled programs and licences

| | Windows | macOS | Linux AppImage |
|---|---|---|---|
| FFmpeg, qpdf, 7-Zip, Pandoc, Typst, yt-dlp + QuickJS, waifu2x | upstream builds | upstream builds | upstream builds |
| Ghostscript (with its built-in Tesseract OCR) | upstream build | built from source | built from source |
| ImageMagick | upstream portable build | built from source, with Homebrew's delegate libraries bundled | the official AppImage, unpacked |
| whisper.cpp | upstream build | built from source | upstream build |
| ONNX Runtime | upstream build | upstream build (1.23.2 on Intel, the last one Microsoft made) | upstream build |
| Potrace | upstream build | upstream on Intel, built from source on Apple Silicon | upstream build |
| OCR languages | English, French, German, Spanish, Italian, Portuguese, Dutch (`tessdata_fast`) | same | same |

- **How Remise runs them**: as separate processes, never linked into the app. ONNX Runtime is a library, loaded only by `remise --cutout`, a child process. Every build includes `vendor/THIRD_PARTY.md`, which lists each program's version, licence, binary URL and corresponding source, along with the licence texts. Ghostscript is AGPL-3.0; FFmpeg (in these builds), Potrace and Pandoc are GPL; 7-Zip is LGPL; the Linux ImageMagick carries Ubuntu libraries under their own licences (x265 is GPL); the rest are permissive.
- **On Windows**, whisper.cpp and ONNX Runtime need the MSVC runtime: the copies qpdf and waifu2x ship are placed next to them, and `remise --cutout` loads them by full path, so an old system-wide copy can't win.
- **Remise's own code** is MIT-licensed.
- **The bundled fonts**, Bricolage Grotesque and IBM Plex Mono, are under the SIL OFL 1.1.

**yt-dlp updates itself.** On first use, the bundled copy is copied into the per-user tools folder, which is writable. Each download first tries `yt-dlp -U` there; if the update fails, the download carries on with the version already there.

## Downloaded on first use

Too big for every installer, so the tool's button offers the download, with its size. `fetch.rs` pins each one by URL and SHA-256, downloads with the system's `curl`, unpacks into a staging folder (7-Zip, `tar`, `msiexec /a` or `hdiutil` + `ditto`) and moves it in place only when complete.

| What | For | Size |
|---|---|---|
| BiRefNet general lite (MIT), as converted by rembg | Remove background | 224 MB |
| Whisper small, `q5_1` (MIT) | Transcribe | 190 MB |
| LibreOffice 26.8.2 (MPL-2.0), from The Document Foundation's archive; the LibreItalia AppImage on Linux | Office to PDF | 299–376 MB |
| PdfCraft 0.19.0 (MIT/Apache-2.0), portable builds | PDF Studio | 115–129 MB |
| chrome-headless-shell 157 (BSD-3-Clause and others), from Chrome for Testing; macOS and Linux only | HTML to PDF / image, when no Chromium browser is installed | 105–128 MB |

## Known limits

These are deliberate shortcuts, marked `CBA:` in the code (`grep -rn CBA:`):

- **Builds**: they are unsigned.
- **Batch rename** has no undo; it shows every new name before you press Run.
- **Stamp** uses light tints instead of transparency, because `.setfillconstantalpha` errors out in Ghostscript 10.08.
- **Sign** and **Crop margins** take page 1's size for every page; on a PDF that mixes page sizes, the others are approximate.
- **PDF to Ebook** keeps the text only.
- **Documents** go through Pandoc's sandbox, and only images come back in: those in the file's folder or below it, and `http(s)` images (downloaded, kept only if the server says it's an image). Anything else becomes its alt text. A symlink inside the folder is still followed (Pandoc's Lua has no `lstat`).
- **AI Upscale** (waifu2x) needs a Vulkan graphics driver. Every recent GPU driver has one, and on Linux Mesa provides one even without a GPU (lavapipe). It works out of the box on Macs.
