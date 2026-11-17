<div align="center">

<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/tacet-horizontal-reversed.svg">
    <img src="docs/brand/tacet-horizontal.svg" alt="Tacet" width="300">
  </picture>
</h1>

**시험 중 교실에 새로 나타난 휴대폰·스마트 안경의 무선 신호를 감독관에게 알리는 데스크톱 앱**

[![CI](https://github.com/devgyurak/tacet/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/devgyurak/tacet/actions/workflows/ci.yml)
[![베타](https://img.shields.io/badge/상태-베타-e96a26)](#설치)
[![Rust 1.98.1](https://img.shields.io/badge/rust-1.98.1-orange?logo=rust)](rust-toolchain.toml)
[![플랫폼: Windows 11 · macOS](https://img.shields.io/badge/플랫폼-Windows%2011%20%C2%B7%20macOS-informational)](#설치)

[설치](#설치) · [쓰는 법](#쓰는-법) · [하지 않는 것](#하지-않는-것) · [개발](#개발) · [문서](#문서) · [기여](#기여)

</div>

**Tacet**은 악보에서 "이 악기는 쉼"을 뜻합니다 — 시험 중에는 기기가 조용해야 합니다. 휴대폰 수거와 전원 차단이 끝난 교실에서
블루투스 광고, 와이파이 비콘, 셀룰러 업링크 같은 무선 신호가 새로 보이면 그것 자체가 이상 신호입니다. AI 안경도 대부분
휴대폰을 거쳐 밖과 통신하는 것으로 보입니다. Tacet은 이런 신호를 **듣기만 해서**(송신 없이) 찾고, 판정하지 않고 **"확인 필요"**로 알립니다.

- **서버 없이 노트북 한 대로** — 감독관 노트북 한 대가 그 교실을 혼자 지켜봅니다. 별도 장비가 필요 없습니다.
- **쉬운 말 알림** — 무엇이 보였나 · 어디서 · 얼마나 확실한가 · 무엇을 하면 되나. 기술 용어는 개발자 모드에서만 보입니다.
- **교실 보정** — 내 휴대폰 하나로 1분쯤 걸려 이 교실의 경계를 잽니다. 고정된 신호 세기 기준을 쓰지 않습니다.
- **창을 닫아도 계속** — 새 알림과 감지 문제는 운영체제 알림으로 옵니다.

## 설치

[GitHub 릴리스](https://github.com/devgyurak/tacet/releases)에서 내려받습니다.

| 운영체제 | 파일 |
|---|---|
| Windows 11 (x64) | `tacet-<버전>-win-x64.exe`(설치 프로그램) 또는 `.zip` |
| macOS (Apple 실리콘·Intel) | `tacet-<버전>-mac-universal.dmg` |

공식 릴리스 페이지에서 받은 파일만 여세요. 열기 전에 함께 올린 `SHA256SUMS.txt`로 내려받은 파일이 온전한지 확인합니다
(전송 오류 확인용이며, 코드 서명을 대신하지는 않습니다).

```bash
shasum -a 256 -c SHA256SUMS.txt --ignore-missing          # macOS
```

```powershell
Get-FileHash .\tacet-<버전>-win-x64.exe -Algorithm SHA256  # Windows: SHA256SUMS.txt의 값과 비교
```

> **코드 서명** — macOS 판은 v0.1.1부터 Developer ID로 서명하고 Apple 공증을 받았습니다. Windows 판은 아직 코드 서명이
> 없습니다.
> - **macOS** — `.dmg`의 Tacet을 응용 프로그램 폴더로 옮긴 뒤 엽니다. 인터넷이 없는 노트북에서도 열리도록 공증 확인서를
>   파일에 붙였습니다.
> - **Windows** — 처음 열 때 SmartScreen 창이 뜰 수 있습니다. "추가 정보" → "실행"을 누릅니다.
>
> macOS는 위치 권한과 Developer ID 서명이 있어야 와이파이 주소를 알려 줍니다. 서명이 없는 v0.1.0의 macOS 판은 휴대폰
> 핫스팟(숨긴 핫스팟 포함)을 놓칠 수 있으니 v0.1.1 이후 판을 쓰세요. 와이파이 주소를 받지 못하면 화면의 문제 안내에 그 사실이
> 보입니다.

## 쓰는 법

1. **처음 설정** — 교실 이름(예: `1-1반`)과, 칠판을 남쪽에 둔 5×5 교실 그림에서 노트북을 둔 칸을 고릅니다.
2. **교실 보정** — 곧바로 이어집니다. 목록에서 내 휴대폰(블루투스)과 내 핫스팟(와이파이)을 고르고, 앱이 추천하는 자리(노트북에서
   가장 먼 구석)에 휴대폰을 둡니다. 건너뛸 수 있지만, 그러면 새 기기가 교실 안인지 밖인지 가리지 못하고 "보정 필요" 안내가 남습니다.
3. **준비 시간** — 수험생 휴대폰을 걷은 뒤 시작하면, 시험 전부터 있던 신호(감독관 장비, 교실 설비)를 1·3·5분 동안 익힙니다.
4. **감시** — 새 신호가 보이면 쉬운 말로 알립니다. 창을 닫아도 감지는 계속되고, 알림은 운영체제 알림으로 옵니다. 끄려면 메뉴
   막대(Windows는 작업 표시줄 알림 영역)의 Tacet 아이콘에서 "종료"를 누릅니다. 종료하면 그 세션의 관측은 버려집니다.

교실·자리·보정을 처음부터 다시 하려면 오른쪽 위 **설정 ▾ → 설정 초기화**를 누릅니다(예외 기기 목록과 보고서는 남습니다).

**권한** — macOS는 블루투스와 위치 권한을 묻습니다. 위치 권한이 있어야 운영체제가 와이파이 주소를 알려 줍니다. Windows 11
24H2는 정밀 위치 권한이 있어야 와이파이 주소를 줍니다. 권한이 없으면 감지가 조용히 꺼지지 않고 화면의 문제 안내에 무엇이
빠졌는지 보입니다. 창을 닫아 둔 동안 알림을 받으려면 운영체제 알림도 허용해 주세요.

**개발자 모드** — `Ctrl/⌘ + Shift + D`. 규칙 ID·신호 세기·식별자 같은 기술 정보와 알림 기록, 예외 기기 목록(감독관 장비,
사전 신고한 의료기기·보청기), 그리고 교실마다 노트북을 두고 시험본부 서버로 모으는 건물 단위 구성(센서 · 서버 · 본부 화면 보기,
예시는 [`docs/examples/`](docs/examples))을 엽니다. 저장하지 않으므로 다음 실행에는 꺼져 있습니다.

## 하지 않는 것

- **전파를 쏘지 않습니다.** 재밍, 주입, 기기 연결, 광고 송신이 없습니다(전파법). 예외로 둔 것은 운영체제가 하는 스캔 두 가지입니다:
  macOS가 블루투스 스캔 때 보내는 스캔 요청(macOS는 수동 스캔을 지원하지 않습니다. Windows는 수동 스캔)과 운영체제 와이파이
  스캔이 내보내는 probe request. 이 두 예외는 법률 검토 대상입니다.
- **통신 내용을 해석하지 않습니다.** 신호 세기, 비콘 헤더, 광고 메타데이터만 씁니다(통신비밀보호법).
- **사람을 판정하지 않습니다.** 알림은 언제나 "확인 필요"이고, 위치는 신뢰도와 근거가 붙은 추정입니다. 좌석이나 수험생을
  지목하지 않으며, 최종 확인은 감독관이 합니다.
- **데이터를 밖으로 보내지 않습니다.** 관측은 이 노트북(건물 단위 구성이면 시험장 내부망) 안에만 있고 세션이 끝나면 버립니다.
  운영체제 알림에도 기기 주소·와이파이 이름 같은 식별자를 넣지 않습니다.

실제 시험장에 쓰려면 주관 기관의 승인과 법률 검토(전파법, 통신비밀보호법, 개인정보 보호)가 필요합니다. 전체 규칙은
[`AGENTS.md`](AGENTS.md) §3에 있습니다.

## 개발

**준비물** — [rustup](https://rustup.rs)(툴체인은 [`rust-toolchain.toml`](rust-toolchain.toml)대로 자동으로 맞춰집니다),
Node.js 22와 npm. macOS 또는 Windows 11에서 실행하고, 리눅스는 개발·검증용입니다.

```bash
cargo build -p tacet               # 감지·판정 백엔드(Rust)
cd desktop && npm ci               # 화면(Electron), 처음 한 번
npm start                          # 감독관 화면: 처음 설정 → 교실 보정 → 감지
npm run demo                       # 장비 없이 시연 모델로 화면만 본다
```

```bash
./scripts/verify.sh                # 전체 게이트: 형식·린트·라이선스·테스트·커버리지(85%)·수동성·언어 검사
./scripts/package_macos.sh         # macOS 유니버설 .dmg (임시 서명, 배포 서명·공증은 docs/platform/packaging.md)
./scripts/package_windows.sh       # Windows 설치 프로그램 .exe와 .zip (Git Bash에서)
```

선택 기능: `--features sdr`(HackRF·SoapySDR로 셀룰러 업링크 에너지). 개발 도구 `tacet-devtools`는 관측을 녹화하고(`record`)
다시 돌려 판정을 분해합니다(`replay`).

### 브랜치와 릴리스

`develop`과 `main`은 PR로만 바뀌고 CI가 초록이어야 합니다. CI는 같은 `verify.sh`를 Windows · macOS · Linux에서 돌립니다.

```
<접두사>/<요약>  ──PR──▶  develop  ──릴리스 PR──▶  main  ──태그 vX.Y.Z──▶  GitHub 릴리스
```

PR 제목은 접두사(`ADD:` · `FIX:` · `REF:` · `DEL:` · `DOCS:` · `UPT:`)로 시작합니다. 릴리스는 `develop`에서 `main`으로 가는 PR이
`Cargo.toml`과 `desktop/package.json`의 버전을 함께 올리고, `main`에 `vX.Y.Z` 태그를 달면
[`release.yml`](.github/workflows/release.yml)이 버전 확인 → 테스트 → Windows·macOS 빌드 → GitHub 릴리스(노트 자동 생성)를 합니다.

### 저장소 구조

```
crates/core/                 tacet-core                 판정: 기준선·규칙·점수·위치 판정·쉬운 말 설명. IO 없음
crates/collector-ble/        tacet-collector-ble        블루투스 광고 수집 (WinRT / CoreBluetooth)
crates/collector-wifi/       tacet-collector-wifi       와이파이 비콘 수집 (WLAN API / CoreWLAN)
crates/collector-sdr/        tacet-collector-sdr        셀룰러 업링크 에너지 (선택 기능)
crates/net/                  tacet-net                  센서↔서버 프로토콜, TLS, 페어링
crates/dashboard/            tacet-dashboard            시험본부 웹 대시보드
crates/app/                  tacet                      화면 없는 백엔드: 모드 배선, 권한, 교실 보정
crates/devtools/             tacet-devtools             개발 도구: 녹화 재생
desktop/                     Electron 화면: 처음 설정, 교실 보정, 감독관 화면, 운영체제 알림
docs/                        설계, 탐지 규칙·플랫폼 노트, 로고
scripts/                     verify.sh, 설치 파일 만들기, 에이전트 훅
AGENTS.md                    사람과 코딩 에이전트 공통의 기여 규칙
```

## 문서

| 문서 | 내용 |
|---|---|
| [`docs/DESIGN.md`](docs/DESIGN.md) | 배경, 위협 모델, 탐지 규칙, 다교실 위치 판정, 플랫폼 제약, 로드맵 |
| [`docs/rules/`](docs/rules/) | 규칙마다 막는 위협, 조건, 근거 출처, 오탐 요인, 미검증 항목 |
| [`docs/platform/`](docs/platform/) | 수집기별 권한·송신 여부·라이선스, 설치 파일 만들기 |
| [`docs/STATUS.md`](docs/STATUS.md) | 결정된 것과 열린 결정 |
| [`docs/brand/`](docs/brand/README.md) | 로고 파일과 사용 안내 |
| [`AGENTS.md`](AGENTS.md) | 아키텍처 규칙(§2), 수동 감지·법·개인정보 불변식(§3), 테스트 규칙(§5) |
| [`decision` 라벨 이슈](https://github.com/devgyurak/tacet/issues?q=label%3Adecision) | 설계 결정과 그 맥락·대안 |

## 기여

먼저 [`AGENTS.md`](AGENTS.md)를 읽어 주세요. 아키텍처 규칙, 수동 감지·개인정보 규칙, 테스트 규칙이 있고, PR 전에
`./scripts/verify.sh`가 통과해야 합니다. 문서·주석·커밋은 한국어로 쓰고, 테스트는 합성 데이터만 씁니다(실제 캡처·기기 주소는
저장소에 들이지 않습니다). PR은 `develop`으로 보냅니다. PR을 보내면 기여한 코드를 프로젝트 소유자가 이 저장소의
라이선스와 이후에 정하는 라이선스로 배포하는 데 동의하는 것으로 봅니다(PR 템플릿에 있습니다).

## 라이선스

[PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0/)에 **공식 배포본 추가 허가**를 더했습니다
([`LICENSE`](LICENSE)).

- **공식 배포본**(GitHub 릴리스 파일, 또는 릴리스 태그를 고치지 않고 빌드한 것)은 학교·공공기관은 물론 학원·민간 시험 업체도
  상업 목적을 포함해 자유롭게 쓸 수 있고, 무료로 재배포할 수 있습니다(판매는 안 됩니다).
- **수정한 판**(포크·바꾼 소스와 그 빌드)은 비상업 목적으로만 쓰고 배포할 수 있으며, Tacet 이름과 로고를 쓸 수 없습니다.

OSI 오픈소스 라이선스가 아닙니다. 이 요약보다 [`LICENSE`](LICENSE) 원문이 앞섭니다.
