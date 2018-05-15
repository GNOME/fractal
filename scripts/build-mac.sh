#!/bin/sh

# This scripts configures meson for creating a .app bundle for macOS.
#
#   scripts/build-mac.sh /tmp/Fractal.app _mac
#   ninja -C _mac install
#   open /tmp/Fractal.app

if [ $# -lt 2 ]; then
    echo "usage: $0 /path/to/Fractal.app [meson options]" >&2
    exit 1
fi

APP_BUNDLE=$1
shift

meson \
    --prefix $APP_BUNDLE \
    --bindir Contents/MacOS \
    --libdir Contents/Frameworks \
    --datadir Contents/Resources \
    -Dmac_bundle=true \
    $@
