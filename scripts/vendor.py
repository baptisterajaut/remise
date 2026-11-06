#!/usr/bin/env python3
"""Fetch the third-party CLIs Remise drives into ./vendor for one platform.

    python3 scripts/vendor.py windows-x64 | macos-arm64 | macos-x64

Every download is pinned by URL and SHA-256. The tools stay separate programs (Remise
only spawns them), and vendor/THIRD_PARTY.md lists each one with its licence and the
URL of its corresponding source, which is what the GPL/AGPL/LGPL ones require.

Linux builds don't vendor anything: the .deb/.rpm depend on the distro packages instead.
Needs `7z` on PATH for the Windows installers (preinstalled on GitHub's Windows runners).
"""

import hashlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "vendor"
CACHE = ROOT / "vendor-cache"
GH = "https://github.com"


@dataclass
class Item:
    name: str
    version: str
    license: str
    source: str  # where the corresponding source lives
    url: str
    sha256: str
    # (glob inside the unpacked download, destination dir under vendor/). A leading "?"
    # marks the pick optional (licence files whose name varies between releases).
    picks: list = field(default_factory=list)


GS = dict(
    name="Ghostscript",
    version="10.08.0",
    license="AGPL-3.0-or-later",
    source=f"{GH}/ArtifexSoftware/ghostpdl-downloads/releases/download/gs10080/ghostscript-10.08.0.tar.xz",
)
QPDF = dict(name="qpdf", version="12.4.2", license="Apache-2.0", source=f"{GH}/qpdf/qpdf/archive/refs/tags/v12.4.2.tar.gz")
SEVENZIP = dict(
    name="7-Zip",
    version="26.05",
    license="LGPL-2.1-or-later + BSD-3-Clause + unRAR restriction",
    source=f"{GH}/ip7z/7zip/releases/download/26.05/7z2605-src.tar.xz",
)
FFMPEG = dict(name="FFmpeg", version="9.0.2", license="GPL-3.0-or-later (static build with x264, LAME, libvpx, Opus)")
POTRACE = dict(
    name="Potrace",
    version="1.16",
    license="GPL-2.0-or-later",
    source="https://potrace.sourceforge.net/download/1.16/potrace-1.16.tar.gz",
)
TESSDATA = "https://github.com/tesseract-ocr/tessdata_fast/raw/4.1.0"


def tessdata(lang, sha):
    return Item(
        name=f"Tesseract traineddata ({lang})",
        version="tessdata_fast 4.1.0",
        license="Apache-2.0",
        source=f"{GH}/tesseract-ocr/tessdata_fast",
        url=f"{TESSDATA}/{lang}.traineddata",
        sha256=sha,
        picks=[(f"{lang}.traineddata", "tesseract/tessdata")],
    )


def macos(arch, riedl, ffmpeg_sha, ffprobe_sha, qpdf_sha):
    qarch = "arm64" if arch == "arm64" else "x86_64"
    riedl_url = f"https://ffmpeg.martin-riedl.de/download/macos/{riedl}"
    ff_source = f"https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz (build recipe: {riedl_url.rsplit('/', 1)[0]})"
    return [
        Item(**QPDF, url=f"{GH}/qpdf/qpdf/releases/download/v12.4.2/qpdf-12.4.2-bin-macos-{qarch}.zip", sha256=qpdf_sha,
             picks=[("**/bin/qpdf", "qpdf/bin"), ("**/lib/*.dylib", "qpdf/lib")]),
        Item(**SEVENZIP, url=f"{GH}/ip7z/7zip/releases/download/26.05/7z2605-mac.tar.xz",
             sha256="12ef6830317743cfd3f0ab1724bde43fcc270d8b6d8564f8c12c2a2b7f993570",
             picks=[("7zz", "7zip"), ("?License.txt", "licenses/7zip")]),
        Item(**FFMPEG, source=ff_source, url=f"{riedl_url}/ffmpeg.zip", sha256=ffmpeg_sha, picks=[("ffmpeg", "ffmpeg")]),
        Item(**FFMPEG, source=ff_source, url=f"{riedl_url}/ffprobe.zip", sha256=ffprobe_sha, picks=[("ffprobe", "ffmpeg")]),
    ]


PLATFORMS = {
    "windows-x64": [
        Item(**GS, url=f"{GH}/ArtifexSoftware/ghostpdl-downloads/releases/download/gs10080/gs10080w64.exe",
             sha256="52a91b8bf09298788d7a57b9206127026c23eacd75405f0a131e26dc381dce50",
             picks=[("**/bin/gswin64c.exe", "ghostscript/bin"), ("**/bin/gsdll64.dll", "ghostscript/bin"),
                    ("**/lib/*", "ghostscript/lib"), ("?**/LICENSE", "licenses/ghostscript")]),
        Item(**QPDF, url=f"{GH}/qpdf/qpdf/releases/download/v12.4.2/qpdf-12.4.2-msvc64.zip",
             sha256="db87077e683630c1217e0e8f9a20a9749d952ab676e881c3689187763a5de25d",
             picks=[("**/bin/*", "qpdf/bin"), ("?**/LICENSE.txt", "licenses/qpdf")]),
        Item(name="Tesseract", version="5.5.3", license="Apache-2.0",
             source=f"{GH}/tesseract-ocr/tesseract/archive/refs/tags/5.5.3.tar.gz",
             url=f"{GH}/tesseract-ocr/tesseract/releases/download/5.5.3/tesseract-ocr-w64-setup-5.5.3.20260724.exe",
             sha256="bee9e3434bd94fd65387d9be28cd467a41f61b1275383b55b0f59a1331270ae4",
             picks=[("tesseract.exe", "tesseract"), ("*.dll", "tesseract")]),
        tessdata("eng", "7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2"),
        tessdata("fra", "ced037562e8c80c13122dece28dd477d399af80911a28791a66a63ac1e3445ca"),
        Item(name="ImageMagick", version="7.1.2-35",
             license="ImageMagick License (Apache-2.0 style); statically links LGPL libraries, see the bundled SBOM",
             source=f"{GH}/ImageMagick/ImageMagick/archive/refs/tags/7.1.2-35.tar.gz",
             url=f"{GH}/ImageMagick/ImageMagick/releases/download/7.1.2-35/ImageMagick-7.1.2-35-portable-Q16-HDRI-x64.7z",
             sha256="34db4364da26c88d6b8cc5bbc1fff22b85d00d3b399c82fe85ee4790efacbbf1",
             picks=[("**/magick.exe", "imagemagick"), ("**/*.xml", "imagemagick"),
                    ("?**/LICENSE*", "licenses/imagemagick"), ("?**/*.cdx.json", "licenses/imagemagick")]),
        Item(**SEVENZIP, url=f"{GH}/ip7z/7zip/releases/download/26.05/7z2605-extra.7z",
             sha256="f27a8f103c84785026edcba1f67faf1357225df9d19ddb0b7760d55947141bc7",
             picks=[("x64/7za.exe", "7zip"), ("?**/License.txt", "licenses/7zip")]),
        Item(**FFMPEG, source="https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz (build recipe: https://www.gyan.dev/ffmpeg/builds/)",
             url=f"{GH}/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip",
             sha256="60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba",
             picks=[("**/bin/ffmpeg.exe", "ffmpeg"), ("**/bin/ffprobe.exe", "ffmpeg"), ("?**/LICENSE*", "licenses/ffmpeg")]),
        Item(**POTRACE, url="https://potrace.sourceforge.net/download/1.16/potrace-1.16.win64.zip",
             sha256="63699f9885b5ed053977fd94452a7bdbb77ee62b77bf28a3f17b2f14b9b9c65d",
             picks=[("**/potrace.exe", "potrace"), ("**/COPYING", "licenses/potrace")]),
    ],
    # CBA: Ghostscript, Tesseract and ImageMagick have no relocatable macOS build upstream;
    # bundling them means brew + dylibbundler + inside-out re-signing per arch. Until then the
    # app greys those tools out with an install hint.
    "macos-arm64": macos("arm64", "arm64/1789931890_9.0.2",
                         "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924",
                         "fcbe839537485eaee7a7a8bc5cbc0f90d53617e80943e8a5b2e31cb851197ea6",
                         "62e46987a30ea167cbc530ccb22690aec3d8c812ed09979a941bcee92e504b79"),
    "macos-x64": macos("x64", "amd64/1789931006_9.0.2",
                       "7c6b4125b191cbf773832dc51f424cf2b6bb7da43007d1e066f95909e47cacd4",
                       "2322438ed2f6319a691291b247d09c69dcaa3a982460d1f269a7e1af335cfdfd",
                       "dd3b01f4414d198529bb0f13bba59c2f016a328bb3506695087a96fb80fc1481")
    + [Item(**POTRACE, url="https://potrace.sourceforge.net/download/1.16/potrace-1.16.mac-x86_64.tar.gz",
            sha256="7b144720c1cdf5f7d73e47e9e9a5df03ee49bcc42037498481d3a29855311940",
            picks=[("**/potrace", "potrace"), ("**/COPYING", "licenses/potrace")])],
}


def fetch(item):
    CACHE.mkdir(exist_ok=True)
    dest = CACHE / f"{item.sha256[:16]}-{item.url.rsplit('/', 1)[1]}"
    if not dest.exists():
        print(f"  get {item.url}")
        with urllib.request.urlopen(item.url) as r, open(dest.with_suffix(".part"), "wb") as f:
            shutil.copyfileobj(r, f)
        dest.with_suffix(".part").rename(dest)
    digest = hashlib.sha256(dest.read_bytes()).hexdigest()
    if digest != item.sha256:
        dest.unlink()
        sys.exit(f"checksum mismatch for {item.url}\n  expected {item.sha256}\n  got      {digest}")
    return dest


def unpack(archive, into):
    name = archive.name
    if name.endswith(".zip"):
        with zipfile.ZipFile(archive) as z:
            z.extractall(into)
    elif name.endswith((".tar.gz", ".tgz", ".tar.xz")):
        with tarfile.open(archive) as t:
            t.extractall(into, filter="data")
    elif name.endswith((".7z", ".exe")):
        subprocess.run(["7z", "x", "-y", f"-o{into}", str(archive)], check=True, stdout=subprocess.DEVNULL)
    else:  # a plain file, keep its upstream name
        shutil.copy(archive, into / archive.name.split("-", 1)[1])


def install(item):
    archive = fetch(item)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        unpack(archive, tmp)
        for pattern, dest in item.picks:
            optional = pattern.startswith("?")
            hits = [p for p in tmp.glob(pattern.lstrip("?")) if p.is_file()]
            if not hits and not optional:
                sys.exit(f"{item.name}: nothing matches {pattern} in {archive.name}")
            (OUT / dest).mkdir(parents=True, exist_ok=True)
            for hit in hits:
                target = OUT / dest / hit.name
                shutil.copy2(hit, target)
                target.chmod(0o755)


def notice(items):
    lines = [
        "# Third-party programs bundled with Remise",
        "",
        "Remise runs these as separate programs; it does not link them. Each keeps its own licence.",
        "Licence texts that ship with the upstream archives are under `licenses/`.",
        "",
    ]
    seen = set()
    for i in items:
        key = (i.name.split(" (")[0], i.version)
        if key in seen:
            continue
        seen.add(key)
        lines += [f"## {i.name} {i.version}", "", f"- Licence: {i.license}", f"- Binary: {i.url}",
                  f"- Corresponding source: {i.source}", ""]
    (OUT / "THIRD_PARTY.md").write_text("\n".join(lines))


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in PLATFORMS:
        sys.exit(f"usage: vendor.py {{{'|'.join(PLATFORMS)}}}")
    items = PLATFORMS[sys.argv[1]]
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir()
    for item in items:
        print(f"{item.name} {item.version}")
        install(item)
    notice(items)
    print(f"vendor/ ready for {sys.argv[1]}")


if __name__ == "__main__":
    main()
