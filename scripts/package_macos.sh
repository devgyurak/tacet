#!/usr/bin/env bash
# macOS 설치 파일(.dmg)을 만든다(docs/platform/packaging.md): Rust 백엔드를 유니버설(Apple 실리콘 + Intel)로 빌드해
# Electron 앱(desktop/)에 넣고 electron-builder로 묶는다. 서명은 환경 변수로만 받고 인증서·키는 저장소에 두지 않는다.
#
#   ./scripts/package_macos.sh                                       # 서명 없는 .dmg
#   TACET_SIGN_IDENTITY="Developer ID Application: ..." ./scripts/package_macos.sh   # 서명만
#   TACET_SIGN_IDENTITY=... APPLE_API_KEY=<.p8 경로> APPLE_API_KEY_ID=... APPLE_API_ISSUER=... \
#     ./scripts/package_macos.sh                                     # 서명 + 공증(.app과 .dmg에 티켓)
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
  # 배포 서명: Developer ID + Hardened Runtime(공증의 전제, 설계 §7.3). 자격은 build/entitlements.mac*.plist에 있다 —
  # 위치 자격이 없으면 Hardened Runtime에서 위치 권한을 못 받아 Wi-Fi BSSID가 빈다(설계 §7.2).
  # APPLE_API_KEY(.p8 경로)·APPLE_API_KEY_ID·APPLE_API_ISSUER가 있으면 electron-builder가 .app을 공증하고 티켓을 붙인 뒤
  # .dmg에 넣는다. 시험장 노트북은 인터넷이 없을 수 있어 Gatekeeper가 온라인 확인 없이 통과시키도록 티켓이 앱에 붙어야 한다.
  # electron-builder는 "Developer ID Application:" 접두사를 붙인 이름을 거부하고 접두사 없는 이름으로 인증서를 고른다.
  # 키체인에 보이는 전체 이름을 그대로 넘겨도 되게 여기서 뗀다.
  CSC_NAME="${TACET_SIGN_IDENTITY#Developer ID Application: }" npx electron-builder --mac --universal --publish never \
    -c.mac.hardenedRuntime=true -c.dmg.sign=true
else
  # 배포 서명 없이 임시(ad-hoc) 서명만 한다 — 서명이 전혀 없으면 Apple 실리콘에서 뜨지 않는다. 배포 서명이 아니고,
  # CoreWLAN은 Developer ID로 서명된 앱이어야 와이파이 주소를 준다(설계 §7.2).
  CSC_IDENTITY_AUTO_DISCOVERY=false npx electron-builder --mac --universal --publish never -c.mac.identity=- -c.mac.hardenedRuntime=false
fi
dmg="$(ls ../target/release/bundle/*.dmg)"

# 3. 공증: .dmg도 공증하고 티켓을 붙인다 — 내려받은 .dmg를 열 때도 Gatekeeper가 막지 않게.
if [ -n "${TACET_SIGN_IDENTITY:-}" ] && [ -n "${APPLE_API_KEY:-}" ]; then
  notary=(--key "$APPLE_API_KEY" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER")
  set +e
  result="$(xcrun notarytool submit "$dmg" "${notary[@]}" --wait --output-format json)"
  set -e
  echo "$result"
  status="$(python3 -c 'import json, sys; print(json.loads(sys.argv[1]).get("status", ""))' "$result" 2>/dev/null || true)"
  if [ "$status" != "Accepted" ]; then
    # 거부 이유(어느 파일이 왜)는 공증 기록에 있다.
    id="$(python3 -c 'import json, sys; print(json.loads(sys.argv[1]).get("id", ""))' "$result" 2>/dev/null || true)"
    [ -n "$id" ] && xcrun notarytool log "$id" "${notary[@]}"
    echo "공증 실패: ${status:-응답 없음}" >&2
    exit 1
  fi
  xcrun stapler staple "$dmg"
fi
ls "$dmg"
