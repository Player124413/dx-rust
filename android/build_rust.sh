#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${ANDROID_SDK_ROOT:?Set ANDROID_SDK_ROOT or ANDROID_HOME}"
NDK="${ANDROID_NDK_HOME:-${NDK_HOME:-$ANDROID_SDK_ROOT/ndk/27.2.12479018}}"
export ANDROID_NDK_HOME="$NDK"
export CARGO_TERM_COLOR=never
command -v cargo-ndk >/dev/null || { echo "Install cargo-ndk: cargo install cargo-ndk --version 4.1.2 --locked"; exit 1; }
OUT=android/app/src/main/jniLibs
ABIS="${ABIS:-arm64-v8a armeabi-v7a x86_64}"
targets=()
for abi in $ABIS; do targets+=(-t "$abi"); done
cargo ndk "${targets[@]}" -o "$OUT" build --profile release --lib -p dxview
# Bevy/cpal use the shared C++ runtime on Android; package the NDK's matching libc++_shared.so.
HOST="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
SYS="$NDK/toolchains/llvm/prebuilt/$HOST/sysroot/usr/lib"
for pair in "arm64-v8a:aarch64-linux-android" "armeabi-v7a:arm-linux-androideabi" "x86_64:x86_64-linux-android"; do
  ABI="${pair%%:*}"; TRIPLE="${pair##*:}"
  case " $ABIS " in *" $ABI "*) ;; *) continue ;; esac
  mkdir -p "$OUT/$ABI"
  cp "$SYS/$TRIPLE/libc++_shared.so" "$OUT/$ABI/libc++_shared.so"
done
