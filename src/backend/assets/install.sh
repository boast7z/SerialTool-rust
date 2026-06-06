#!/bin/bash
# CH341 no_dtr_on_open driver installer
# Usage: install.sh <source_dir>
# source_dir: directory containing patched ch341.c, ch341.h, Makefile
# Must be run as root (via pkexec).
set -euo pipefail

SOURCE_DIR="$1"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

# Copy bundled source to build dir
mkdir -p "$TMPDIR/build"
cp "$SOURCE_DIR/ch341.c" "$SOURCE_DIR/ch341.h" "$SOURCE_DIR/Makefile" "$TMPDIR/build/"

cd "$TMPDIR/build"

# Install via DKMS (survives kernel updates) or fall back to direct install
if command -v dkms &>/dev/null; then
    PKG_SRC="/usr/src/ch341-nodtr-1.0"
    rm -rf "$PKG_SRC"
    mkdir -p "$PKG_SRC"
    cp ch341.c ch341.h Makefile "$PKG_SRC/"

    cat > "$PKG_SRC/dkms.conf" << 'DKMS_EOF'
PACKAGE_NAME="ch341-nodtr"
PACKAGE_VERSION="1.0"
BUILT_MODULE_NAME[0]="ch341"
DEST_MODULE_LOCATION[0]="/kernel/drivers/usb/serial/"
AUTOINSTALL="yes"
MAKE[0]="make KERNELDIR=/lib/modules/${kernelver}/build"
CLEAN="make KERNELDIR=/lib/modules/${kernelver}/build clean"
DKMS_EOF

    dkms remove ch341-nodtr/1.0 --all 2>/dev/null || true
    dkms add ch341-nodtr/1.0
    dkms build ch341-nodtr/1.0
    dkms install --force ch341-nodtr/1.0
else
    make
    make install
fi

echo "options ch341 no_dtr_on_open=1" > /etc/modprobe.d/ch341.conf

modprobe -r ch341 2>/dev/null || true
modprobe ch341

echo "SUCCESS"
