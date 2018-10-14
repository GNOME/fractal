#!/bin/sh

export CARGO_HOME=$1/target/cargo-home
export FRACTAL_LOCALEDIR="$3"
export FRACTAL_APP_ID="$4"
export FRACTAL_NAME_SUFFIX="$5"
export FRACTAL_VERSION="$6"
export FRACTAL_PROFILE="$7"

if [ "$FRACTAL_PROFILE" = "Devel" ]
then
    echo "DEBUG MODE"
    cargo build --manifest-path $1/Cargo.toml -p fractal-gtk && cp $1/target/debug/fractal-gtk $2
else
    echo "RELEASE MODE"
    cargo build --manifest-path $1/Cargo.toml --release -p fractal-gtk && cp $1/target/release/fractal-gtk $2
fi
