#!/usr/bin/env bash
# Builds, from pinned upstream sources, the programs that have no relocatable binary upstream
# for a platform, into deps-out/<name>-<version>-<target>.tar.xz. The deps workflow publishes
# them on the "deps" release, and vendor.py pins each by SHA-256 like any other download.
#
#   scripts/build-deps.sh linux-x64 | macos-arm64 | macos-x64
set -euo pipefail

target=$1
root=$PWD
out=$root/deps-out
work=$(mktemp -d)
mkdir -p "$out"
jobs=$(getconf _NPROCESSORS_ONLN)
export MACOSX_DEPLOYMENT_TARGET=11.0

fetch() { # url sha256 -> unpacked into $work
    local file
    file=$work/$(basename "$1")
    curl -fsSL -o "$file" "$1"
    echo "$2  $file" | shasum -a 256 -c -
    tar xf "$file" -C "$work"
}

# Only the C/C++ runtime may come from the system (libstdc++ and libgcc_s ship with every desktop
# Linux): a Homebrew or distro library would not be there on the user's machine.
check_links() {
    if [[ $target == linux-* ]]; then
        ldd "$1"
        ! ldd "$1" | grep -vE 'linux-vdso|ld-linux|lib(c|m|dl|pthread|rt|stdc\+\+|gcc_s)\.so' | grep -q '=>'
    else
        otool -L "$1"
        ! otool -L "$1" | tail -n +2 | grep -vE '^\s*(/usr/lib/|/System/|@executable_path/|@loader_path/)' | grep -q .
    fi
}

pack() { # name version dir
    tar -cJf "$out/$1-$2-$target.tar.xz" -C "$3" .
    shasum -a 256 "$out/$1-$2-$target.tar.xz"
}

# Ghostscript with its bundled Tesseract and Leptonica (the pdfocr/ocr devices), its init files
# and base fonts compiled in: one self-contained binary.
ghostscript() {
    fetch https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/gs10080/ghostscript-10.08.0.tar.xz \
        c20492bc8ebb96c87fa2e52a0926e1cda8cde95d66145e018ac713fed5da38cf
    cd "$work/ghostscript-10.08.0"
    ./configure --without-x --disable-cups --disable-gtk --disable-fontconfig --disable-dbus \
        --without-libidn --without-libpaper --without-pdftoraster --without-ijs >/dev/null
    make -j"$jobs" >/dev/null
    strip bin/gs
    ./bin/gs -h | grep -q pdfocr24
    check_links bin/gs
    local dest=$work/gs-out
    mkdir -p "$dest/bin"
    cp bin/gs "$dest/bin/"
    cp LICENSE "$dest/"
    pack ghostscript 10.08.0 "$dest"
    cd "$root"
}

potrace() {
    fetch https://potrace.sourceforge.net/download/1.16/potrace-1.16.tar.gz \
        be8248a17dedd6ccbaab2fcc45835bb0502d062e40fbded3bc56028ce5eb7acc
    cd "$work/potrace-1.16"
    ./configure >/dev/null
    make -j"$jobs" >/dev/null
    strip src/potrace
    check_links src/potrace
    local dest=$work/potrace-out
    mkdir -p "$dest"
    cp src/potrace COPYING "$dest/"
    pack potrace 1.16 "$dest"
    cd "$root"
}

# dylibbundler adds the same LC_RPATH twice to libraries that already used @rpath (libheif), and
# dyld refuses to load a file with a duplicate one.
dedupe_rpaths() {
    local f dup
    for f in "$@"; do
        while dup=$(otool -l "$f" | awk '/LC_RPATH/ { getline; getline; print $2 }' | sort | uniq -d | head -1) && [[ -n $dup ]]; do
            install_name_tool -delete_rpath "$dup" "$f"
        done
    done
}

# ImageMagick without modules (every coder built in) against Homebrew's delegate libraries, which
# dylibbundler then copies next to it and rewrites to @executable_path/lib. Linux uses the
# official AppImage instead, Windows the official portable build.
imagemagick() {
    brew install --quiet pkg-config dylibbundler libpng jpeg-turbo libtiff webp libheif freetype little-cms2 \
        openjpeg zstd xz >/dev/null
    fetch https://github.com/ImageMagick/ImageMagick/releases/download/7.1.2-35/ImageMagick-7.1.2-35.tar.xz \
        9554556c8cbf7d347cbd6bedae931ccf323a2bd5e0f08d4834856568ee19f080
    cd "$work/ImageMagick-7.1.2-35"
    local prefix=$work/im-prefix dest=$work/im-out
    ./configure --prefix="$prefix" --disable-shared --enable-static --disable-installed --disable-openmp \
        --disable-docs --without-x --without-perl --without-magick-plus-plus --without-fontconfig --without-djvu \
        --without-openexr --without-raw --without-jxl --without-lqr >/dev/null
    make -j"$jobs" >/dev/null
    make install >/dev/null
    mkdir -p "$dest/config"
    cp "$prefix/bin/magick" "$dest/"
    cp "$prefix"/etc/ImageMagick-7/*.xml "$prefix"/share/ImageMagick-7/*.xml "$prefix"/lib/ImageMagick-7.1.2/config-Q16HDRI/*.xml \
        "$dest/config/"
    cp LICENSE NOTICE "$dest/"
    dylibbundler -od -b -cd -x "$dest/magick" -d "$dest/lib" -p @executable_path/lib/ >/dev/null
    dedupe_rpaths "$dest/magick" "$dest"/lib/*.dylib
    # install_name_tool voids the signatures; Apple Silicon refuses to run unsigned code.
    codesign -f -s - "$dest/magick" "$dest"/lib/*.dylib
    "$dest/magick" -version
    "$dest/magick" -version | grep -q heic
    "$dest/magick" -size 64x64 gradient:red-blue "$work/t.webp"
    "$dest/magick" "$work/t.webp" "$work/t.pdf"
    check_links "$dest/magick"
    for lib in "$dest"/lib/*.dylib; do check_links "$lib" >/dev/null; done
    pack imagemagick 7.1.2-35 "$dest"
    cd "$root"
}

ghostscript
if [[ $target == macos-* ]]; then
    imagemagick
fi
# Upstream ships potrace binaries for Linux and Intel Macs only.
if [[ $target == macos-arm64 ]]; then
    potrace
fi
ls -l "$out"
