#!/usr/bin/env bash
# Windows 설치 파일(.exe 설치 프로그램과 .zip)을 만든다(docs/platform/packaging.md): Rust 백엔드를 빌드해 Electron 앱
# (desktop/)에 넣고 electron-builder로 묶는다. Git Bash(MSYS)에서 돈다 — CI의 Windows 실행기도 같다.
#
#   ./scripts/package_windows.sh
#
# 서명(Authenticode)은 아직 없다 — 인증서가 필요하다(TODO(phase2.9)). 선택 기능(sdr)은 넣지 않는다.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) ;;
  *) echo "Windows(Git Bash)에서만 만든다" >&2; exit 1 ;;
esac
command -v npm >/dev/null 2>&1 || { echo "Node.js(npm)가 필요하다" >&2; exit 1; }

# 1. 백엔드.
TARGET=x86_64-pc-windows-msvc
cargo build --release --locked -p tacet --target "$TARGET"
mkdir -p desktop/resources/bin
cp "target/$TARGET/release/tacet.exe" desktop/resources/bin/

# 2. 화면: 잠긴 의존성 그대로 설치하고 묶는다. 결과는 target/release/bundle/.
cd desktop
npm ci --no-audit --no-fund
CSC_IDENTITY_AUTO_DISCOVERY=false npx electron-builder --win --x64 --publish never
ls ../target/release/bundle/*.exe ../target/release/bundle/*.zip
