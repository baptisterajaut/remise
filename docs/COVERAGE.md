# Coverage of Hangar's tool list

Remise is an independent project, built clean-room using only Hangar's public website as a guide. It is not affiliated with Hangar.

Their tools page (`hangar.tools/tools/`) shows 89 cards: 77 shipped and 12 marked *soon*. **Every one of the 77 shipped tools has a counterpart in Remise.** Four of the *soon* ones are already in Remise: Flatten, Repair, Office to PDF and PDF to Ebook.

To replay the check against their live page:

```sh
REMISE_MANIFEST=$PWD/src-tauri/target/manifest.json cargo test --manifest-path src-tauri/Cargo.toml -- --ignored manifest
python3 scripts/coverage.py src-tauri/target/manifest.json
# hangar.tools: 77 shipped, 12 marked soon
# remise: 95 tools, covers 77/77 shipped
```

The mapping from their tool ids to ours lives in `scripts/coverage.py`. In a few places the two differ in depth:

- **Redact** works on areas given in %, with no text search.
- **Sign** places a signature image; it is not a cryptographic signature.
- **Ebook to PDF** reads EPUB only (through Pandoc and Typst).
- **Fonts** produces a specimen sheet.
- **Remove background and Transcribe** need an optional install (rembg, Whisper). AI Upscale is bundled: waifu2x, which also runs on the CPU.
- **HTML rendering** uses the Chrome, Edge or Chromium already on the machine.

Remise also has tools that their list doesn't: Grayscale PDF, Optimize for web, PDF Studio (via PdfCraft), Auto-crop borders, non-AI Upscale, Extract frames, Waveform image, the Docs family through Pandoc, and Chains.
