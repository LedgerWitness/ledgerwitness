#!/usr/bin/env bash
# Runs every vector in vectors.json through the strkey decoders of Rust
# (stellar-strkey), Go (go-stellar-sdk), JavaScript (@stellar/stellar-sdk,
# two releases), Python (py-stellar-base) and Java (java-stellar-sdk), then
# scores them.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
out="${1:-$repo_root/results/strkey}"
mkdir -p "$out"
v="$here/vectors.json"
cargo run -q --locked --manifest-path "$repo_root/Cargo.toml" -p ledgerwitness-strkey-check -- "$v" > "$out/rust.json"
(cd "$here/go" && "${GO:-go}" run . "$v") > "$out/go.json"
for ver in 17.4.0 17.2.0; do node "$here/check.mjs" "$v" "$ver" > "$out/js-$ver.json"; done
PYTHONPATH="$repo_root/runners/python/.deps" python3 "$here/check.py" "$v" > "$out/python.json"
"$repo_root/runners/java/fetch.sh"
java -cp "$repo_root/runners/java/.deps/*" "$here/Check.java" "$v" > "$out/java.json"
node "$here/score.mjs" "$out/rust.json" "$out/go.json" "$out/js-17.4.0.json" "$out/js-17.2.0.json" "$out/python.json" "$out/java.json"
