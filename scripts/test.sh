#! /usr/bin/sh

set -o errexit
set -o pipefail

export CARGO_TARGET_DIR="$1/target/"
export CARGO_HOME="$CARGO_TARGET_DIR/cargo-home"

cargo test -j 1 -- --test-threads=1 --nocapture
