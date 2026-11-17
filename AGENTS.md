# AGENTS.md — Tacet (시험장 무선 기기 탐지기)

시험 중 교실에 나타난 **폰과 스마트 안경의 무선 흔적**(BLE 광고, Wi-Fi 비콘, 셀룰러 업링크
에너지)을 **수동적으로** 감지해 감독관에게 "확인 필요"를 알리는 **Windows / macOS
데스크톱 앱**이다(판정·수집은 Rust 백엔드, 화면은 Electron). 수거·전원 차단이 끝난 교실에서 이런 흔적이 보이는 것 자체가
이상 신호다. 이 도구는 **송신하지 않고, 통신 내용을 해석하지 않는다.**

이 파일은 **모든 코딩 에이전트의 단일 정본**이다. Claude Code와 Codex가 각자의 진입점에서 여기로
모인다(`.agents/README.md`). 설계 문서는 `docs/DESIGN.md`이고 아래에서 "설계 §N"으로 가리킨다. 현재
단계와 열린 결정은 `docs/STATUS.md`에 있다.

---

## 1. 명령

```bash
./scripts/verify.sh                                         # 전체 게이트를 순서대로 — CI도 같은 스크립트를 돌린다
cargo fmt --all                                             # 포맷
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo deny --locked check licenses bans sources             # 의존성 라이선스 (deny.toml)
cargo llvm-cov nextest --workspace --all-features --locked --fail-under-lines 85   # 테스트 + 커버리지 하한
python3 scripts/check_agent_config.py                       # 에이전트 설정 무결성
python3 scripts/agent_hooks/selftest.py                     # 훅 자체 테스트
python3 scripts/agent_hooks/guard_passive.py --scan-tracked # 캡처·녹화·서명 키·디코더 디렉터리 추적 여부
python3 scripts/agent_hooks/check_architecture.py --all     # core 순수성 · 크레이트 그래프 · 이름
python3 scripts/agent_hooks/check_passive.py --all          # 송신·디코딩 코드 · 실제 MAC · 단정 문구
python3 scripts/agent_hooks/check_tests.py --all            # GWT 이름 · 제어 흐름 없음 · 단언
python3 scripts/agent_hooks/check_language.py --all         # 한국어 전용
```

**작업을 끝났다고 말하기 전에 `./scripts/verify.sh`가 통과해야 한다.** 실패하면 끝난 것이 아니다.
Phase 0에서 Cargo 작업 공간이 생기기 전까지 verify.sh는 Python 게이트만 돌리고 cargo 게이트는
건너뛰었다고 분명히 출력한다. 그것을 통과했다고 보고하지 않는다.

실패를 `#[allow(...)]`, `#[ignore]`, `--no-verify`, 게이트 수정으로 우회하지 않는다. 린트 예외가 정말
맞으면 `#[expect(clippy::<lint>, reason = "<이유>")]`로 이유를 항목에 붙인다. 건너뛰는 테스트는
`#[ignore = "<이유>"]`를 단다.

훅 스크립트는 Python 표준 라이브러리만 쓰고 Python 3.9(macOS의 `/usr/bin/python3`)에서 돌아야 한다.
`scripts/check_agent_config.py`는 `tomllib` 때문에 3.11 이상이 필요하다.

---

## 2. 아키텍처 — 절대 어기지 않는다

작업 공간은 설계 §5.3을 따른다. 프로젝트 이름은 **Tacet**(악보의 "쉼" — 시험 중에는 기기가 조용해야 한다)이고 패키지 접두사는 `tacet`다. `core`라는
패키지 이름은 Rust 내장 `core` 크레이트와 부딪치므로 디렉터리는 `crates/core`, 패키지는 `tacet-core`다.
앱 패키지는 그냥 `tacet`다. 접두사는 `scripts/agent_hooks/check_architecture.py`의 `PREFIX` 한 곳에만 있다.

```
crates/core/                tacet-core                타입·기준선·기기 상관·규칙 엔진·점수·위치 판정. IO 없음.
crates/collector-ble/       tacet-collector-ble       btleplug (Windows: WinRT, macOS: CoreBluetooth)
crates/collector-wifi/      tacet-collector-wifi      Win32 WLAN API / CoreWLAN (objc2)
crates/collector-sdr/       tacet-collector-sdr       feature "sdr": libhackrf 또는 SoapySDR + rustfft, 대역 전력만
crates/net/                 tacet-net                 센서↔서버 프로토콜, mDNS, TLS, 페어링
crates/dashboard/           tacet-dashboard           axum 웹 대시보드 (서버 모드)
crates/app/                 tacet                     화면 없는 백엔드: 모드 배선, 권한 요청, 절전 방지, 화면과의 표준 입출력 JSON
crates/devtools/            tacet-devtools            개발 도구(macOS/리눅스): 관측 녹화·재생
tests/                      통합 테스트, 합성 시나리오
docs/rules/                 탐지 규칙 노트 (규칙마다 하나)
docs/platform/              수집기·플랫폼 노트
docs/field/                 실측·교실 보정 기록 (요약 수치만)
desktop/                    Electron 화면: 처음 설정(교실·자리), 감독관 화면, 개발자 모드. 웹 대시보드도 같은 화면을 쓴다
```

의존은 **core 쪽으로만** 향한다.

```
collector-ble ───────┐
collector-wifi ──────┤
collector-sdr ───────┼──▶ core     core는 작업 공간 크레이트에 의존하지 않는다
net ─────────────────┤             수집기·net·dashboard는 서로 모르고 core만 안다
dashboard ───────────┘
app ──────▶ 전부(devtools 제외)     아무것도 app(`tacet`)과 devtools에 의존하지 않는다
devtools ─▶ 전부(app 제외)
```

### 실행 모드 (설계 §5.0)

**감독관 화면(기본)은 이 노트북 혼자 감지하는 단독 모드다.** 처음 켜면 교실 이름과 교실 안 노트북 자리(칠판을
남쪽에 둔 5×5 그림에서 고름)를 묻고, 그것으로 단독 모드 설정을 만든 뒤 곧바로 교실 보정으로 넘어간다(보정을 그만두면
보정 없이 감지를 시작한다). 고른 칸은 안내 문구의 "어디"(`SensorSpec.place`)와 보정 자리 추천에만 쓰고 판정에는 쓰지 않는다. 서버를 쓰는 건물 단위 구성(**센서 · 서버 · 본부
화면 보기**)은 개발자 모드(Ctrl/⌘+Shift+D)에서 설정 파일을 골라 쓰는 개발자용이다. 모드는 `app`이 배선만 바꾼다:
센서는 수집기 → `net`(보내기), 서버는 `net`(받기) → `core` → `dashboard`, 단독은 수집기 → `core` → 화면.
**판정은 언제나 core에서 한다.** 센서는 판정하지 않고, 서버와 단독 모드가 같은 core 코드로 판정한다.

화면은 `desktop/`의 Electron이다. Electron이 `tacet` 바이너리를 자식 프로세스로 띄우고 표준 입출력으로 JSON 줄을
주고받는다(`crates/app/src/ipc/`) — 네트워크 포트를 열지 않는다. 감독관이 읽는 문구는 core의 `explain`이 쉬운 말로
만든다(무엇이 보였나 · 어디 · 얼마나 확실한가 · 할 일, 문제 안내는 어디서 · 무엇이 · 영향 · 할 일). 기술 용어(규칙 ID,
dBm, 식별자)는 개발자 모드에서만 보인다. 렌더러는 Node에 닿지 않는다(contextIsolation, sandbox, CSP).

**창을 닫아도 감지는 계속된다.** 감지가 도는 동안 창 닫기는 창을 숨기기만 하고, 끄는 것은 메뉴 막대(Windows는 작업 표시줄
알림 영역) 아이콘의 '종료'나 Cmd+Q다. 창이 안 보이는 동안 새 알림과 감지 문제(무게 '문제')는 운영체제 알림으로 알린다
(`desktop/src/notify.js`). **운영체제 알림에는 식별자를 넣지 않는다** — 알림 센터가 기록을 남기므로 core가 만든 쉬운 말
(확인 필요 · 어디 · 무엇 · 의심 정도)만 쓴다(§3.3).

### 지켜야 할 규칙

1. **core는 순수하다.** 입력은 관측값(`Observation`, 서버에서는 `SensorReport`), 기준선, 설정(교실 보정값·
   가중치·화이트리스트)이고 출력은 규칙 적중(`Hit`), 의심 점수, 위치 판정(`Localization`), 알림 후보뿐이다.
   `std::fs` / `io` / `net` / `process` / `thread` / `env`, 시계, `print!` / `dbg!`, `async`, 난수, OS별 `cfg`가
   없다. IO와 비동기는 수집기·net·app의 몫이다.
   **core 금지 크레이트:** `anyhow` · `async-trait` · `axum` · `bluer` · `btleplug` · `chrono` · `eframe` · `egui` · `getrandom` · `libc` · `mdns-sd` · `nix` · `objc2` · `objc2-core-location` · `objc2-core-wlan` · `rand` · `rusb` · `rustfft` · `rustls` · `soapysdr` · `tauri` · `time` · `tokio` · `tokio-tungstenite` · `windows` · `windows-sys`.

2. **시각은 값으로 받는다.** core는 시계를 읽지 않는다. 관측값은 세션 시작 기준 밀리초 `Timestamp(u64)`를
   달고 들어온다. 단독 모드는 수집기가 단조 시계로 찍고, 센서 모드는 **NTP로 맞춘 시계**로 서버가 알린
   세션 시작 기준 시각을 찍는다(설계 §5.1). 설계 §5.4 스케치의 `Instant`는 이것으로 바꾼다(결정됨).

3. **완전한 결정성.** 같은 기준선·보정값과 같은 관측 열이면 적중, 점수, 위치 판정, 알림 순서가 언제나 같다.
   컬렉션은 `Vec`이나 `BTreeMap`을 쓴다. `HashMap` / `HashSet`은 프로세스마다 순회 순서가 달라서 결과가
   달라진다. 동점은 (타임스탬프, 센서 ID, 수집 순번)으로 명시적으로 정렬한다. **가중치와 점수는 정수다**
   (설계 §4.2의 낮음~매우 높음은 설정에서 정수로 매긴다). RSSI는 정수 dBm이고 중앙값도 정수로 정한 규칙
   (짝수 개면 아래쪽)으로 구한다. SDR 전력만 `f32`이고 비교는 명시한 기준으로 한다.

4. **고정 RSSI 임계치를 쓰지 않는다.** "교실 안" 경계는 교실별 보정값(설계 §4.4)으로, 판정은 기준선 대비
   상대 변화로 한다. 코드에 `-70` 같은 절대 dBm 판정 상수를 두지 않는다 — 보정값이 없으면 판정을 미루고
   "보정 필요" 상태를 보고한다. 약한 신호를 "멀리 있음"으로 해석하지 않는다(인체 차폐, 설계 §4.4).

5. **수집기는 번역만 한다.** OS·칩·USB 장비가 준 데이터를 `Observation`으로 바꾸는 것까지다. 기준선, 보정,
   점수, 화이트리스트는 모른다. 수집기 하나가 실패해도(어댑터 없음, 권한 없음, 드라이버 미바인딩) 다른
   수집기는 계속 돈다. 실패는 상태값으로 보고되어 감독관 화면과 점검 모드에 "Wi-Fi 수집 꺼짐 — 정밀 위치
   권한 없음"처럼 보인다. **조용히 꺼진 수집기는 탐지 공백**이고, 감독관은 그것을 알아야 한다.

6. **플랫폼 분기는 수집기의 `sys/` 모듈과 app에만 둔다.** `#[cfg(target_os = …)]`는 수집기, app(권한 온보딩,
   절전 방지), devtools에만 있다. core·net·dashboard에는 없다. 원시 데이터 → `Observation` 변환(BSS 항목,
   광고 AD 구조, FFT 빈 → 대역 전력)은 **순수 함수**로 분리해서 모든 OS에서
   합성 입력으로 테스트한다. FFI 호출은 얇은 어댑터다.

7. **플랫폼 차이를 타입에 드러낸다.** macOS BLE는 주소 대신 호스트마다 다른 UUID(`BleId::HostLocal`)를 준다.
   이것을 다른 센서의 관측과 같은 기기로 묶지 않는다 — "같은 시간 창 + 같은 제조사 데이터"로만 묶고
   위치 판정 신뢰도를 한 단계 낮춘다(설계 §7.2). 플랫폼이 못 주는 정보를 다른 값으로 메우지 않는다.

8. **선택 기능은 기본으로 꺼져 있다.** `sdr` feature가 없으면 libhackrf·SoapySDR 없이 빌드된다. CI는 기본 빌드와 `--all-features` 빌드를 모두 돌린다.

9. **`unsafe`는 FFI에만 쓰고 `// SAFETY:` 주석을 단다.** 주석은 왜 건전한지 논증한다(핸들 수명, 버퍼 길이,
   콜백 스레드, 해제 책임). 작업 공간 린트가 `unsafe_code`를 막고, 필요한 곳은 `#[expect(unsafe_code, reason = "…")]`로 연다.

10. **외부 입력은 패닉을 일으키지 않는다.** 광고 패킷, 비콘 IE, 센서 보고서는 남이 만든
    바이트다. 길이와 필드를 검사하고 맞지 않으면 버린다(`proptest`로 임의 바이트를 넣어 확인). 서버는 센서
    보고서를 신뢰하지 않는다 — 페어링된 센서만 받고, 형식이 틀린 보고서는 버리고 기록한다. 오류는
    라이브러리에서 `thiserror`, app·devtools에서만 `anyhow`.

11. **UI는 판단하지 않는다.** 데스크톱 화면(Electron), 웹 대시보드는 core가 정한 의심 강도·위치·신뢰도·근거를
    그릴 뿐이다. 임계치나 판정 로직이 UI·프런트엔드 코드에 생기면 테스트를 우회한다.

12. **세션 데이터는 메모리에만 있다.** MAC, BLE UUID, 광고 데이터는 세션이 끝나면 폐기한다
    (설계 §8). 보고서로 남겨야 하면 세션마다 새로 만든 솔트로 해시한 식별자만 남긴다. 솔트는 난수이므로
    app에서 만들어 core에 값으로 넘긴다. 센서는 서버로 보낸 뒤 원본을 쌓아 두지 않는다.

`check_architecture.py`(편집 훅과 CI)가 규칙 1, 6(core·net·dashboard의 OS 분기), 9와 크레이트 그래프·이름을
기계적으로 강제한다. 정당한 예외는 그 줄이나 바로 윗줄에 `// PURITY-OK: <이유>`를 단다. 리뷰어가 그 이유에
동의해야 하며 "그게 쉬웠다"는 이유가 아니다.

**플랫폼** (결정됨): **Windows 11과 macOS가 배포 대상**이다. 리눅스는 개발·검증용(devtools,
CI)으로만 쓰고 리눅스 전용 수집기 경로는 만들지 않는다. Windows 10 지원은 열린 결정이다. OS마다 되는 것이
다르므로(설계 §7) 지원하지 못하는 기능은 숨기지 말고 상태로 보고한다(규칙 5).

---

## 3. 수동 감지·법·개인정보 불변식 — 가장 위험한 부분

여기서 실수하면 되돌릴 수 없다. 전파를 한 번 쏘면 그 자체로 전파법 위반이고, 남의 주소가 든 캡처나
라이선스가 맞지 않는 코드가 git 기록에 들어가면 모든 클론과 포크에 남는다. 강제 푸시로도 회수되지 않는다. 이 규칙은
다른 모든 고려보다 앞선다(설계 §2 비목표, §8).

### 3.1 송신하지 않는다 — 전파법

- 재밍, 프레임 주입, 디어스, 가짜 비콘·AP, BLE 광고, BLE 연결·GATT 조회, Wi-Fi 접속·호스팅, SDR 송신
  (`hackrf_start_tx` 등) 코드를 쓰지 않는다. 장비 펌웨어에 송신 기능이 있어도 그 명령을 보내지 않는다. `check_passive.py`가 대표 API를 막는다.
  HCI·USB 원시 명령을 직접 보내는 코드도 쓰지 않는다(대표 API 검사로는 잡히지 않으니 리뷰가 본다). USB 무선 장비 수집기를 새로
  들이려면 먼저 `decision` 이슈를 거친다(블루투스 연결 탐지는 다루지 않기로 했다 — `docs/STATUS.md`).
- 에이전트는 송신 도구를 실행하지 않는다. `guard_passive.py`가 막고 권한 정책도 거부한다.
  **송신 도구:** `aireplay-ng` · `airbase-ng` · `mdk3` · `mdk4` · `wifiphisher` · `hostapd` · `create_ap` · `rpitx` · `sendiq`.
  인자에 따라 송신이 되는 명령(`hackrf_transfer -t`, `bluetoothctl advertise`, `nmcli … hotspot` 등)도 막는다.
- **허용하는 예외 두 가지** (설계 §7.2, §8):
  - **macOS BLE 능동 스캔** — CoreBluetooth는 수동 스캔을 지원하지 않아 스캔 요청을 보낸다. 일반 블루투스
    기기의 정상 동작으로 본다. **Windows는 수동(passive) 스캔을 쓴다.**
  - **OS Wi-Fi 스캔**(`WlanScan`, CoreWLAN `scanForNetworks`)이 내보내는 probe request.
  우리 코드가 프레임이나 스캔 요청을 직접 만들어 보내는 것은 예외가 아니다. 두 예외 모두 법률 검토 대상이다.
- 센서↔서버 통신은 시험장 내부망 통신이고 탐지용 송신이 아니다. **유선 LAN을 권장**하고, 무선이면 그 AP를
  기준선 화이트리스트에 넣는다(설계 §5.1).
- 시험 시나리오를 만들려고 핫스팟을 켜거나 광고를 쏘지 않는다. 시나리오는 **합성 관측값**으로 만든다.
  실측은 사람이 자기 기기로 하고(`field-test` 스킬), 에이전트는 그 결과 요약만 다룬다.

### 3.2 내용을 해석하지 않는다 — 통신비밀보호법

- 쓰는 것: BLE 광고 메타데이터(주소·UUID, RSSI, 제조사 ID와 데이터, 서비스 UUID), Wi-Fi 비콘 헤더(BSSID,
  SSID, 채널, RSSI), SDR 대역별 에너지. 블루투스 연결 패킷은 다루지 않는다(사용자 결정 2026-11-16).
- BLE 광고의 **기기 이름(Local Name)**은 교실 보정에서 감독관이 자기 휴대폰을 고르는 목록에만 쓴다(사용자 결정
  2026-11-16). 판정·알림·보고서·로그에 넣지 않고 저장하지 않는다. 남이 보낸 문자열이라 정리해서(제어 문자 제거, 40자)
  쓴다.
- 쓰지 않는 것: 패킷 본문, BR/EDR·BLE 연결 페이로드 복원, LTE/5G 복조·MIB/SIB·DCI 디코딩, IMSI·TMSI·GUTI·RNTI 같은 식별자. SSB 동기화로 n78 UL 슬롯만
  재는 방안(설계 §4.5 (A))은 셀 서치가 필요하므로 착수 전에 법률 검토와 `decision` 이슈를 거친다.
- 모니터 모드 수집기는 없다(설계 §5.2). 데이터 프레임과 probe request 내용은 애초에 다루지 않는다.
- 에이전트는 디코더 프로젝트의 소스를 열거나 받거나 설치하지 않는다.
  **디코더 프로젝트:** `LTESniffer` · `srsRAN` · `srsLTE` · `IMSI-catcher` · `gr-gsm` · `OpenBTS` · `LTE-Cell-Scanner`.

### 3.3 남의 식별자는 저장소에 오지 않는다 — 개인정보

- 실제 무선 캡처, IQ 녹음, Kismet DB, devtools의 관측 녹화는 추적하지 않고, 그 바이트를 대화로 읽지도 않는다
  (Read 도구, `xxd`, `tshark -r` 모두). 우리 바이너리로 재생하는 것은 된다.
  **캡처 확장자:** `.pcap` · `.pcapng` · `.cap` · `.btsnoop` · `.iq` · `.cfile` · `.cu8` · `.cs8` · `.cs16` · `.cf32` · `.sigmf-data` · `.sigmf-meta` · `.kismet` · `.kismet-journal` · `.obslog`.
  합성 데이터도 파일로 커밋하지 않는다. 테스트 데이터는 테스트 코드가 사양대로 조립한다.
- 문서·테스트·주석·커밋의 식별자는 **합성 값만** 쓴다.
  - MAC·BSSID: `00:00:5E:00:53:xx`(RFC 7042 문서용), 랜덤 MAC 예시는 `02:00:5E:00:53:xx`. multicast 예시는
    `01:00:5E:90:10:xx`(RFC 7042), 그 랜덤 예시는 `03:00:5E:90:10:xx`.
  - macOS BLE UUID: `Uuid::from_u128(작은 정수)`.
  실측에서 본 주소, SSID, 기기 이름은 어디에도 적지 않는다. `guard_passive.py`가 쓰기 전에, `check_passive.py`가
  CI에서 MAC을 막는다.
- **관측 데이터는 시험장 내부망을 벗어나지 않는다.** 클라우드 전송, 원격 텔레메트리, 데이터가 담긴 크래시
  리포트, 외부 분석 서비스를 넣지 않는다. 웹 대시보드는 내부망에만 열고 인터넷에 노출하지 않는다.
- 실행 중에는 §2 규칙 12를 따른다. 로그에는 원본 식별자를 찍지 않는다(디버그 빌드도 마찬가지).
- `docs/field/`의 실측·보정 기록은 요약 수치(RSSI 분포, 거리, 벽 재질, 기종 분류)만 남긴다.

### 3.4 알림은 판정이 아니다

- 알림 문구는 언제나 **"확인 필요"**다. "부정행위", "적발", "cheating detected" 같은 단정은 쓰지 않는다.
  `check_passive.py`가 Rust 문자열과 앱·대시보드 프런트엔드 파일을 본다.
- 위치는 "추정 위치"와 **신뢰도**(높음·중간·낮음), **근거**(센서별 RSSI 중앙값)와 함께 보여 준다(설계 §4.7).
  좌석 단위로 특정하지 않는다 — "교실 안이냐 밖이냐", 교실 앞뒤 정도가 한계다(설계 §4.4). 신뢰도가 낮으면
  인접 교실 감독관에게도 알린다.
- 특정 수험생을 지목하지 않는다. 최종 확인은 사람이 한다(휴대용 추적 포함).
- 화이트리스트(감독관 장비, 사전 신고한 의료기기·보청기의 주소)에 있는 기기는 알림을 내지 않고 상세도
  보여 주지 않는다. 의료기기 화이트리스트를 우회하는 기능은 만들지 않는다 — 수험생 불이익을 막는 장치다.

### 3.5 범위를 넓히지 않는다

- 세션을 넘어 기기나 사람을 추적하거나, 개인 기기 지문 DB를 만들거나, 시험 밖에서 쓰는 감시 기능을 만들지 않는다.
- 블루투스조차 쓰지 않는 완전 오프라인 기기는 범위 밖이다(설계 §2). 그것을 잡겠다고 능동 기법을 들이지 않는다.

### 3.6 근거 있는 규칙만 — 추측 금지

- 모든 탐지 규칙과 위치 판정 방식은 `docs/rules/<규칙ID>-<요약>.md`(템플릿 `docs/rules/README.md`)에서
  출발한다: 막는 위협(T1–T7), 조건, 가중치, 근거 출처, 오탐 요인, 미검증 항목.
- 제조사 ID와 서비스 UUID는 Bluetooth SIG Assigned Numbers와 설계 §11의 출처에서, 채널·대역은 IEEE 802.11과
  3GPP 표에서 가져와 출처를 적는다. 기억으로 쓰지 않는다.
- 실측으로 확인하지 못한 값(교실 보정, 위치 신뢰도 기준, 안경의 광고 특성)은
  노트의 "미검증"에 적고 코드에 `TODO(field-unverified)`를 남긴다. 확인된 것처럼 구현하지 않는다. 설계가
  "가설"이나 "자체 추정"이라고 적은 것은 미검증이다.

### 3.7 라이선스와 의존성

- 프로젝트 라이선스는 **PolyForm Noncommercial 1.0.0 + "추가 허가 — 공식 배포본"**이다(`LICENSE`, 사용자 결정 2026-11-16).
  공식 배포본은 상업 목적을 포함해 누구나 쓰고, 수정한 판은 비상업 목적으로만 쓴다. OSI 오픈소스가 아니다. 의존성은 이
  조건으로 배포할 수 있는 허용형만 들인다(copyleft 금지). 파일마다 SPDX 머리글을 달지는 않는다.
- 외부 기여는 PR 템플릿의 기여 동의(소유자가 이 라이선스와 이후 라이선스로 배포할 수 있다)를 받는다.
- `-sys` 크레이트가 묶어 오는 C 라이브러리(libhackrf, SoapySDR 등)의 라이선스는 `cargo deny`가 보지 못하므로
  리뷰가 확인한다.
- Rust 의존성은 허용형 라이선스만 쓴다. 허용 목록은 `deny.toml`이고 `cargo deny`가
  `./scripts/verify.sh`에서 검사한다. 목록을 넓히는 것은 `decision` 이슈다.
- 제3자 코드를 라이선스와 출처 없이 붙여넣지 않는다.

---

## 4. Rust 관례

- **Rust stable, edition 2024**, `rust-toolchain.toml`에 **정확한 버전으로 고정**(`channel = "1.NN.0"`,
  `rustfmt`·`clippy`·`llvm-tools-preview` 포함). 고정 버전 올리기는 별도 `UPT:` PR이다. `Cargo.lock`은 커밋한다.
  verify.sh는 고정이 없거나 떠 있으면 실패한다.
- `cargo fmt`와 `cargo clippy -D warnings`가 늘 깨끗하다.
- **상수에 이름을 붙인다.** 제조사 ID(`0x004C`, `0x01AB` …), 서비스 UUID(`0xFD5F`, `0xFE2C`), 대역 경계,
  장비 USB 명령 번호는 이름 있는 상수로 두고, 문서 주석에 출처와 규칙·플랫폼 노트를 적는다.
- **주석은 이유를 설명한다.** 오탐을 줄이려고 일부러 넣은 예외(수거폰 보관 위치 감점, 건물 밖 감점, macOS
  UUID 신뢰도 하향)는 이유가 없으면 다음 사람이 "정리"해서 회귀를 만든다. 규칙 노트를 인용한다.
- 가시성은 기본 `pub(crate)`, `pub` 항목에는 문서 주석.
- **모듈 파일은 `mod.rs` 한 가지 방식.** `[workspace.lints.clippy]`에서 `self_named_module_files`를 막는다.
- 핫 패스 최적화(`#[inline]`, 사전 할당)는 측정이 있을 때만.
- **의존성 추가**는 먼저 묻는다(`cargo add`는 `ask` 권한). 라이선스(§3.7)와 core 금지 목록(§2 규칙 1)을 지킨다.
- **주석과 문서는 한국어**, 식별자는 영어(§9). 프런트엔드(TypeScript 등)도 같다.

---

## 5. 테스트 — 강제 규칙

### 5.1 core 변경에는 테스트가 따른다

`FIX:`에는 고치기 전에 실패하던 테스트가 따른다. 종류:

- **단위** — 모듈의 `tests.rs`(`#[cfg(test)] mod tests;`): 규칙 조건, 점수 합산, 기준선 비교, 기기 상관,
  위치 판정(중앙값·신뢰도), 보정값 적용, 파서. 커지면 `tests/mod.rs` + `tests/<주제>.rs`. 인라인
  `mod tests { … }`와 `tests_<주제>.rs` 형제 파일은 안 된다 — `check_tests.py`가 `crates/` 아래에서 강제한다.
- **속성** — `proptest`: 적중을 더하면 점수가 줄지 않는다, 화이트리스트 기기는 어떤 관측에서도 알림을 내지
  않는다, 파서는 임의 바이트에 패닉하지 않는다, 같은 시각의 관측 순서를 섞어도 결과가 같다, 한 센서의 RSSI만
  올리면 그 교실로 배정될 가능성이 줄지 않는다.
- **시나리오** — `tests/`의 통합 테스트: 위협 T1–T7마다 합성 관측 열을 만들어 기대 알림을 단언하고,
  오탐 시나리오(수거폰이 안 꺼짐, 옆 교실 기기의 교실 재배정, 건물 밖 기기, 사전 신고 CGM·보청기, 감독관
  장비, macOS UUID 상관)는 기대 결과를 단언한다. 다교실 시나리오는 센서 여러 대의 `SensorReport`로 만든다.
- **프로토콜** — `net`: 센서 보고서 직렬화 왕복, 페어링되지 않은 센서 거부, 잘못된 보고서 버림. TLS·mDNS는
  루프백에서만, 인증서는 테스트 코드가 생성한다.
- **하드웨어** — 실제 어댑터·HackRF·OS 권한이 필요한 테스트는 `#[ignore = "실제 장비 필요: <무엇>"]`이고
  CI에서 돌지 않는다. 로직은 이것에 기대지 않는다.

### 5.1a 화면(desktop/) 시험

화면의 순수 함수(처음 설정 → 설정 파일, 백엔드 출력 나누기, 보일 알림·안내 고르기)는 `desktop/test/`의 `node --test`로
시험하고 `./scripts/verify.sh`가 돌린다. 시험 이름은 한국어 문장으로 "조건 → 동작 → 관찰값"을 쓴다. 화면 코드에 판정이
생기지 않게 판정에 쓰는 값(무게, 알릴 교실)은 core가 보낸 것만 쓴다. 화면 모양은 `npx electron . --demo --screenshot
<경로>`로 창만 캡처해 확인한다(전체 화면 캡처는 하지 않는다).

### 5.2 이름은 Given–When–Then

모든 테스트 함수는 `given_<상태>_when_<동작>_then_<관찰값>`이다(식별자라 영어). 마지막 `_then_` 뒤는 관찰할 수
있는 것이다 — `then_r8_hits`, `then_room_is_3_2_with_medium_confidence`이지 `then_works`, `then_ok`가 아니다.

### 5.3 테스트 본문에 제어 흐름이 없다

`if` / `else` / `for` / `while` / `loop` / `match`(그래서 `let … else`도)와 `for_each`를 쓰지 않는다. 사례 표는
실패 목록을 돌려주는 하네스 함수로 돌리고, 테스트는 목록이 비었음을 단언한다. 실패한 사례가 첫 번째에서
멈추지 않고 한 번에 다 보인다.

```rust
// 나쁨 — 첫 실패에서 멈추고 나머지를 가린다
#[test]
fn given_glasses_vendor_ids_when_scored_then_all_hit_r1a() {
    for id in GLASSES_VENDOR_IDS {
        assert!(score(&adv_with_vendor(id)).contains(&RuleId::R1a));
    }
}

// 좋음 — 반복은 하네스에, 테스트는 결과를 한 번 단언한다
#[test]
fn given_each_glasses_vendor_id_when_scored_then_no_vendor_misses_r1a() {
    let misses = vendors_missing_rule(GLASSES_VENDOR_IDS, RuleId::R1a);
    assert_eq!(misses, Vec::<u16>::new());
}
```

### 5.4 모든 테스트는 단언한다

`assert*!`, `prop_assert*!`, insta 스냅숏, 이름이 `assert_*`인 도우미 — 아니면 `#[should_panic(expected = "...")]`.
코드만 돌리는 테스트는 커버리지만 올리고 아무것도 증명하지 않는다.

### 5.5 결정성은 가정하지 않고 테스트한다

같은 시나리오를 두 번 돌리면 보고서 바이트가 같다. 센서 보고서가 도착하는 순서를 바꿔도(같은 타임스탬프 안에서)
위치 판정이 같다. 기능이 생길 때마다 이 속성을 더한다. `proptest-regressions/`는 커밋한다.

### 5.6 스냅숏은 증거다

알림·위치 판정·보고서 스냅숏이 바뀌면 **PR에 이유를 적는다**: 어떤 규칙·가중치·보정 방식이 바뀌었고 왜 새
결과가 맞는지. 빨간 테스트를 초록으로 만들려고 `cargo insta accept`로 일괄 승인하지 않는다.

### 5.7 합성 데이터만, 외부 네트워크 없음

테스트는 합성 관측값과 사양대로 조립한 바이트만 쓴다. 실제 캡처, 실제 식별자, 외부 네트워크, 무선 하드웨어를
쓰지 않는다. 식별자는 §3.3의 합성 값만.

### 5.8 경계 동작

버그는 경계에 산다. 다룰 것: 보정 경계와 같은 RSSI, 보정값이 없는 교실, 세션 중 랜덤 MAC 교체, 기준선 창 끝
(정확히 N분에 나타난 기기), 숨긴 SSID의 두 표현(길이 0, NUL로 채움), `DIRECT-`
접두사 대소문자, locally administered·multicast 비트, 2.4GHz와 6GHz에서 겹치는 채널 번호, 제조사 데이터가 여러 개인
광고, 1·2위 교실 RSSI 차이가 신뢰도 경계와 같을 때, 센서 하나만
본 기기, 센서 간 시각 차이, macOS `HostLocal` UUID와 Windows 주소의 혼합, 같은 타임스탬프의 관측, 화이트리스트와
규칙 적중이 겹칠 때, 수집기·센서 재시작과 상태 보고.

### 5.9 커버리지 하한

**커버리지 하한:** `85%` 줄 커버리지(작업 공간 전체, `cargo-llvm-cov`). `./scripts/verify.sh`와 CI가
`--fail-under-lines 85`로 강제한다.

- 하한을 낮추거나 파일을 빼거나(`--ignore-filename-regex`, `#[coverage(off)]`) 코드만 돌리는 테스트로 올리지
  않는다. 동작을 단언하는 테스트로 올린다.
- OS FFI 어댑터와 앱 진입점(`main.rs`, Electron 메인 프로세스)은 얇게 두고(§2 규칙 6) 그 주변 로직을 순수 함수로 빼서 덮는다.
- 커버리지는 필요조건이지 충분조건이 아니다.

---

## 6. 탐지 작업과 단계 규율

- **탐지 규칙·위치 판정·보정 방식 추가·변경**은 `add-rule` 스킬을 따른다: 단계 확인 → 규칙 노트 → 실패하는
  합성 시나리오 → core 구현 → 오탐 검토 → verify → `decision` 이슈.
- **수집기·플랫폼 추가**는 `add-collector` 스킬: 단계 확인 → 플랫폼 노트(송신 여부, 권한, 라이선스) → 순수
  변환과 합성 테스트 → 얇은 OS·USB 어댑터 → 상태 보고 → 수동 실측 → verify.
- **오탐·미탐·위치 오판 조사**는 `triage-detection` 스킬: 익명화한 재현 → 점수·위치 분해로 첫 갈림점 찾기 →
  규칙 노트 대조 → 실패 테스트 → 수정.
- **실측과 교실 보정**(경계, 안경 특성)은 `field-test` 스킬: 사람이 자기 기기로, 송신 없이,
  요약만 기록.
- **현재 단계 안에서만 일한다**(`docs/STATUS.md`). 이후 단계의 기능은 스텁 + `TODO(phaseN)`이다(`phase2.5`처럼
  소수 단계도 그대로 쓴다). 단계의 완료 기준을 채워야 다음 단계를 시작한다.
- **설계 결정은 GitHub 이슈다**, 저장소 파일이 아니다. 되돌리기 비싸거나 누군가 나중에 "정리"하고 싶어질
  결정(크레이트 그래프, 점수·위치 판정 모델, 기본 가중치·신뢰도 기준, 보존 정책, 센서 프로토콜, 라이선스,
  `unsafe` 허용 범위)은 `.github/ISSUE_TEMPLATE/decision.md`로 `decision` 라벨 이슈를 만든다: 맥락, 결정, 결과,
  대안. 그것을 반영하는 PR은 이슈를 링크하고, 그 때문에 존재하는 코드는 인용한다(`// #42에서 결정.`). 에이전트는
  요청받았을 때만 이슈 초안을 쓴다(`gh issue create`는 `ask`). 결정과 종료는 사람이 한다.
- 표식: `TODO(phaseN)`(단계 N으로 미룸), `TODO(field-unverified)`(실측 미확인, §3.6), `PURITY-OK:`(§2 예외),
  `PASSIVE-OK:`(§3 검사 예외 — 리뷰어 동의 필수), `SAFETY:`(§2 규칙 9).

---

## 7. CI/CD, 배포, 에이전트 게이트

- **저장소** — `github.com/devgyurak/tacet`.
- **릴리스** — main에 병합된 커밋에 `vX.Y.Z` 태그를 달면 `.github/workflows/release.yml`이 태그·`Cargo.toml` 버전 일치를
  확인하고 테스트 → Windows·macOS 빌드 → GitHub 릴리스(PR 라벨로 노트 자동 생성)를 한다(`docs/platform/packaging.md`).
  태그는 사용자가 요청할 때만 단다.
- **브랜치 모델** — 작업 PR은 `develop`으로, `main`은 릴리스 PR(`develop` → `main`)만 받는다. 두 브랜치 모두
  삭제·강제 푸시 금지, 초록 CI 필수, PR 승인 1개. 직접 푸시는 막는다.
- **CI**(`.github/workflows/ci.yml`)는 `./scripts/verify.sh`를 Windows · macOS · Linux에서 돌린다. 리눅스는 배포
  대상이 아니지만 개발용 빌드와 devtools를 지킨다. Rust 툴체인과 시스템 라이브러리 단계는 `Cargo.toml`이 생기는
  순간 켜진다.
- **배포**(Phase 2.9) — Windows는 **Microsoft Store의 MSIX가 표준**(Microsoft 서명, 사용자 결정 2026-11-17)이고 Store를 쓸 수
  없는 PC용 설치 프로그램(`.exe`, Authenticode 서명 미완)을 함께 낸다. macOS `.dmg`(Developer ID 서명 + 공증 +
  Hardened Runtime). `scripts/package_macos.sh`·`scripts/package_windows.sh`가 Rust 백엔드를 빌드해 Electron 앱에 넣고
  electron-builder로 묶으며 서명 정보는 환경 변수로만 받는다(`docs/platform/packaging.md`). **서명 인증서·키·Apple 계정 정보는 저장소에 두지 않고 CI 비밀값으로만 쓴다.** 수동성 스캔이 추적된
  인증서 파일(`.p12`, `.pfx`, `.p8` …)을 잡는다. 시험 기간 중에는 버전을 고정한다: 앱에 자동 업데이트를 넣지 않고, Store
  앱은 Store가 업데이트를 관리해 게시자가 끌 수 없으므로 시험 기간에는 Store에 새 버전을 게시하지 않는다.
- **커버리지 하한은 CI 전용이 아니고 끄지 않는다.** `check_agent_config.py`는 §5.9와 `scripts/verify.sh`의 하한이
  다르면 실패한다.
- **에이전트 설정**(`.github/workflows/agents.yml`)은 `scripts/check_agent_config.py`와 훅 자체 테스트를 돌린다.
- **로컬 훅은 최선 노력이고 CI가 진짜 게이트다.** `Bash`/`sed`로 바꾼 파일이나 훅이 연결되지 않은 하네스는 편집
  훅을 건너뛴다. 중요한 규칙은 모두 저장소 전체 모드(`--all`, `--scan-tracked`)가 있고 verify.sh와 CI가 돌린다.
- **게이트마다 구현은 하나.** 판단 로직은 `scripts/agent_hooks/`에만 있다. `check_agent_config.py`는 훅의 목록을
  가져와 §2·§3이 같은 목록을 적는지 확인한다.
- **권한 정책은 하나.** 읽기 전용 검증(`cargo fmt|clippy|check|build|test|nextest|llvm-cov|tree|doc|deny`,
  `python3 scripts/…`, `./scripts/verify.sh`, `git status|diff|log|show|branch`)과 `git add|commit|push`는 허용 —
  그래도 커밋·푸시는 사용자가 요청할 때만 한다(§8). 의존성과 바깥으로 나가는 명령(`cargo add`, `cargo install`,
  `cargo update`, `gh issue create`, `gh pr create`)은 **묻고**, 파괴적 명령(`rm -rf`, 강제 푸시)과 송신 도구는
  **거부**한다. `.claude/settings.json`과 `.codex/rules/default.rules`에 같은 정책을 두고 `check_agent_config.py`가
  둘이 같은지 확인한다.

---

## 8. 작업 흐름

1. **바꾸기 전에 읽는다.** 오탐 방지 예외와 플랫폼 우회에는 이유가 있다. 규칙·플랫폼 노트가 그 이유를 적는다.
2. **요청한 범위와 현재 단계만** 한다. 관계없는 코드를 "정리"하지 않는다.
3. core를 건드렸으면 **테스트로 증명하고** 어떤 테스트인지 말한다.
4. `./scripts/verify.sh`가 모두 초록.
5. 무엇을 바꾸고 무엇을 확인했는지 사실대로 보고한다. 실패하거나 건너뛴 것이 있으면 그렇다고 말한다.
6. **커밋과 푸시는 사용자가 요청할 때만.** 권한은 `git commit` / `git push`를 묻지 않는다. 사용자의 요청이 승인이다.

### 커밋 · PR · 브랜치 이름

커밋 제목, PR 제목, 브랜치는 **같은 접두사 하나**를 쓴다. 접두사는 영어, 요약은 한국어다.

```
<접두사>: <요약>
```

`ADD:` 새것 — 기능 · 탐지 규칙 · 수집기 · 실행 모드 · 테스트 · 워크플로 · 의존성 ·
`FIX:` 버그 수정(오탐·미탐·위치 오판 포함) · `REF:` 리팩터링(동작 그대로) · `DEL:` 제거 ·
`DOCS:` 문서만 · `UPT:` 의존성 버전 올리기

`ADD:`/`DEL:`은 **코드와 의존성**을 말한다. 문서, 규칙·플랫폼 노트, 실측·보정 기록, 에이전트 설정
(`.claude/` · `.agents/` · `.codex/`)은 언제나 `DOCS:`다. `scripts/`와 `.github/workflows/`는 코드다 — 실행된다.

**PR 하나 = 규칙 하나, 수집기 하나, 모드 하나, 또는 버그 하나.** 브랜치는 `<소문자 접두사>/<영어-케밥>`
(`add/r1a-glasses-vendor`, `fix/median-even-count`)이고 `develop`에서 딴다. 자세한 안내와 커밋 전 체크리스트는
`commit` 스킬(`.claude/skills/commit/SKILL.md`)에 있다. 커밋 전에 읽는다.

---

## 9. 문서와 언어 — 한국어만

- **이 저장소에 쓰는 글은 한국어다**: 문서, 주석, 문서 주석, 독스트링, 감독관에게 보이는 문구(앱·대시보드·
  권한 온보딩·`Info.plist` 사용 목적 문구), 커밋 메시지 요약과 본문, PR·이슈 본문, 스킬, 서브에이전트, 훅 메시지.
- **영어로 두는 것**: 식별자(타입·함수·변수·테스트 이름), 명령, 경로, 크레이트·API·표준 이름
  (BLE, RSSI, BSSID, btleplug, CoreWLAN), 커밋 접두사(`ADD:` 등), 브랜치 이름.
- **번역본은 만들지 않는다.** `README-EN.md` 같은 `-{언어코드}` 파일은 두지 않는다. 대상 독자는 한국 시험 운영자와
  개발자다.
- `check_language.py`(편집 훅과 CI)가 영어로만 쓴 문단·주석 묶음·사용자 문구(Rust와 프런트엔드)와 번역본 파일을
  잡는다. 문장이 좋은 한국어인지는 판단하지 못하므로 그것은 리뷰의 몫이다.

---

## 10. 하지 말 것

- 전파를 쏘는 코드나 명령 — 재밍, 주입, 디어스, 광고, 연결, 핫스팟, SDR 송신.
- 패킷 본문, 블루투스 연결 내용 복원, 셀룰러 제어 정보·식별자를 디코딩하는 것.
- 실제 캡처·녹화나 실제 MAC·SSID를 저장소·문서·커밋·대화에 들이는 것. 관측 데이터를 내부망 밖으로 보내는 것.
- 알림에서 부정행위를 단정하거나 특정인·좌석을 지목하는 것. 의료기기 화이트리스트를 우회하는 것.
- 고정 RSSI 임계치, 실측 없이 확정한 보정값·신뢰도 기준 — `TODO(field-unverified)`로 남긴다.
- 서명 인증서·키를 저장소에 넣는 것.
- core에 IO, 시계, async, 난수, OS 분기, 해시 컨테이너를 넣는 것. UI에 판정 로직을 넣는 것.
- 테스트 없이 core를 바꾸거나, 이유 없이 스냅숏을 바꾸는 것.
- 게이트를 낮추는 것(커버리지 하한 포함), 이유 없는 `#[allow]`, 초록을 만들려는 `#[ignore]`.
- 이후 단계의 기능을 지금 만드는 것.
- 게이트를 돌리지 않고 "고쳤다", "통과한다"고 보고하는 것.
