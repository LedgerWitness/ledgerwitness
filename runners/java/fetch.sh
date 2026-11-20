#!/usr/bin/env bash
# Downloads the pinned java-stellar-sdk jar and the dependencies the runner
# loads into runners/java/.deps, refusing any file whose SHA-256 differs.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p .deps
M=https://repo1.maven.org/maven2
while read -r sum path; do
  f=".deps/$(basename "$path")"
  [ -f "$f" ] || curl -sSfL --retry 6 --retry-all-errors --retry-delay 5 -o "$f" "$M/$path"
  echo "$sum  $f" | sha256sum -c --quiet || { rm -f "$f"; exit 1; }
done <<'JARS'
d13e24aa87eaddf4c54296d1af97ae7cc010290b308ad4876ccb94a99ac55be9 network/lightsail/stellar-sdk/6.0.0/stellar-sdk-6.0.0.jar
96d3dd843dc2911de01cdabcd50064cb9e860353102a0a78c96fd833d396fc7d com/google/code/gson/gson/2.15.0/gson-2.15.0.jar
2af190b300cbb0b35e248ccf5f4a06b6072030aeb3da7a98ec73abe5b4cb371f org/bouncycastle/bcprov-jdk18on/1.86/bcprov-jdk18on-1.86.jar
47504a3ce71d73a1b0af07675546b8d8c5af87f82ef12b6381f9eb3c17dd5a5b commons-codec/commons-codec/1.23.0/commons-codec-1.23.0.jar
JARS
