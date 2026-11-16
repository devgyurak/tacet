#!/usr/bin/env bash
# macOS 설치 파일(.dmg)을 만든다(docs/platform/packaging.md): Rust 백엔드를 유니버설(Apple 실리콘 + Intel)로 빌드해
# Electron 앱(desktop/)에 넣고 electron-builder로 묶는다. 서명은 환경 변수로만 받고 인증서·키는 저장소에 두지 않는다.
#
#   ./scripts/package_macos.sh                                       # 서명 없는 .dmg
#   TACET_SIGN_IDENTITY="Developer ID Application: ..." ./scripts/package_macos.sh
#
# 선택 기능(sdr)은 넣지 않는다 — SoapySDR을 함께 배포하는 방법이 정해지지 않았다(TODO(phase2.9)).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
[ "$(uname -s)" = "Darwin" ] || { echo "macOS에서만 만든다" >&2; exit 1; }
command -v npm >/dev/null 2>&1 || { echo "Node.js(npm)가 필요하다" >&2; exit 1; }

# 1. 백엔드: 두 아키텍처를 따로 빌드해 lipo로 합친다.
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
rustup target add "${TARGETS[@]}"
BINARIES=()
for target in "${TARGETS[@]}"; do
  cargo build --release --locked -p tacet --target "$target"
  BINARIES+=("target/$target/release/tacet")
done
mkdir -p desktop/resources/bin
lipo -create -output desktop/resources/bin/tacet "${BINARIES[@]}"
echo "백엔드 아키텍처: $(lipo -archs desktop/resources/bin/tacet)"

# 2. 화면: 잠긴 의존성 그대로 설치하고 묶는다. 결과는 target/release/bundle/.
cd desktop
npm ci --no-audit --no-fund
if [ -n "${TACET_SIGN_IDENTITY:-}" ]; then
  # 공증을 받으려면 Hardened Runtime이 필요하다(설계 §7.3). 공증(notarytool)은 아직 없다(TODO(phase2.9)).
  CSC_NAME="$TACET_SIGN_IDENTITY" npx electron-builder --mac --universal --publish never \
    -c.mac.hardenedRuntime=true
else
  # 배포 서명 없이 임시(ad-hoc) 서명만 한다 — 서명이 전혀 없으면 Apple 실리콘에서 뜨지 않는다. 배포 서명이 아니고,
  # CoreWLAN은 Developer ID로 서명된 앱이어야 와이파이 주소를 준다(설계 §7.2).
  CSC_IDENTITY_AUTO_DISCOVERY=false npx electron-builder --mac --universal --publish never -c.mac.identity=- -c.mac.hardenedRuntime=false
fi
ls ../target/release/bundle/*.dmg
