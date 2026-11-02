#!/usr/bin/env bash
# Linux-host build check only; execution still requires an Android device/emulator.
set -eu
ndk_dir=${1:-${ANDROID_NDK_HOME:-}}
if [ -z "$ndk_dir" ]; then
    echo 'Set ANDROID_NDK_HOME or pass the NDK directory as the first argument.' >&2
    exit 2
fi
toolchain_dir="$ndk_dir/toolchains/llvm/prebuilt/linux-x86_64/bin"
compiler="$toolchain_dir/aarch64-linux-android24-clang"
if [ ! -x "$compiler" ]; then
    echo "Android ARM64/API24 Linux-host compiler not found: $compiler" >&2
    exit 2
fi
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$compiler"
export CC_aarch64_linux_android="$compiler"
export AR_aarch64_linux_android="$toolchain_dir/llvm-ar"
export CMAKE_TOOLCHAIN_FILE="$ndk_dir/build/cmake/android.toolchain.cmake"
export ANDROID_NDK="$ndk_dir"
export TURBOJPEG_STATIC=1 TURBOJPEG_DYNAMIC=0 TURBOJPEG_SOURCE=vendor
cargo build --locked --release --target aarch64-linux-android --features cli --bin dt-img -j 2
"$toolchain_dir/llvm-readelf" -h target/aarch64-linux-android/release/dt-img
"$toolchain_dir/llvm-readelf" -d target/aarch64-linux-android/release/dt-img
