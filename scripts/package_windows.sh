#!/usr/bin/env bash
# Windows 배포 파일을 만든다(docs/platform/packaging.md): Rust 백엔드를 빌드해 Electron 앱(desktop/)에 넣고
# electron-builder로 묶는다. Git Bash(MSYS)에서 돈다 — CI의 Windows 실행기도 같다.
#
#   - .msix: 표준 배포. Microsoft Store에 올리면 Microsoft가 서명한다. 서명 없는 이 파일은 직접 설치용이 아니다.
#   - .exe 설치 프로그램·.zip: Store를 쓸 수 없는 PC용. Authenticode 서명이 없다(TODO(phase2.9)).
#
#   ./scripts/package_windows.sh
#   TACET_APPX_IDENTITY_NAME=... TACET_APPX_PUBLISHER="CN=..." TACET_APPX_PUBLISHER_DISPLAY_NAME=... ./scripts/package_windows.sh
#
# TACET_APPX_* 는 Partner Center의 제품 ID 값이다(공개 정보, 비밀값 아님). 없으면 시험용 기본값으로 묶이고 그 .msix는 Store에
# 올릴 수 없다. 선택 기능(sdr)은 넣지 않는다.
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
appx=()
[ -n "${TACET_APPX_IDENTITY_NAME:-}" ] && appx+=("-c.appx.identityName=$TACET_APPX_IDENTITY_NAME")
[ -n "${TACET_APPX_PUBLISHER:-}" ] && appx+=("-c.appx.publisher=$TACET_APPX_PUBLISHER")
[ -n "${TACET_APPX_PUBLISHER_DISPLAY_NAME:-}" ] && appx+=("-c.appx.publisherDisplayName=$TACET_APPX_PUBLISHER_DISPLAY_NAME")
CSC_IDENTITY_AUTO_DISCOVERY=false npx electron-builder --win --x64 --publish never "${appx[@]}"
ls ../target/release/bundle/*.msix ../target/release/bundle/*.exe ../target/release/bundle/*.zip
