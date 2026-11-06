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

# Only the C/C++ runtime may come from the system: a Homebrew or distro library would not be
# there on the user's machine.
check_links() {
    if [[ $target == linux-* ]]; then
        ldd "$1"
        ! ldd "$1" | grep -vE 'linux-vdso|ld-linux|lib(c|m|dl|pthread|rt)\.so' | grep -q '=>'
    else
        otool -L "$1"
        ! otool -L "$1" | tail -n +2 | grep -vE '^\s*/(usr/lib|System)/' | grep -q .
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
    local ldflags=""
    [[ $target == linux-* ]] && ldflags="-static-libstdc++ -static-libgcc"
    ./configure LDFLAGS="$ldflags" --without-x --disable-cups --disable-gtk --disable-fontconfig --disable-dbus \
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

ghostscript
# Upstream ships potrace binaries for Linux and Intel Macs only.
[[ $target == macos-arm64 ]] && potrace
ls -l "$out"
