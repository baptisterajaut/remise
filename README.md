# remise.

Local file tools for PDF, image, video, audio and archives, in one desktop app. Drop files and pick a tool, or line tools up into a chain. Nothing is uploaded, and your originals are never overwritten.

![The wall](docs/light-wall.png)

Remise is a thin, honest wrapper around the classics: Ghostscript, qpdf, Tesseract, FFmpeg, ImageMagick, 7-Zip and Potrace. Each tool is a few lines of data describing which binary to call with which arguments. The work is done by programs that have been doing it well for decades.

## What's on the wall

| Family | Tools |
|---|---|
| **PDF** (14) | Compress, Merge, Split, Extract pages, Rotate pages, OCR (PDFs and photos → searchable PDF), PDF to images, Images to PDF, Extract text, Grayscale, Encrypt (AES-256), Unlock, Optimize for web, **PDF Studio** (opens the file in [PdfCraft](https://github.com/storytold/pdfcraft) to annotate, sign, redact and reorder) |
| **Image** (14) | Convert (WebP/AVIF/PNG/JPG/TIFF/GIF), Compress, Resize, Rotate & flip, Auto-crop borders, Strip metadata, Remove background (local AI via rembg), Vectorize to SVG, Stylize, Watermark, Upscale, Icon pack (.ico), Color palette, Contact sheet |
| **Video** (11) | Compress (H.264), Convert, Video to GIF (palette-optimised), Trim (frame accurate), Extract audio, Mute, Change speed, Resize, Rotate, Join, Extract frames |
| **Audio** (5) | Convert, Compress, Volume & normalize (−16 LUFS), Speed, Waveform image |
| **Files** (3) | Create archive (zip/7z), Extract archive (zip, 7z, rar, tar, iso…), Batch rename |

**Chains** run several tools one after another, for example *Compress PDF → Grayscale → Optimize for web*. Each step's output becomes the next step's input. Intermediate files stay in a temp folder, and only the final result appears next to your file, as `contract.compressed.gray.web.pdf`.

**Batch** works with any tool: drop a folder's worth of files and each one is processed. Tools such as Merge, Join or Contact sheet instead combine everything you dropped into a single output.

**Dropping files on the wall** filters it down to the tools that can take them. If exactly one tool fits, it opens straight away.

![A tool on the bench](docs/light-bench.png)

## Download

Releases come in four builds: Windows x64 (NSIS installer), macOS Apple Silicon and Intel (.dmg), and Linux x64 (AppImage). Here is what each build bundles:

| | Windows | macOS | Linux AppImage |
|---|---|---|---|
| FFmpeg, qpdf, 7-Zip | bundled | bundled | bundled |
| Potrace | bundled | bundled (Intel), install on Apple Silicon | bundled |
| Ghostscript, Tesseract, ImageMagick | bundled | `brew install ghostscript tesseract imagemagick` | from your distro |
| rembg, PdfCraft (optional) | install separately | install separately | install separately |

The app looks for bundled binaries first, then for ones you have installed. A tool whose binary is missing shows up hatched on the wall, with the install command in its tooltip.

The builds are not code-signed yet. On macOS, right-click the app and choose **Open** the first time. If a bundled tool is still blocked, run `xattr -dr com.apple.quarantine /Applications/Remise.app`. On Windows, choose **More info → Run anyway** in SmartScreen.

## How it works

```
src/                    the UI: plain HTML/CSS/JS, no framework, no bundler
src-tauri/src/
  tools.rs              the registry: one entry per tool (data + a pure argv builder)
  runner.rs             runs a chain of tools; scratch dir, progress, never overwrites
  bins.rs               finds binaries: vendor/ first, then PATH and the usual places
  tests/                unit tests, the real-binary smoke test, and the manifest dump
scripts/vendor.py       fetches the bundled binaries, pinned by URL + SHA-256
```

Adding a tool means adding one `Tool { … }` entry to `tools.rs`. The UI renders its parameters on its own, and the chain builder picks it up automatically.

Commands are always spawned with an argument vector and never through a shell, because file names are user input. For the same reason, the UI builds every node with `textContent` and never uses `innerHTML`.

## Development

```sh
pnpm install
pnpm tauri dev          # run the app
pnpm lint               # eslint (complexity ≤ 10) + clippy -D warnings (cognitive complexity ≤ 15)
pnpm test               # unit tests
pnpm smoke              # every tool against the real binaries on generated fixtures
pnpm test:ui            # headless walk-through of the UI (needs a Chromium)
python3 scripts/vendor.py linux-x64   # fetch bundled binaries into vendor/ (windows-x64, macos-arm64, macos-x64)
```

CI runs lint and unit tests on all three OSes, then the smoke test on each one against the bundled binaries. Pushing a `v*` tag builds all four installers into a draft release.

## Licences

Remise is MIT-licensed. The bundled programs keep their own licences: Ghostscript is AGPL-3.0, FFmpeg (in these builds) and Potrace are GPL, 7-Zip is LGPL, and the rest are Apache-style. Remise only runs them as separate processes and never links them. Every build includes `vendor/THIRD_PARTY.md`, which lists each program's version, licence, binary URL and corresponding source, along with the licence texts. The bundled fonts, Bricolage Grotesque and IBM Plex Mono, are under the SIL OFL 1.1.

## Known limits

These are deliberate shortcuts, marked `CBA:` in the code (`grep -rn CBA:`):

- Builds are unsigned (see above).
- macOS does not bundle Ghostscript, Tesseract or ImageMagick, because upstream ships no relocatable macOS build.
- On Ubuntu 22.04/24.04 and Debian 12, the distro's ImageMagick 6 refuses to write PDF by default (`policy.xml`), which breaks *Images to PDF* there.
- *Join videos* copies streams without re-encoding, so the clips need matching codecs (same camera or same export settings).
- *Batch rename* has no live preview and no undo, and a running job can't be cancelled yet.
- *Upscale* uses Lanczos plus sharpening, not AI. There is no transcription yet.

Remise is an independent clean-room project. It is not affiliated with any commercial product.
