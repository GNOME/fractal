#!/bin/sh

SOURCE=$1
TARGET=$2
PACKAGE=${3:-"fractal-gtk"}
export CARGO_HOME=$SOURCE/target/cargo-home

set -x

if [[ $DEBUG = true ]]
then
    echo "DEBUG MODE"
    cargo build --manifest-path $SOURCE/Cargo.toml -p $PACKAGE && cp $SOURCE/target/debug/$PACKAGE $TARGET
else
    echo "RELEASE MODE"
    cargo build --manifest-path $SOURCE/Cargo.toml --release -p $PACKAGE && cp $SOURCE/target/release/$PACKAGE $TARGET
fi
