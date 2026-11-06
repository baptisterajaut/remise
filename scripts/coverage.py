#!/usr/bin/env python3
"""Replayable check: does Remise cover every tool hangar.tools lists as shipped?

    REMISE_MANIFEST=$PWD/src-tauri/target/manifest.json \
      cargo test --manifest-path src-tauri/Cargo.toml -- --ignored manifest
    python3 scripts/coverage.py src-tauri/target/manifest.json

Reads the public tools page (https://hangar.tools/tools/), keeps the cards not marked
"soon", and maps each of their tool ids to the Remise tool that does the job.
"""

import json
import re
import sys
import urllib.request

# Their public id → our tool id.
MAP = {
    "archive.create": "arc-create", "archive.extract": "arc-extract",
    "audio.audiogram": "aud-audiogram", "audio.compress": "aud-compress", "audio.convert": "aud-convert",
    "audio.extract": "vid-extract-audio", "audio.merge": "aud-merge", "audio.speed": "aud-speed",
    "audio.split": "aud-split", "audio.transcribe": "vid-transcribe", "audio.trim": "aud-trim",
    "audio.volume": "aud-volume",
    "create.carousel": "create-carousel", "create.device_mockup": "create-mockup", "create.gradient": "create-gradient",
    "create.html_shot": "create-html-image", "create.qr": "create-qr", "create.shader": "create-shader",
    "create.type_specimen": "create-font",
    "ebook.to_pdf": "pdf-ebook", "html.to_pdf": "pdf-html",
    "fs.dates": "file-dates", "fs.rename": "batch-rename", "fs.sort": "file-sort",
    "image.beautify": "create-beautify", "image.compare": "img-compare", "image.compress": "img-compress",
    "image.contact_sheet": "img-contact-sheet", "image.convert": "img-convert", "image.crop": "img-crop",
    "image.extract_text": "img-text", "image.icon_pack": "img-icon", "image.ocr": "pdf-ocr",
    "image.palette": "img-palette", "image.redact": "img-redact", "image.remove_bg": "img-remove-bg",
    "image.resize": "img-resize", "image.rotate": "img-rotate", "image.seo": "img-seo",
    "image.strip_exif": "img-strip", "image.stylize": "img-stylize", "image.to_pdf": "images-to-pdf",
    "image.upscale": "img-ai-upscale", "image.vectorize": "img-vectorize", "image.watermark": "img-watermark",
    "pdf.append": "pdf-append", "pdf.compress": "pdf-compress", "pdf.crop": "pdf-crop", "pdf.encrypt": "pdf-encrypt",
    "pdf.extract": "pdf-extract", "pdf.merge": "pdf-merge", "pdf.ocr": "pdf-ocr", "pdf.organize": "pdf-organize",
    "pdf.page_numbers": "pdf-page-numbers", "pdf.redact": "pdf-redact", "pdf.sign": "pdf-sign",
    "pdf.split": "pdf-split", "pdf.stamp": "pdf-stamp", "pdf.to_images": "pdf-to-images",
    "pdf.to_markdown": "pdf-markdown", "pdf.unlock": "pdf-unlock",
    "pdf.flatten": "pdf-flatten", "pdf.repair": "pdf-repair", "pdf.office": "pdf-office",
    "pdf.to_ebook": "doc-pdf-ebook",
    "video.add_audio": "vid-add-audio", "video.caption": "vid-caption", "video.compress": "vid-compress",
    "video.convert": "vid-convert", "video.crop": "vid-crop", "video.download": "vid-download",
    "video.insert_clips": "vid-insert", "video.join": "vid-join", "video.mute": "vid-mute",
    "video.resize": "vid-resize", "video.rotate": "vid-rotate", "video.speed": "vid-speed",
    "video.split": "vid-split", "video.to_gif": "vid-gif", "video.trim": "vid-trim", "video.watermark": "vid-watermark",
}


def their_tools():
    req = urllib.request.Request("https://hangar.tools/tools/", headers={"User-Agent": "remise-coverage/1.0"})
    page = urllib.request.urlopen(req, timeout=30).read().decode("utf-8", "replace")
    cards = re.findall(r'<article class="([^"]*)"[^>]*data-tool="([^"]+)"', page)
    return {tid: "soon" in cls.split() for cls, tid in cards}


def main():
    ours = {t["id"] for t in json.load(open(sys.argv[1]))}
    tools = their_tools()
    shipped = sorted(t for t, soon in tools.items() if not soon)
    soon = sorted(t for t, soon in tools.items() if soon)
    missing = [t for t in shipped if MAP.get(t) not in ours]
    ahead = [t for t in soon if MAP.get(t) in ours]
    print(f"hangar.tools: {len(shipped)} shipped, {len(soon)} marked soon")
    print(f"remise: {len(ours)} tools, covers {len(shipped) - len(missing)}/{len(shipped)} shipped")
    print(f"already here though marked soon there: {', '.join(ahead) or 'none'}")
    if missing:
        print("missing:", ", ".join(missing))
        sys.exit(1)


if __name__ == "__main__":
    main()
