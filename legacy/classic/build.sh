#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
docker run --rm --platform linux/amd64 -v "$ROOT:/work" -w /work ghcr.io/autc04/retro68 \
  /bin/bash -lc 'cmake -S legacy/classic -B target/classic-ppc -DCMAKE_TOOLCHAIN_FILE=/Retro68-build/toolchain/powerpc-apple-macos/cmake/retroppc.toolchain.cmake && cmake --build target/classic-ppc'
