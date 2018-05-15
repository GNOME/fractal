#!/bin/sh
#
# This scripts is used to bundle all the dependencies in a .app bundle for macOS.
# It is called by meson if the `mac_bundle` option is set to true.

if [ $# -ne 2 ]; then
    echo "usage: $0 /path/to/Fractal.app/Contents/MacOS/fractal /path/to/Fractal.app" >&2
    exit 1
fi

# Useful for debugging, but meson doesn't eat the outputs of install scripts
#set -eux
set -eu

BINARY="$1"
APP_BUNDLE="$2"

CONTENTS="$APP_BUNDLE/Contents"

FRAMEWORKS="$CONTENTS/Frameworks" # Where all the libraries live
RESOURCES="$CONTENTS/Resources" # Where all the resources are

mkdir -p $FRAMEWORKS $RESOURCES

# List of library that where already copied and fixed
DONE=""

# Copy and fix a dependency
copy_and_fix () {
    OLD_PATH="$1"
    LIB_NAME=`basename $OLD_PATH`
    NEW_PATH="$FRAMEWORKS/$LIB_NAME"
    if echo "$DONE" | grep -q $LIB_NAME; then return; fi

    cp $OLD_PATH $NEW_PATH
    chmod u+w $NEW_PATH
    install_name_tool -change "$OLD_PATH" "$LIB_NAME" "$NEW_PATH"
    install_name_tool -id "@rpath/$LIB_NAME" "$NEW_PATH"

    DONE="$DONE $LIB_NAME"
    fix "$NEW_PATH"
}

# Fix a lib or binary by searching all it's dependencies, copying them inside
# the Frameworks directory, fix the library search path and recursivly fix it's
# dependencies
fix () {
    LIBS="`otool -L "$1" | grep '\(local\|opt\)' | awk '{ print $1 }'`"

    for LIB in $LIBS; do
        install_name_tool -change "$LIB" "@rpath/`basename $LIB`" "$1"
        copy_and_fix "$LIB"
    done
}

install_name_tool -add_rpath "@executable_path/../Frameworks" "$BINARY" || echo "Executable already has 'Frameworks' in his @rpath"
fix $BINARY

# GTK Input modules stuff
GTK_VERSION=`pkg-config gtk+-3.0 --variable gtk_binary_version`
GTK_PREFIX=`pkg-config gtk+-3.0 --variable prefix`

for LIB in `gtk-query-immodules-3.0 | grep .so | sed 's/"//g'`; do
    copy_and_fix $LIB
done

gtk-query-immodules-3.0 \
  | sed "s|$GTK_PREFIX/lib/gtk-3.0/$GTK_VERSION/immodules/|@rpath/|g" \
  | sed "s|$GTK_PREFIX/shareaa/locale|@executable_path/../Resources/locale|g" \
  > $RESOURCES/gtk.immodules


# GDK Pixbuf modules things
GDK_PIXBUF_PREFIX=`pkg-config gdk-pixbuf-2.0 --variable prefix`
GDK_PIXBUF_VERSION=`pkg-config gdk-pixbuf-2.0 --variable gdk_pixbuf_binary_version`

for LIB in `pkg-config gdk-pixbuf-2.0 --variable=gdk_pixbuf_moduledir`/*.so; do
    copy_and_fix $LIB
done

gdk-pixbuf-query-loaders \
  | sed "s|$GDK_PIXBUF_PREFIX/lib/gdk-pixbuf-2.0/$GDK_PIXBUF_VERSION/loaders/|@rpath/|g" \
  > $RESOURCES/gdk-loaders.cache


# Compile the GLib schemas
mkdir -p $RESOURCES/glib-2.0/schemas
glib-compile-schemas --targetdir=$RESOURCES/glib-2.0/schemas "`brew --prefix gtk+3`/share/glib-2.0/schemas"


# Copy the theme and icons
mkdir -p $RESOURCES/icons
cp -a `brew --prefix adwaita-icon-theme`/share/icons/* $RESOURCES/icons
cp -a `brew --prefix hicolor-icon-theme`/share/icons/* $RESOURCES/icons

mkdir -p $RESOURCES/themes/Mac
cp -a `brew --prefix gtk+3`/share/themes/Mac/* $RESOURCES/themes/Mac


# Create the GTK settings file
mkdir -p $RESOURCES/etc/gtk-3.0
cat > $RESOURCES/etc/gtk-3.0/settings.ini << EOF
[Settings]
gtk-theme-name=Adwaita
EOF


# Copy mime database
mkdir -p $RESOURCES/mime
cp -a $(pkg-config shared-mime-info --variable=prefix)/share/mime/mime.cache \
  $RESOURCES/mime
