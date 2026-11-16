# 설치 파일과 서명(Phase 2.9)

- 상태: main의 `vX.Y.Z` 태그로 서명 없는 macOS `.dmg`와 Windows 설치 프로그램(`.exe`)·`.zip`을 GitHub 릴리스에 올린다.
  배포 서명·공증은 인증서가 필요해 미완.

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
| `build (macos)` | `scripts/package_macos.sh` → 아키텍처·서명·백엔드·앱 실행·이미지 확인 → `tacet-<버전>-mac-universal.dmg` |
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

서명은 환경 변수로만 받는다(`TACET_SIGN_IDENTITY`). 없으면 임시(ad-hoc) 서명만 하고 Hardened Runtime을 끈다 — 서명이
전혀 없으면 Apple 실리콘에서 뜨지 않고, 임시 서명과 Hardened Runtime을 함께 쓰면 라이브러리 검증에 걸린다. 배포 서명이
아니다. 인증서·키·Apple 계정 정보는 저장소에 두지 않는다(AGENTS.md §7). 공증(`notarytool`)은 계정이 준비되면 같은
스크립트에 더한다 — `TODO(phase2.9)`.

## Windows

`scripts/package_windows.sh`(Git Bash)가 `x86_64-pc-windows-msvc` 백엔드를 빌드하고 electron-builder로 NSIS 설치 프로그램
(`.exe`)과 `.zip`을 만든다. Authenticode 서명은 아직 없다 — `TODO(phase2.9)`. 시험 기간 중
자동 업데이트는 넣지 않는다.
