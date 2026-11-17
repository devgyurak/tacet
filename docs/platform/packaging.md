# 설치 파일과 서명(Phase 2.9)

- 상태: main의 `vX.Y.Z` 태그로 macOS `.dmg`(Developer ID 서명 + 공증, v0.1.1부터)와 Windows 설치 프로그램(`.exe`)·`.zip`을
  GitHub 릴리스에 올리고, Windows 표준 배포인 Store용 `.msix`를 작업 산출물로 만든다. Store 게시(Partner Center 등록·제출)는
  미완. `.exe`의 Authenticode 서명은 인증서가 없어 미완.

## 테스트 (`.github/workflows/ci.yml`)

`develop`·`main` 푸시와 PR마다 `test (windows)` · `test (macos)` · `test (linux)`가 `./scripts/verify.sh` 전체 게이트
(테스트·커버리지 포함)를 돌린다. 빌드는 하지 않는다.

## 릴리스 (`.github/workflows/release.yml`)

1. `develop` → `main` 릴리스 PR에서 `Cargo.toml`의 `[workspace.package] version`과 `desktop/package.json`의 `version`을
   함께 `X.Y.Z`로 올린다.
2. main에 병합된 커밋에 태그를 단다: `git tag vX.Y.Z origin/main && git push origin vX.Y.Z`.

| 작업 | 하는 일 |
|---|---|
| 태그 확인 | 태그가 `vX.Y.Z` 형식인지(접미사 없음), 태그 커밋이 main에 있는지, `Cargo.toml`·`desktop/package.json` 버전과 같은지. 하나라도 틀리면 멈춘다 |
| 테스트 | `ci.yml`을 불러 세 OS에서 전체 게이트 |
| `build (windows)` | `scripts/package_windows.sh` → 백엔드·앱 실행 확인 → `tacet-<버전>-win-x64.exe`(설치 프로그램), `.zip` |
| `build (macos)` | 서명 키체인 → `scripts/package_macos.sh`(서명·공증) → 아키텍처·서명·자격·티켓·Gatekeeper·백엔드·앱 실행·이미지 확인 → `tacet-<버전>-mac-universal.dmg` |
| GitHub 릴리스 | `SHA256SUMS.txt`를 만들고, 설치 파일 안내 + 자동 생성 노트로 릴리스를 만든다 |

- 릴리스 노트는 이전 태그 이후 병합된 PR을 라벨별로 묶는다(`.github/release.yml`). 라벨은 커밋 접두사에서 온다
  (`commit` 스킬 §7). 노트에서 빼려면 `skip-changelog` 라벨을 단다.
- 실행 확인: 백엔드를 없는 설정 파일로 띄워 한국어 오류로 끝나는지 보고, 묶은 앱을 시연 모델(`--demo`)로 띄워 창을
  캡처한다(`--screenshot`). 수집기와 권한은 CI에서 확인할 수 없다(실기 시험의 몫).
- 같은 태그를 다시 쓰지 않는다. 릴리스가 실패하면 고친 뒤 다음 패치 버전으로 다시 낸다.
- 선택 기능(`sdr`)은 배포 빌드에 넣지 않는다 — SoapySDR·libhackrf를 함께 배포하는 방법이 먼저다.

## 구성

Electron 앱(`desktop/`)의 리소스 `bin/`에 Rust 백엔드(`tacet`)를 넣는다. 앱은 실행하면 그 백엔드를 자식 프로세스로
띄운다. 제품 이름은 ASCII인 `Tacet`이다 — Electron 보조
프로세스 이름이 제품 이름에서 오는데, 한글 제품 이름이면 macOS 묶음 앱이 시작하자마자 멈췄다(2026-11-16 확인). 제품 이름은
ASCII로 둔다. 창 제목에는 "Tacet — 시험장 무선 기기 탐지기"로 설명을 붙인다.

## macOS

`scripts/package_macos.sh`가 백엔드를 Apple 실리콘과 Intel로 따로 빌드해 `lipo`로 합치고, electron-builder로 유니버설
`.dmg`를 만든다. 블루투스·위치 사용 목적 문구(한국어)는 `desktop/package.json`의 `build.mac.extendInfo`에 있다 — CoreWLAN은
위치 권한과 서명된 앱이 있어야 BSSID를 준다(설계 §7.2).

서명은 환경 변수로만 받는다. 인증서·키·Apple 계정 정보는 저장소에 두지 않는다(AGENTS.md §7).

| 환경 변수 | 뜻 |
|---|---|
| `TACET_SIGN_IDENTITY` | 키체인의 `Developer ID Application: …` 이름. 없으면 임시(ad-hoc) 서명만 하고 Hardened Runtime을 끈다 — 서명이 전혀 없으면 Apple 실리콘에서 뜨지 않고, 임시 서명과 Hardened Runtime을 함께 쓰면 라이브러리 검증에 걸린다. 배포 서명이 아니다 |
| `APPLE_API_KEY` · `APPLE_API_KEY_ID` · `APPLE_API_ISSUER` | App Store Connect 팀 API 키(`.p8` 경로)와 ID들. 있으면 공증한다 |

- **서명**: Developer ID + Hardened Runtime(공증의 전제). 자격은 `desktop/build/entitlements.mac.plist`(앱)와
  `entitlements.mac.inherit.plist`(보조 프로세스·백엔드)에 있다 — Electron 기본 세 가지에 **위치**를 더했다. Hardened
  Runtime에서는 위치 자격이 없으면 위치 권한을 요청하지 못하고, CoreWLAN은 위치 권한이 있어야 BSSID를 준다(설계 §7.2).
- **공증**: electron-builder가 `.app`을 공증하고 티켓을 붙인 뒤 `.dmg`에 넣는다. 스크립트가 서명한 `.dmg`도 `notarytool`로
  공증하고 티켓을 붙인다. 시험장 노트북은 인터넷이 없을 수 있어 Gatekeeper가 온라인 확인 없이 통과시키도록 티켓을 둘 다에
  붙인다. 거부되면 `notarytool log`로 이유를 찍고 멈춘다.
- **CI**(`release.yml`): 비밀값 `MAC_CERT_P12`(base64 `.p12`)·`MAC_CERT_PASSWORD`·`APPLE_API_KEY`(`.p8` 내용)·
  `APPLE_API_KEY_ID`·`APPLE_API_ISSUER`로 임시 키체인을 만든다. Apple Developer ID 중간 인증서(G2)는 실행기에도
  electron-builder에도 없어서 apple.com에서 받아 고정한 SHA-256 지문으로 확인한 뒤 넣는다 — 빠지면 인증서가 "유효하지
  않음"으로 보여 서명하지 못한다. 작업이 끝나면 키체인과 `.p8`을 지운다.
- **로컬**: 인증서를 만든 Mac의 로그인 키체인에 G2 중간 인증서도 있어야 한다(`security find-identity -v -p codesigning`에
  유효한 항목으로 보여야 한다).

## Windows

**Windows 표준 배포는 Microsoft Store의 MSIX다**(사용자 결정 2026-11-17). Store에 올린 묶음은 Microsoft가 서명하므로
Authenticode 인증서 없이 SmartScreen 경고 없이 설치된다. `scripts/package_windows.sh`(Git Bash)가 `x86_64-pc-windows-msvc`
백엔드를 빌드하고 electron-builder로 세 가지를 만든다.

| 파일 | 쓰임 |
|---|---|
| `tacet-<버전>-win-x64.msix` | **표준.** Partner Center에 올린다(릴리스 작업 산출물 `windows-store`). 서명이 없어 직접 설치용이 아니므로 GitHub 릴리스 파일에는 넣지 않는다 |
| `tacet-<버전>-win-x64.exe`(NSIS) · `.zip` | Store를 쓸 수 없는 PC(Store를 막은 기관 노트북, 인터넷 없는 설치)용. Authenticode 서명이 없다 — `TODO(phase2.9)` |

- **매니페스트**: `desktop/build/appxmanifest.xml`(electron-builder 기본 틀 + 권한). 권한은 `runFullTrust`(Electron과 백엔드가
  Win32 API를 쓴다, 제한 권한이라 제출 때 사유를 적는다), `location`(Windows 11 24H2부터 Wi-Fi BSSID에 위치 권한이 필요하고,
  묶음 앱은 매니페스트에 선언해야 받을 수 있다), `bluetooth`(BLE 광고 수신, 연결하지 않는다). electron-builder의
  `appx.capabilities` 목록에 `bluetooth`가 없어 틀을 직접 둔다. 묶음 앱으로 실행할 때 수집이 되는지는 실기 미확인 —
  `TODO(field-unverified)`.
- **타일 이미지**: `desktop/build/appx/`(앱 아이콘 판 `#1b1b1a` 위에 반전 표식, 작은 크기는 `tacet-symbol-small` 모양).
- **제품 ID**: Partner Center의 `Package/Identity/Name`·`Publisher`·`PublisherDisplayName`은 공개 값이라 저장소 변수
  `TACET_APPX_IDENTITY_NAME`·`TACET_APPX_PUBLISHER`·`TACET_APPX_PUBLISHER_DISPLAY_NAME`(`gh variable set`)으로 넣는다. 없으면
  시험용 기본값(`CN=ms`)으로 묶이고 그 `.msix`는 Store에 올릴 수 없다 — 릴리스가 경고를 남긴다.
- **확인**: `scripts/check_msix.py`가 권한·타일 이미지(electron-builder 견본 그림이 아닌지)·제품 ID를 본다. 묶음 설정을 바꾼
  PR은 `.github/workflows/package.yml`이 Windows에서 실제로 묶어 본다.
- **자동 업데이트**: Store 앱은 Store가 자동으로 업데이트하고 게시자가 막을 수 없다. 그래서 시험 기간에는 Store에 새 버전을
  게시하지 않는다(AGENTS.md §7). 기관이 Store 자동 업데이트를 끄려면 Windows 정책으로 끈다.
- **Store 제출**(사람이 Partner Center에서): 무료·시장, 연령 등급(IARC), 한국어 소개·스크린샷·로고, 개인정보 처리방침 주소
  (`docs/privacy.md`), `runFullTrust` 사유, 추가 사용권 조건(`LICENSE` 주소 — 지정하지 않으면 Microsoft 표준 앱 사용권 조건만
  보인다. Store 조건과 `LICENSE`의 관계는 법률 검토 대상). 심사는 보통 영업일 며칠.
