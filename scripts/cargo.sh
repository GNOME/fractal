#!/bin/sh

export CARGO_HOME=$1/target/cargo-home
export FRACTAL_PROFILE="$3"

if [ "$FRACTAL_PROFILE" = "Devel" ]
then
    echo "DEBUG MODE"
    cargo build --manifest-path $1/Cargo.toml -p fractal-gtk && cp $1/target/debug/fractal-gtk $2
else
    echo "RELEASE MODE"
    cargo build --manifest-path $1/Cargo.toml --release -p fractal-gtk && cp $1/target/release/fractal-gtk $2
fi
