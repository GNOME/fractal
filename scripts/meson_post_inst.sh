#!/bin/sh

SCHEMADIR=$MESON_INSTALL_PREFIX/share/glib-2.0/schemas

if [ ! -d $DESTDIR ]
then
    echo 'Compiling gsettings schemas...'
    glib-compile-schemas $SCHEMADIR
fi
