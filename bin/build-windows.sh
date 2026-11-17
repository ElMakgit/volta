#!/usr/bin/env sh
# Cross-build volta for 64-bit Windows from Linux.
#
# Needs a Linux-hosted LLVM-MinGW toolchain (the releases from
# https://github.com/mstorsjo/llvm-mingw). Point LLVM_MINGW at its directory,
# or unpack it into ~/.local/toolchains; the script finds either.
#
# The rustup target is enough for the standard library; the linker and the
# Windows import libraries come from LLVM-MinGW. Rust's windows-gnu target
# asks for libgcc and libgcc_eh, which LLVM-MinGW ships as compiler-rt and
# libunwind, so the script makes two symlinks instead of pulling a second
# toolchain.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
cd "$here"

target=x86_64-pc-windows-gnu

find_llvm_mingw() {
    if [ -n "${LLVM_MINGW:-}" ] && [ -d "$LLVM_MINGW/bin" ]; then
        printf '%s\n' "$LLVM_MINGW"
        return 0
    fi
    for dir in "$HOME"/.local/toolchains/llvm-mingw-* "$HOME"/llvm-mingw-* /opt/llvm-mingw; do
        if [ -d "$dir/bin" ] && [ -x "$dir/bin/x86_64-w64-mingw32-clang" ]; then
            printf '%s\n' "$dir"
            return 0
        fi
    done
    return 1
}

if ! mingw=$(find_llvm_mingw); then
    echo "LLVM-MinGW not found. Unpack a Linux x86_64 release into" >&2
    echo "~/.local/toolchains/ or set LLVM_MINGW to its directory." >&2
    echo "https://github.com/mstorsjo/llvm-mingw/releases" >&2
    exit 1
fi

if ! rustup target list --installed | grep -qx "$target"; then
    rustup target add "$target"
fi

export PATH="$mingw/bin:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-clang

# Rust asks for these two names; LLVM-MinGW has the same code under the
# compiler-rt and libunwind names.
shim="target/windows-libs"
mkdir -p "$shim"
builtins=$(find "$mingw/lib/clang" -name libclang_rt.builtins-x86_64.a | head -n 1)
unwind="$mingw/x86_64-w64-mingw32/lib/libunwind.a"
if [ -z "$builtins" ] || [ ! -f "$unwind" ]; then
    echo "LLVM-MinGW is missing compiler-rt or libunwind in $mingw" >&2
    exit 1
fi
ln -sf "$builtins" "$shim/libgcc.a"
ln -sf "$unwind" "$shim/libgcc_eh.a"
RUSTFLAGS="-L $here/$shim ${RUSTFLAGS:-}"
export RUSTFLAGS

cargo build --offline --locked --release --target "$target" "$@"

out="target/windows"
mkdir -p "$out"
cp "target/$target/release/volta.exe" "$out/volta.exe"
if [ -f docs/windows.md ]; then
    cp docs/windows.md "$out/README-windows.md"
fi
cp LICENSE "$out/LICENSE"
if command -v zip >/dev/null 2>&1; then
    files="volta.exe LICENSE"
    if [ -f "$out/README-windows.md" ]; then
        files="volta.exe README-windows.md LICENSE"
    fi
    # shellcheck disable=SC2086
    (cd "$out" && rm -f volta-windows-x86_64.zip && zip -q -9 -j volta-windows-x86_64.zip $files)
    echo "Windows build: $here/$out/volta-windows-x86_64.zip"
else
    echo "Windows build: $here/$out/volta.exe (zip not installed, not packaged)"
fi
