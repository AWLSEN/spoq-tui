#!/bin/bash

show_build_time() {
    local binary="$1"
    local type="$2"
    if [[ -f "$binary" ]]; then
        local timestamp=$(stat -f "%Sm" -t "%Y-%m-%d %H:%M:%S" "$binary")
        echo "Last $type build: $timestamp"
    else
        echo "No $type build found"
    fi
}

case "$1" in
    --debug|-d)
        show_build_time "target/debug/spoq" "debug"
        cargo run
        ;;
    --release|-r)
        show_build_time "target/release/spoq" "release"
        cargo run --release
        ;;
    *)
        echo "Usage: ./run.sh [--debug|-d] [--release|-r]"
        echo ""
        show_build_time "target/debug/spoq" "debug"
        show_build_time "target/release/spoq" "release"
        exit 1
        ;;
esac
