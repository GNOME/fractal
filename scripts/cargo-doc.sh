#!/bin/sh

export OUTPUT="$2"
export CARGO_TARGET_DIR="$3"/target
export CARGO_HOME="$CARGO_TARGET_DIR"/cargo-home
export FRACTAL_PROFILE="$4"

echo "GENERATING DOCUMENTATION"
cargo doc --manifest-path $1/Cargo.toml -p fractal-matrix-api --no-deps &&
cargo rustdoc --manifest-path $1/Cargo.toml -p fractal-gtk -- --document-private-items
