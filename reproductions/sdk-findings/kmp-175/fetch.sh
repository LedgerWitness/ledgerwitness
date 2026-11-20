#!/usr/bin/env bash
# Downloads kmp-stellar-sdk 1.18.0 (JVM) and its runtime dependencies into
# .deps, refusing any file whose SHA-256 differs.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p .deps
M=https://repo1.maven.org/maven2
while read -r sum path; do
  f=".deps/$(basename "$path")"
  [ -f "$f" ] || curl -sSfL --retry 6 --retry-all-errors --retry-delay 5 -o "$f" "$M/$path"
  echo "$sum  $f" | sha256sum -c --quiet || { rm -f "$f"; exit 1; }
done <<'JARS'
5e9b47cc37fa98e2cb339afc3e7482fbe6a208cf7c52ffdd838556b81a33a183 com/soneso/stellar/stellar-sdk-jvm/1.18.0/stellar-sdk-jvm-1.18.0.jar
add5915e6acfc6ab5836e1fd8a5e21c6488536a8c1f21f386eeb3bf280b702d7 org/bouncycastle/bcprov-jdk18on/1.78.1/bcprov-jdk18on-1.78.1.jar
c5c718466ef822290ed6ea2013dee788c468a993fa53f34d0e07acc8d6ef087d com/ionspin/kotlin/bignum-jvm/0.3.10/bignum-jvm-0.3.10.jar
5ca175b38df331fd64155b35cd8cae1251fa9ee369709b36d42e0a288ccce3fd org/jetbrains/kotlinx/kotlinx-coroutines-core-jvm/1.10.2/kotlinx-coroutines-core-jvm-1.10.2.jar
3b8b98657c9aff3be7f6b4b575e9f3fb9a3a1f452a801da0f438901734acdade org/jetbrains/kotlinx/kotlinx-datetime-jvm/0.7.1/kotlinx-datetime-jvm-0.7.1.jar
1f0afa172110e45a7231ef1b44ae8fd84c1ebaff96f3fc3ad68ef8c48120b59c org/jetbrains/kotlinx/kotlinx-serialization-core-jvm/1.9.0/kotlinx-serialization-core-jvm-1.9.0.jar
d94cc34cae39246a1af74fda63f9c4812ce12216ef641d5fa3bbbb539a6922d8 org/jetbrains/kotlinx/kotlinx-serialization-json-jvm/1.9.0/kotlinx-serialization-json-jvm-1.9.0.jar
6558a3d233da56a20934b32159f9db5f86ed5816ef098f78a2c223dc6abb79dd org/jetbrains/kotlin/kotlin-stdlib/2.2.21/kotlin-stdlib-2.2.21.jar
JARS
