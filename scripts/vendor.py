#!/usr/bin/env python3
"""Fetch the third-party CLIs Remise drives into ./vendor for one platform.

    python3 scripts/vendor.py windows-x64 | macos-arm64 | macos-x64

Every download is pinned by URL and SHA-256. The tools stay separate programs (Remise
only spawns them), and vendor/THIRD_PARTY.md lists each one with its licence and the
URL of its corresponding source, which is what the GPL/AGPL/LGPL ones require.

Where upstream ships no relocatable build (Ghostscript, Tesseract and ImageMagick on macOS
and Linux), the app falls back to an installed copy and greys the tool out otherwise.
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
    # (glob inside the unpacked download, destination dir under vendor/[, new file name]).
    # A leading "?" marks the pick optional (licence files whose name varies between releases).
    picks: list = field(default_factory=list)
    raw: bool = False  # a bare executable, not an archive or installer


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
PANDOC = dict(name="Pandoc", version="3.12.1", license="GPL-2.0-or-later",
              source=f"{GH}/jgm/pandoc/archive/refs/tags/3.12.1.tar.gz")
TYPST = dict(name="Typst", version="0.15.1", license="Apache-2.0", source=f"{GH}/typst/typst/archive/refs/tags/v0.15.1.tar.gz")
YTDLP = dict(name="yt-dlp", version="2026.10.21",
             license="Unlicense (the standalone build bundles Python and its dependencies under their own licences)",
             source=f"{GH}/yt-dlp/yt-dlp/archive/refs/tags/2026.10.21.tar.gz")
QJS = dict(name="QuickJS-NG", version="0.17.0", license="MIT",
           source=f"{GH}/quickjs-ng/quickjs/archive/refs/tags/v0.17.0.tar.gz")
TESSDATA = "https://github.com/tesseract-ocr/tessdata_fast/raw/4.1.0"


WAIFU2X = dict(name="waifu2x-ncnn-vulkan", version="20250915", license="MIT (code and the upconv_7_photo model)",
               source=f"{GH}/nihui/waifu2x-ncnn-vulkan/archive/refs/tags/20250915.tar.gz")


def upscaler(os_name, sha):
    """AI upscale with a CPU fallback (-g -1), unlike Real-ESRGAN-ncnn which needs a Vulkan GPU."""
    exe = ".exe" if os_name == "windows" else ""
    picks = [
        (f"**/waifu2x-ncnn-vulkan{exe}", "waifu2x"),
        ("**/models-upconv_7_photo/scale2.0x_model.*", "waifu2x/models-upconv_7_photo"),
        ("?**/LICENSE", "licenses/waifu2x"),
    ]
    if exe:
        picks.append(("**/vcomp140.dll", "waifu2x"))
    asset = f"waifu2x-ncnn-vulkan-20250915-{os_name}.zip"
    return [Item(**WAIFU2X, url=f"{GH}/nihui/waifu2x-ncnn-vulkan/releases/download/20250915/{asset}", sha256=sha, picks=picks)]


def documents(pandoc, typst, ytdlp, qjs):
    """Pandoc + Typst (documents, Markdown → PDF) and yt-dlp + QuickJS (downloads), per platform.
    Each argument is (asset name, sha256)."""
    rel = lambda repo, tag, asset: f"{GH}/{repo}/releases/download/{tag}/{asset}"
    exe = ".exe" if pandoc[0].endswith(".zip") and "windows" in pandoc[0] else ""
    return [
        Item(**PANDOC, url=rel("jgm/pandoc", "3.12.1", pandoc[0]), sha256=pandoc[1],
             picks=[(f"**/pandoc{exe}", "pandoc")] if exe else [("**/bin/pandoc", "pandoc")]),
        Item(**TYPST, url=rel("typst/typst", "v0.15.1", typst[0]), sha256=typst[1],
             picks=[(f"**/typst{exe}", "typst"), ("?**/LICENSE", "licenses/typst"), ("?**/NOTICE", "licenses/typst")]),
        Item(**YTDLP, url=rel("yt-dlp/yt-dlp", "2026.10.21", ytdlp[0]), sha256=ytdlp[1],
             picks=[(ytdlp[0], "yt-dlp", f"yt-dlp{exe}")], raw=True),
        # yt-dlp needs a JavaScript runtime for YouTube; QuickJS is the 2 MB one it supports.
        Item(**QJS, url=rel("quickjs-ng/quickjs", "v0.17.0", qjs[0]), sha256=qjs[1],
             picks=[(qjs[0], "quickjs", f"qjs{exe}")], raw=True),
    ]


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


def macos(arch, riedl, ffmpeg_sha, qpdf_sha):
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
    ]


PLATFORMS = {
    "windows-x64": [
        Item(**GS, url=f"{GH}/ArtifexSoftware/ghostpdl-downloads/releases/download/gs10080/gs10080w64.exe",
             sha256="52a91b8bf09298788d7a57b9206127026c23eacd75405f0a131e26dc381dce50",
             picks=[("**/bin/gswin64c.exe", "ghostscript/bin"), ("**/bin/gsdll64.dll", "ghostscript/bin"),
                    ("?**/LICENSE", "licenses/ghostscript")]),
        Item(**QPDF, url=f"{GH}/qpdf/qpdf/releases/download/v12.4.2/qpdf-12.4.2-msvc64.zip",
             sha256="db87077e683630c1217e0e8f9a20a9749d952ab676e881c3689187763a5de25d",
             picks=[("**/bin/*", "qpdf/bin"), ("?**/LICENSE.txt", "licenses/qpdf")]),
        Item(name="Tesseract", version="5.5.3", license="Apache-2.0",
             source=f"{GH}/tesseract-ocr/tesseract/archive/refs/tags/5.5.3.tar.gz",
             url=f"{GH}/tesseract-ocr/tesseract/releases/download/5.5.3/tesseract-ocr-w64-setup-5.5.3.20260724.exe",
             sha256="bee9e3434bd94fd65387d9be28cd467a41f61b1275383b55b0f59a1331270ae4",
             # configs/pdf, configs/txt and pdf.ttf: the PDF/text renderers silently write nothing without them.
             picks=[("tesseract.exe", "tesseract"), ("*.dll", "tesseract"),
                    ("tessdata/configs/*", "tesseract/tessdata/configs"), ("tessdata/pdf.ttf", "tesseract/tessdata")]),
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
             picks=[("**/bin/ffmpeg.exe", "ffmpeg"), ("?**/LICENSE*", "licenses/ffmpeg")]),
        Item(**POTRACE, url="https://potrace.sourceforge.net/download/1.16/potrace-1.16.win64.zip",
             sha256="63699f9885b5ed053977fd94452a7bdbb77ee62b77bf28a3f17b2f14b9b9c65d",
             picks=[("**/potrace.exe", "potrace"), ("**/COPYING", "licenses/potrace")]),
    ] + documents(
        ("pandoc-3.12.1-windows-x86_64.zip", "6ef431cb20b2c24a0371fc19200dc49d452be54b1b5c9239f3462841ef53e587"),
        ("typst-x86_64-pc-windows-msvc.zip", "19ce3551153c2fe7ee9fa2f95208310c8f4d3209fedb699e0333faf8913f6736"),
        ("yt-dlp.exe", "33121d192d706e62f684a06f80918e3f3a8ba52ac3a65213b6951c39c7d9f555"),
        ("qjs-windows-x86_64.exe", "2aeabf0092c3262d6b2609824418f7dd7ed1f1df939f73b2b15645230cac0d77"),
    ) + upscaler("windows", "7425be94b94e4c8f37a1e433ac0e0100c43790e2c37418f4b65d8235adfbdc87"),
    # CBA: Ghostscript, Tesseract and ImageMagick have no relocatable macOS build upstream;
    # bundling them means brew + dylibbundler + inside-out re-signing per arch. Until then the
    # app greys those tools out with an install hint.
    "macos-arm64": macos("arm64", "arm64/1789931890_9.0.2",
                         "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924",
                         "62e46987a30ea167cbc530ccb22690aec3d8c812ed09979a941bcee92e504b79")
    + documents(
        ("pandoc-3.12.1-arm64-macOS.zip", "c2146cf33a583f3ac93eccddf1bb662371d5cfcce1aad1c87ad860cca1435d46"),
        ("typst-aarch64-apple-darwin.tar.xz", "48f62ed034aa3a7978309579ac6ca00045e2ef0da73114e8af27cfd8e74dc05a"),
        ("yt-dlp_macos", "7a6fe71d7d680f1fb0ac18d4949873d77db2859e90017c0f44bd94b23be4ee40"),
        ("qjs-darwin-arm64", "8be3ddfe3397d2e692e4e1e8972ee9d032a0a580505d2f8b4ea528cf1b651c11"),
    ) + upscaler("macos", "a5b58b239eb3aa030db5464f6759637fe412d8c07891a4346ea57f708b514d42"),
    "macos-x64": macos("x64", "amd64/1789931006_9.0.2",
                       "7c6b4125b191cbf773832dc51f424cf2b6bb7da43007d1e066f95909e47cacd4",
                       "dd3b01f4414d198529bb0f13bba59c2f016a328bb3506695087a96fb80fc1481")
    + [Item(**POTRACE, url="https://potrace.sourceforge.net/download/1.16/potrace-1.16.mac-x86_64.tar.gz",
            sha256="7b144720c1cdf5f7d73e47e9e9a5df03ee49bcc42037498481d3a29855311940",
            picks=[("**/potrace", "potrace"), ("**/COPYING", "licenses/potrace")])]
    + documents(
        ("pandoc-3.12.1-x86_64-macOS.zip", "81ed3e2cfecf096df15d407152f89a684fd65221082b919963f738e977fad3bc"),
        ("typst-x86_64-apple-darwin.tar.xz", "7f9fdd9584866245de9a79e0add8f9236fae6f40a8a45e2c4771ccc14db4e0fa"),
        ("yt-dlp_macos", "7a6fe71d7d680f1fb0ac18d4949873d77db2859e90017c0f44bd94b23be4ee40"),
        ("qjs-darwin-x86_64", "9e5e101b4fd13cda3204222ca9f8be35412c41dcdef3745829633b7a67245412"),
    ) + upscaler("macos", "a5b58b239eb3aa030db5464f6759637fe412d8c07891a4346ea57f708b514d42"),
    # AppImage. BtbN keeps month-end autobuilds, so this tag doesn't vanish like the dailies.
    "linux-x64": [
        Item(**QPDF, url=f"{GH}/qpdf/qpdf/releases/download/v12.4.2/qpdf-12.4.2-bin-linux-x86_64.zip",
             sha256="db367d897829f22c4198ce1094143c9d467bd6ee7dfabc44ba6f02056b24f8b1",
             picks=[("**/bin/qpdf", "qpdf/bin"), ("**/lib/*.so*", "qpdf/lib")]),
        Item(**SEVENZIP, url=f"{GH}/ip7z/7zip/releases/download/26.05/7z2605-linux-x64.tar.xz",
             sha256="22cf82fcc6eb7474f1d5bb4f02c0f8ebd573a718089d6833dc30f8350a3b232e",
             picks=[("7zzs", "7zip"), ("?License.txt", "licenses/7zip")]),
        Item(**FFMPEG, source=f"https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz (build recipe: {GH}/BtbN/FFmpeg-Builds)",
             url=f"{GH}/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-09-30-13-08/ffmpeg-n9.0.2-17-g2a571b6068-linux64-gpl-9.0.tar.xz",
             sha256="68ee646831adaae2495618346f3bba94ff207ff83bbd34d643e7004730d66269",
             picks=[("**/bin/ffmpeg", "ffmpeg"), ("?**/LICENSE.txt", "licenses/ffmpeg")]),
        Item(**POTRACE, url="https://potrace.sourceforge.net/download/1.16/potrace-1.16.linux-x86_64.tar.gz",
             sha256="158ef1cb6bde9f057510c7aa8b555117be10fa5318cd875f6a26a5811dadc2d8",
             picks=[("**/potrace", "potrace"), ("**/COPYING", "licenses/potrace")]),
    ] + documents(
        ("pandoc-3.12.1-linux-amd64.tar.gz", "d0c90410e90204c9ca83b8539fac5c7aed01fd537207e4585849f8abc5df20b8"),
        ("typst-x86_64-unknown-linux-musl.tar.xz", "a6d077d0a95eed5a2eba715b2dae06be954f624ccbf85758a03f389ded33118c"),
        ("yt-dlp_linux", "d8bd33fca9d5b23bd5b9952b3fca36db5c46331933fda9f598a3da14ce2c243f"),
        ("qjs-linux-x86_64", "0bfc02511a9f549c28b53880d988fc7cd5d361e90c5e8afdfcd7dc6774ceace5"),
    ) + upscaler("linux", "848e0fba55657d34da90b775b8139e9806dc754798b029f95e106ba8850a731f"),
}


def fetch(item):
    CACHE.mkdir(exist_ok=True)
    dest = CACHE / f"{item.sha256[:16]}-{item.url.rsplit('/', 1)[1]}"
    if not dest.exists():
        print(f"  get {item.url}")
        req = urllib.request.Request(item.url, headers={"User-Agent": "remise-vendor/1.0"})
        with urllib.request.urlopen(req) as r, open(dest.with_suffix(".part"), "wb") as f:
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
            # zipfile writes symlinks as text files holding the target; make them real copies
            # (libqpdf.so.30 -> libqpdf.so.30.4.2), Tauri resources don't keep links anyway.
            for info in z.infolist():
                if (info.external_attr >> 16) & 0o170000 == 0o120000:
                    link = into / info.filename
                    target = link.parent / link.read_text()
                    link.unlink()
                    shutil.copy2(target, link)
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
        if item.raw:
            shutil.copy(archive, tmp / archive.name.split("-", 1)[1])
        else:
            unpack(archive, tmp)
        for pattern, dest, *rename in item.picks:
            optional = pattern.startswith("?")
            hits = [p for p in tmp.glob(pattern.lstrip("?")) if p.is_file()]
            if not hits and not optional:
                sys.exit(f"{item.name}: nothing matches {pattern} in {archive.name}")
            (OUT / dest).mkdir(parents=True, exist_ok=True)
            for hit in hits:
                target = OUT / dest / (rename[0] if rename else hit.name)
                shutil.copy2(hit, target)
                target.chmod(0o755)


def notice(items):
    lines = [
        "# Third-party programs bundled with Remise",
        "",
        "Remise runs these as separate programs; it does not link them. Each keeps its own licence.",
        "Licence texts shipped with the upstream archives are under `licenses/<tool>/`, standard texts under `licenses/texts/`.",
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
    # Full texts of the licences named above, for archives that don't ship their own.
    shutil.copytree(ROOT / "scripts" / "licenses", OUT / "licenses" / "texts", dirs_exist_ok=True)


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
