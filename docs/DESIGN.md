# 시험장 무선 기기 탐지기 (가칭) — 설계 문서

> 상태: 초안 · 언어: Rust · 배포 형태: **Windows / macOS 데스크톱 앱** · 최종 수정: 2026-11-16
>
> 에이전트와 문서는 이 파일을 "설계 §N"으로 인용한다. 구현 규칙의 정본은 `AGENTS.md`이고, 이 문서와
> 다르게 정한 것은 `docs/STATUS.md`의 "결정됨"에 이유와 함께 적는다. §4.7 알림 예시의 BSSID는 저장소 규칙
> (AGENTS.md §3.3)에 따라 합성 값으로 바꿔 적었다.

## 1. 배경

AI 안경(메타 레이밴 등)을 이용한 시험 부정행위가 국가자격시험·토익 등에서 연이어 적발되고 있고, 수능에서도 대응이 쟁점이 되었다. 현재 대응은 반입 금지와 감독관 육안 식별에 의존하고 있으며, 최신 안경은 외형만으로 구분하기 어렵다.

### 왜 "네트워크 차단"으로는 안 되는가

- AI 안경은 시험장 네트워크를 쓰지 않는다. **안경 ↔ 폰은 Bluetooth/Wi-Fi(핫스팟·Wi-Fi Direct)**, **폰 ↔ 외부는 셀룰러(LTE/5G)** 로 연결된다.
- 시험장 방화벽·포트 차단은 시험장 망에 붙은 기기에만 효과가 있다.
- 셀룰러 재밍·전파 차단은 법적 제약이 크고, 교실 단위 전파 차폐는 시공 비용과 평상시 학교 운영 문제로 현실성이 낮다.

### 핵심 관찰

외부(클라우드 AI·조력자)와 통신하려면 안경은 거의 반드시 **폰을 경유**해야 하고, 폰은 다음 흔적을 남긴다.

1. **BLE 광고 패킷**: 블루투스가 켜진 폰은 연동·기기 찾기 기능 때문에 광고를 송신한다(예: 아이폰 Continuity는 화면 잠금·해제, 통화, 화면 탭 같은 동작에 반응해 광고를 보낸다 [S5a]). 반면 안경 자체는 폰과 연결된 뒤 광고를 멈출 수 있다 [S4].
2. **Wi-Fi 비콘**: 핫스팟·Wi-Fi Direct는 SSID를 숨겨도 비콘을 송신한다.
3. **셀룰러 업링크**: 폰이 외부로 데이터를 보낼 때 반드시 송신한다.

시험 중에는 모든 폰이 수거·전원 차단되어야 하므로, **교실 안에서 이런 흔적이 나타나는 것 자체가 이상 신호**다. 이 도구는 이 흔적을 *수동적으로 감지*한다. 송신하지 않고 통신 내용도 해석하지 않는다.

## 2. 목표 / 비목표

### 목표

- 일반 노트북(내장 BT/Wi-Fi 칩)만으로 1차 스크리닝을 할 수 있을 것
- 시험 시작 직후의 **기준선(baseline)** 대비 새로 나타난 무선 기기를 감지할 것
- 감독관이 바로 이해할 수 있는 알림(의심 강도, 대략적 근접도)을 줄 것
- **Windows / macOS 데스크톱 앱**으로 배포한다 (설치형, 코드 서명). 기능 수준은 OS별로 다르다(§7). 리눅스는 개발·검증용으로만 쓴다.
- 선택 기능으로 SDR을 이용한 셀룰러 업링크 에너지 감지를 지원할 것

### 비목표

- 통신 내용·식별자 디코딩 (LTE 스니핑 등) — 법적 문제(통신비밀보호법) 및 불필요
- 전파 송신·재밍
- 특정 수험생 자리를 정밀하게 특정하는 것 (근접도는 참고용이며, 최종 확인은 사람이 한다)
- 송신하지 않는 완전 오프라인 기기 탐지
- 광고 없이 블루투스 연결만 유지하는 기기를 연결 패킷으로 잡는 것 — 전용 스니퍼 장비가 필요해 다루지 않는다(사용자 결정 2026-11-16, §4.6)

## 3. 위협 모델

| # | 시나리오 | 경로 | 남는 흔적 | 탐지 단계 |
|---|---|---|---|---|
| T1 | 안경 + 숨긴 폰 + 클라우드 AI | BT/Wi-Fi → 폰 → LTE | BLE 광고, 핫스팟 비콘, 업링크 | Phase 1 / 3 |
| T2 | 안경 + 숨긴 폰 + 외부 조력자 | 위와 동일 | 위와 동일 | Phase 1 / 3 |
| T3 | 숨긴 폰 핫스팟(SSID 숨김) | Wi-Fi | 빈 SSID 비콘 | Phase 1 |
| T4 | 6GHz 핫스팟 + 6GHz 지원 안경 | Wi-Fi 6E/7 | 6GHz 비콘 | Phase 4 (6E 칩 필요) |
| T5 | 폰 부가 기능을 꺼서 BLE 광고 최소화 | BT Classic 연결만 유지 | BT 연결 패킷, 업링크 | Phase 3 (업링크만). BT 연결 패킷은 범위 밖 (사용자 결정 2026-11-16) |
| T6 | 숨긴 폰에서 로컬 모델 실행 (외부 송신 없음) | BT만 | BT 연결 패킷 | 범위 밖 (사용자 결정 2026-11-16: 블루투스 연결 탐지를 다루지 않음) |
| T7 | 안경이 폰과 연결된 뒤 광고 중단 (레이밴 메타 2세대 등) [S4] | BT Classic / BLE 연결 | BT 연결 패킷 | 범위 밖 (사용자 결정 2026-11-16: 블루투스 연결 탐지를 다루지 않음). 같은 폰의 광고(R1)·Wi-Fi Direct(R6)는 Phase 1 규칙이 따로 보지만, 폰이 광고하거나 Wi-Fi Direct로 전송하는 구간에만 잡힌다 — 연결 뒤 폰이 조용하면 공백이다 |

참고 (2026-11 기준):

- 메타 레이밴 1·2세대, 오클리 메타, 레이밴 디스플레이는 **Wi-Fi 6 (2.4/5GHz)** 이다.
- 삼성 인텔리전트 아이웨어는 FCC 서류상 **Wi-Fi 6E/7** 을 지원하고 12월 출시 예정이다. 갤럭시 S21 울트라 이후 기종의 6GHz 핫스팟과 조합하면 T4가 현실화된다.

## 4. 탐지 전략

### 4.1 기준선 (Baseline)

1. 폰 수거와 전원 차단을 확인한 뒤, 시험 시작 직전부터 N분(기본 3분) 동안 관측한다.
2. 관측된 BLE 주소·Wi-Fi BSSID와 대역별 전력을 기준선으로 저장한다. 학교 AP, 옆 교실 기기, 감독관 장비가 여기에 포함된다.
3. 감독관 장비는 화이트리스트로 따로 등록한다.

### 4.2 탐지 규칙

| ID | 대상 | 조건 | 기본 가중치 |
|---|---|---|---|
| R1 | BLE | 제조사 ID가 폰 제조사(Apple `0x004C`, Samsung `0x0075`, Google `0x00E0`) | 높음 |
| R1a | BLE | 제조사 ID가 스마트 안경 계열(Meta `0x01AB`, Meta Platforms Technologies `0x058E`, Luxottica `0x0D53`, Snap `0x03C2`)이거나 서비스 UUID `0xFD5F`(Oculus) — **전원 켤 때와 페어링 구간에서만 잡히는 것으로 보고됨** | 매우 높음 |
| R1b | BLE | Fast Pair 서비스 데이터(`0xFE2C`) — 폰이 아니라 **액세서리(이어폰 등) 쪽 광고**임 | 낮음~중간 |
| R2 | BLE | 기준선에 없던 광고자의 RSSI가 **교실별 보정값** 이상 (고정 임계치 사용 금지, §4.4) | 중간 |
| R3 | Wi-Fi | 기준선에 없던 BSSID의 RSSI가 임계치 이상 | 중간 |
| R4 | Wi-Fi | BSSID의 locally administered 비트가 1 (랜덤 MAC, 폰 핫스팟 특성) | 높음 |
| R5 | Wi-Fi | SSID가 비어 있음 (숨긴 네트워크) | 높음 |
| R6 | Wi-Fi | SSID가 `DIRECT-` 로 시작함 (Wi-Fi Direct) | 높음 |
| R7 | RF (SDR) | LTE 업링크 대역 전력이 기준선 대비 +X dB 버스트 | 높음 |

- 규칙별 가중치를 합산해 **의심 점수**를 낸다. 점수가 임계치를 넘으면 알림을 보낸다.
- MAC 랜덤화 때문에 BLE 주소가 바뀌므로, 같은 제조사 데이터와 비슷한 RSSI가 연속되면 동일 기기로 묶는다(휴리스틱).
- R8·R9(블루투스 연결 탐지)는 뺐다(사용자 결정 2026-11-16, §4.6). 두 ID는 다시 쓰지 않는다.

### 4.3 오탐 처리

- **가장 흔한 원인**: 수거한 폰이 꺼지지 않은 경우. 보관 위치(교실 앞)를 기준선 단계에서 등록하고, 해당 방향·RSSI 대역에서 나오는 신호는 감점한다.
- 옆 교실이나 복도 기기는 버리지 않고 건물 서버에서 **어느 교실 기기인지 다시 배정**한다(§4.7). 건물 밖(운동장, 인접 건물)으로 판정되면 감점한다.
- 의료기기(인슐린 펌프·CGM 등 BLE 기기)는 사전 신고를 받아 화이트리스트에 등록한다. 수험생 불이익을 막기 위해 필요하다.
- 알림은 "부정행위 판정"이 아니라 **"확인 필요"** 로 표현한다.

### 4.4 RSSI 보정 원칙 (레퍼런스 검토 결과)

- 몸에 가려지면 RSSI가 크게 떨어진다. 뒷주머니에 넣은 경우가 앞주머니보다 약 15dB 낮았다는 측정 결과가 있다 [S6]. 숨긴 폰은 보통 몸에 붙어 있으므로 "약한 신호 = 멀리 있음"으로 해석하면 안 된다.
- 벽 하나에 1m 거리에서도 약 6dB가 떨어졌고, 8.5m에서는 패킷 손실이 50% 안팎까지 올라갔다 [S7].
- RSSI가 10dB 흔들리면 위치 오차가 평균 1.3m 정도 생긴다 [S8]. 좌석 단위로 특정하는 건 불가능하고, **"교실 안이냐 밖이냐"** 정도만 구분할 수 있다고 본다.
- 따라서 고정 임계치(-70dBm 등)를 쓰지 않는다. 운영 전 교실마다 보정한다.
  1. 교탁, 교실 뒤쪽, 복도, 옆 교실에서 기준 폰의 RSSI를 측정한다.
  2. "교실 내부" 판정 경계를 교실별로 저장한다.
  3. 판정은 절대값이 아니라 **기준선 대비 상대 변화**로 한다.
- 센서가 하나면 방향을 알 수 없다. 교실 앞뒤에 센서를 2개 두는 구성을 권장한다.

### 4.5 셀룰러 업링크 감지 시 5G 처리 (레퍼런스 검토 결과)

- 국내 5G는 NSA 중심으로 구축되었다 [S10]. NSA(EN-DC)에서는 LTE가 제어 평면의 앵커 역할을 하고, 단말이 LTE와 5G 무선을 모두 업링크에 쓴다 [S11]. 즉 5G 폰도 **LTE FDD 업링크 송신을 남길 가능성이 높으므로**, Phase 3는 LTE FDD 업링크 대역 감지를 우선한다.
- 예외: SA로 접속한 단말은 LTE 앵커가 없을 수 있다. 이 경우는 n78을 직접 봐야 한다.
- n78(3.5GHz)은 TDD다. 국내 이통3사는 다운로드:업로드 시간 비율을 4:1로 채택했다(2019년 보도) [S12]. n77/n78 대역에서는 DDDSU 프레임이 널리 쓰인다 [S13].
- n78을 직접 감지하는 방안:
  - **(A) 시간 게이팅**: 기지국 SSB에 동기화해 UL 슬롯(U)에서만 전력을 측정한다. 정확하지만 구현 난도가 높다(NR 셀 서치 필요).
  - **(B) 근접 전력 비교**: 교실 안 단말 업링크는 거리가 매우 가까워서, 원거리 기지국 다운링크보다 수신 전력이 높을 가능성이 있다. *자유공간 기준 3.5GHz, 1m 경로손실은 약 43dB(자체 계산)*. 다만 단말 전력 제어 때문에 송신 전력이 낮으면 차이가 줄어든다. **실측 전에는 가설로 취급한다.**

### 4.6 (뺌) 블루투스 연결 존재 탐지

광고 없이 블루투스 연결만 유지하는 기기를 전용 스니퍼 장비로 잡던 절이다. **다루지 않기로 했다**(사용자 결정
2026-11-16). 규칙 R8·R9와 그 수집기·실측 계획을 함께 뺐고, 이 경로로만 막던 위협은 §3에서 범위 밖으로 적는다.
다른 문서와 코드가 "설계 §4.7"을 가리키므로 절 번호는 그대로 둔다.

### 4.7 다교실 위치 판정 (옆 교실 신호 처리)

#### 문제

- 옆 교실에서 켠 기기가 우리 교실 센서에도 잡힌다. 교실마다 독립적으로 판정하면 **엉뚱한 교실에서 알림이 뜬다.**
- 몸에 가려진 폰은 뒷주머니 기준 약 15dB까지 약해질 수 있어 [S6], 우리 교실의 숨긴 폰이 벽 너머 옆 교실 기기보다 약하게 잡히는 경우도 생긴다.

#### 관점 전환: 탐지 문제가 아니라 위치 판정 문제

- 시험 중에는 **건물 안 모든 폰이 꺼져 있어야** 한다. 옆 교실 기기를 잡았다면 그건 오탐이 아니라 "건물 어딘가에서 실제로 켜졌다"는 **정탐**이다. 틀린 건 교실 판정뿐이다.
- 따라서 판정을 두 단계로 나눈다.
  1. **건물 단위 탐지**: 어느 센서에서든 잡히면 "건물 내 탐지"로 확정한다.
  2. **교실 단위 위치 판정**: 여러 센서의 관측을 모아 어느 교실인지 추정한다.

#### 위치 판정 방법

1. **같은 기기로 묶기 (상관)**: BSSID와 Windows에서 본 BLE 주소는 어느 센서에서 보든 같은 값이다. 교실별 센서 데이터를 건물 서버로 모아 "같은 기기를 여러 센서가 동시에 봤다"를 판별한다.
   - BLE 광고는 MAC 랜덤화 때문에 주소만으로 묶기 어렵다. 같은 시간 창 안의 제조사 데이터 일치로 보조한다.
2. **최대 RSSI 교실로 배정**: 기기를 RSSI가 가장 높은 센서의 교실에 배정한다. 벽 하나만으로 1m 거리에서도 약 6dB가 떨어지므로 [S7], 벽을 사이에 둔 교실끼리는 차이가 난다.
3. **중앙값으로 비교**: 단발 측정이 아니라 **수십 초 동안의 중앙값**으로 비교한다. 순간적인 인체 차폐와 다중경로 흔들림을 걸러낸다.
4. **교실 내 앞뒤 구분**: 교실마다 앞뒤 센서 2개(§4.4)를 두면 교실 안 대략적인 위치(앞/뒤)까지 추정한다.
5. **신뢰도 산정**: 1위와 2위 교실의 RSSI 중앙값 차이로 신뢰도를 매긴다. *예시 기준(실측으로 보정 필요): 차이 ≥ 10dB 높음, 5~10dB 중간, < 5dB 낮음.*

#### 알림 형태

```
[확인 필요] 숨긴 Wi-Fi 핫스팟 감지 (BSSID 02:00:5E:00:53:2A)
추정 위치: 3-2 교실 뒤쪽 · 신뢰도 중
근거: 3-2 뒤 센서 -58dBm / 3-2 앞 -66 / 3-3 -71 / 3-1 미탐지
```

- 판정 근거(센서별 RSSI)를 함께 보여 준다. 신뢰도가 낮으면 인접 교실 감독관에게도 동시에 알린다.

#### 최종 확인: 휴대용 추적

- 추정 위치가 나오면 감독관이 휴대용 장비(노트북, 또는 BLE·Wi-Fi 스캔 앱)를 들고 신호가 세지는 쪽으로 좁혀 간다(전파 추적, 이른바 "여우 사냥").
- 지향성 안테나를 쓰면 훨씬 빨라진다. *적합한 안테나는 확인 필요.*
- 자동 판정은 어디까지나 "확인 필요" 수준이며, 최종 판단은 사람이 한다(§4.3).

## 5. 아키텍처

### 5.0 앱 형태와 실행 모드

하나의 데스크톱 앱(Windows `.msi`, macOS `.dmg`)이 설정에 따라 네 가지 모드로 동작한다.

| 모드 | 하는 일 | 쓰는 곳 |
|---|---|---|
| **단독** | 수집 + 판정 + 알림을 한 대에서 | 교실 1개 시범, 소규모 시험장 |
| **센서** | 수집만 하고 건물 서버로 전송 | 교실 앞/뒤 노트북 |
| **서버** | 건물 단위 상관·위치 판정·알림, 웹 대시보드 제공 | 시험본부 노트북 |
| **감독관 뷰** | 서버 대시보드를 앱 창으로 표시 | 감독관 노트북 |

- 감독관이 태블릿을 쓰는 경우 앱을 설치하지 않고, 서버가 내부망에 띄우는 **웹 대시보드**에 브라우저로 접속한다. 배포 대상이 Windows / macOS뿐이어도 태블릿을 지원할 수 있다.
- 앱 프레임워크는 **Tauri 2**를 1순위로 둔다(가정). Rust 백엔드를 그대로 쓰고, UI는 웹 기술이라 서버의 웹 대시보드와 화면을 공유할 수 있다. 대안은 `egui` / `iced`(순수 Rust GUI). 확정은 §10.

### 5.1 배포 구조: 교실 센서 → 건물 서버

다교실 위치 판정(§4.7)을 위해 **교실별 독립 판정에서 중앙 집계 구조로** 바꾼다.

```
[3-1 센서 앞] [3-1 센서 뒤]   [3-2 센서 앞] [3-2 센서 뒤]   [3-3 ...]
 (Win/mac 앱)  (Win/mac 앱)
      │            │               │            │
      └────────────┴───── 시험장 내부망(유선 LAN 권장) ─────┘
                               │  SensorReport (TLS)
                               ▼
                 ┌──────── 서버 모드 앱 (Win/mac) ─────────┐
                 │ 건물 단위 기준선                          │
                 │ cross-sensor correlator (같은 기기 묶기)   │
                 │ room locator (최대 RSSI 중앙값, 신뢰도)    │
                 │ rule engine → score                       │
                 │ 웹 대시보드 (axum)                         │
                 └──────────────────┬───────────────────────┘
                                    ▼
             감독관 뷰 앱 / 감독관 태블릿 브라우저 (교실별 + 인접 교실 알림)
```

- 센서는 **수집만** 하고 판정은 서버가 한다. 센서끼리 시간이 맞아야 중앙값 비교가 가능하므로 NTP 동기화를 필수로 둔다.
- 센서 ↔ 서버 연결은 무선 스캔 대상과 섞이지 않도록 **유선 LAN을 권장**한다. 무선을 쓰면 해당 AP를 기준선 화이트리스트에 등록한다.
- 서버 찾기와 연결: 내부망 mDNS로 서버를 찾고, 서버 화면의 **페어링 코드**를 센서에 입력해 등록한다. 통신은 TLS(서버 인증서 고정)로 암호화한다.

### 5.2 수집기 구성

```
┌──────────────────── collectors ────────────────────┐
│  ble       (btleplug: WinRT / CoreBluetooth)        │
│  wifi      (Win32 WLAN API / CoreWLAN)              │──┐
│  sdr       (libhackrf / SoapySDR + rustfft) [opt]   │  │  tokio mpsc
└─────────────────────────────────────────────────────┘  │  Observation
                                                         ▼
                               ┌─────────── core ───────────┐
                               │ baseline store              │
                               │ device correlator           │
                               │ rule engine → score         │
                               └──────────────┬──────────────┘
                                              ▼
                               ┌─────── alert / UI ─────────┐
                               │ 앱 창 (Tauri) · 웹 대시보드  │
                               └─────────────────────────────┘
```

- **모니터 모드 수집기는 뺀다.** Windows는 사실상 불가능하고, macOS는 14.4부터 `airport` 도구가 동작하지 않는 등 지원이 축소됐다 [S31]. 따라서 숨긴 SSID의 **이름**은 알 수 없지만, 빈 SSID + BSSID 탐지(R3–R5)로 충분하다.

### 5.3 워크스페이스 구성

```
crates/
  core/                # 타입, 기준선, 규칙 엔진, 점수 계산, 위치 판정
  collector-ble/       # btleplug (Windows: WinRT, macOS: CoreBluetooth)
  collector-wifi/      # cfg(windows): Win32 WLAN API / cfg(macos): CoreWLAN (objc2)
  collector-sdr/       # feature = "sdr" — libhackrf FFI 또는 SoapySDR, rustfft
  net/                 # 센서↔서버 프로토콜, mDNS, TLS, 페어링
  dashboard/           # axum 웹 대시보드 (서버 모드)
  app/                 # Tauri 앱: 모드 선택, 권한 온보딩, 알림 UI
  devtools/            # 개발 도구(macOS/리눅스): 관측 녹화·재생
```

### 5.4 핵심 타입 (스케치)

```rust
pub enum Observation {
    Ble {
        /// Windows: 실제 (랜덤) 주소 / macOS: CoreBluetooth가 주는 기기별 UUID
        id: BleId,
        rssi: i16,
        manufacturer: Vec<(u16, Vec<u8>)>,
        services: Vec<Uuid>,
        at: Instant,
    },
    Wifi {
        bssid: [u8; 6],
        ssid: Option<String>, // None = hidden
        rssi: i16,
        channel: u16,
        band: Band, // B2_4, B5, B6
        at: Instant,
    },
    Rf {
        band: CellBand, // B1, B3, B5, B7 uplink
        power_dbm: f32,
        at: Instant,
    },
}

pub enum BleId {
    Addr([u8; 6]),     // Windows
    HostLocal(Uuid),   // macOS — 다른 센서와 비교 불가 (§7.2)
}

/// 센서가 서버로 보내는 단위
pub struct SensorReport {
    pub sensor_id: SensorId,
    pub room_id: RoomId,
    pub position: SensorPos, // Front | Back
    pub platform: Platform,  // Windows | MacOS
    pub obs: Observation,
}

/// 서버의 위치 판정 결과
pub struct Localization {
    pub device: DeviceKey,          // BSSID / BLE 상관 키
    pub room: Option<RoomId>,       // None = 건물 내 탐지, 교실 미정
    pub position: Option<SensorPos>,
    pub confidence: Confidence,     // High | Medium | Low
    pub evidence: Vec<(SensorId, i16 /* median RSSI */)>,
}

#[async_trait]
pub trait Collector {
    async fn run(&self, tx: mpsc::Sender<Observation>) -> Result<()>;
}

pub trait Rule {
    fn id(&self) -> &'static str;
    fn score(&self, obs: &Observation, baseline: &Baseline) -> Option<Hit>;
}
```

> 구현 메모: core는 시계를 읽지 않으므로 `at: Instant`는 `at: Timestamp`(세션 시작 기준 ms, 센서는 NTP로 맞춘
> 시계로 찍음)로 바꾼다. `Collector` 트레이트는 tokio에 의존하므로 core 밖에 둔다. 위치는 `docs/STATUS.md`의 열린
> 결정이다. `Observation`, `SensorReport`, `Localization`은 core 타입이고 `net`이 직렬화한다.

## 6. 의존성

| 영역 | 크레이트 / API | 비고 |
|---|---|---|
| 앱 | `tauri` (2.x) | 설치 파일 생성(.msi / .dmg), 서명·공증 연동 (§10에서 확정) |
| 비동기 | `tokio` | 수집기마다 태스크 분리 |
| BLE | `btleplug` | Windows(WinRT) / macOS(CoreBluetooth) |
| Wi-Fi (Windows) | `windows` → `WlanScan`, `WlanGetNetworkBssList` | **정밀 위치 권한 필요** (§7.1) |
| Wi-Fi (macOS) | `objc2-core-wlan`, `objc2-core-location` | **위치 권한 + Developer ID 서명 필요** (§7.2) |
| SDR | `libhackrf` FFI 또는 SoapySDR, `rustfft` | HackRF는 Windows용 네이티브 DLL·도구가 공식 제공된다 [S34] |
| 네트워크 | `mdns-sd`, `rustls`, `tokio-tungstenite` | 서버 찾기, TLS, 센서 스트림 |
| 대시보드 | `axum` | 서버 모드 웹 UI |
| 절전 방지 | `windows`(`SetThreadExecutionState`) / IOKit 전원 어서션 | 시험 중 잠자기 방지 |
| 설정 | `serde`, `toml` | 임계치, 화이트리스트, 교실 보정값 |

## 7. 플랫폼별 제약

| 항목 | Windows 11 | macOS |
|---|---|---|
| BLE 스캔 | O — 수동(passive) 스캔 가능 | O — **능동 스캔만 가능**, 기기 주소 대신 기기별 UUID 제공 |
| Wi-Fi 스캔 (BSSID) | O — **정밀 위치 권한 필요** | △ — **위치 권한 + 안정적 Developer ID 서명 필요**, 버전별 이슈 있음 |
| 모니터 모드 | X | X (앱에서 사용 안 함) |
| 6GHz 스캔 | 6E 칩 필요 (예: Intel AX211, BE200) | 6E 지원 기종 |
| SDR (HackRF) | O — 공식 네이티브 DLL | O |
| 백그라운드 실행 | 로그인한 사용자 세션의 앱으로 실행 | **로그인한 사용자 세션 필수** (데몬 불가) |

### 7.1 Windows

- Windows 11 24H2부터 BSSID를 주는 API는 **사용자가 정밀 위치 접근을 허용한 앱에만** 열린다. 허용하지 않으면 처음 막힐 때 시스템이 한 번 알림을 띄우고, `WlanGetNetworkBssList`, `WlanScan` 등은 `ERROR_ACCESS_DENIED`를 돌려준다 [S32].
- 기기 전체 위치 서비스와 "데스크톱 앱이 위치에 접근하도록 허용"을 켜야 한다. 이 설정이 꺼져 있으면 기업용 보안·VPN 제품들도 Wi-Fi 정보를 읽지 못한다고 안내하고 있다 [S35].
- → 앱 첫 실행 시 **권한 온보딩 화면**에서 위치 설정을 확인하고, 꺼져 있으면 설정 화면으로 바로 안내한다.
- 학교·기관 PC는 그룹 정책으로 위치 서비스가 막혀 있을 수 있다. 시험장 장비 준비 체크리스트에 넣는다.

### 7.2 macOS

- **Wi-Fi**: CoreWLAN의 `scanForNetworks`는 위치 권한이 없으면 SSID·BSSID를 비워서 준다(macOS 14 이후) [S36]. 실제 BSSID는 **위치 권한을 받은, 안정적인 Developer ID로 서명된 앱**에만 주어지고, 위치 권한 확인이 사용자 세션 단위라 시스템 데몬(launchd)으로는 동작하지 않는다 [S37].
  - → 반드시 서명·공증된 `.app`으로 배포하고, 로그인한 사용자 세션에서 실행한다.
  - ⚠️ macOS 15.1에서 권한을 줘도 SSID·BSSID가 비어 나온다는 개발자 보고가 있다 [S38]. **macOS를 Wi-Fi 센서로 쓸 수 있는지 버전별로 실측**해야 한다. 안 되면 macOS 센서는 BLE 전용으로 두고 Wi-Fi는 Windows 센서가 맡는다.
- **BLE**: CoreBluetooth는 기기 주소를 주지 않고 기기마다 UUID를 부여한다 [S39]. 이 UUID는 **호스트마다 달라서 다른 센서와 같은 기기인지 비교할 수 없다** [S40].
  - → 다교실 상관(§4.7)에서 macOS 센서의 BLE 관측은 "같은 시간 창 + 같은 제조사 데이터"로만 묶는다. 위치 판정 신뢰도를 한 단계 낮춘다.
  - macOS는 수동 스캔을 지원하지 않는다 [S41]. 능동 스캔은 스캔 요청을 **송신**하므로, "송신하지 않는다"는 원칙(§8)의 예외로 명시한다. 일반 블루투스 기기의 정상 동작 범위다.
- **HackRF**: 앱 샌드박스 밖(Mac App Store 미사용, Developer ID 직접 배포)에서 USB 장비에 접근한다.

### 7.3 설치와 배포

| 항목 | Windows | macOS |
|---|---|---|
| 설치 파일 | `.msi` (Tauri 번들) | `.dmg` (Tauri 번들) |
| 서명 | Authenticode 코드 서명 (SmartScreen 경고 방지) | Developer ID 서명 + 공증(notarization) + Hardened Runtime |
| 권한 문구 | — | `Info.plist`: 블루투스 사용 목적, 위치 사용 목적 |
| 첫 실행 | 위치 설정 확인 | 블루투스·위치 권한 요청 |
| 배포 경로 | 웹 다운로드 / 기관 배포 | 웹 다운로드 (Mac App Store 미사용) |
| 업데이트 | 시험 기간 중 자동 업데이트 **끔** (버전 고정) | 동일 |

### 7.4 운영 시 주의

- **절전**: 시험 시간 동안 잠자기·화면 꺼짐을 막는다. 노트북 덮개를 닫으면 잠들 수 있으므로 덮개를 연 채 전원 어댑터를 연결한다.
- **권한 사전 점검**: 시험 전날 "점검 모드"로 BLE·Wi-Fi·USB 장비 인식과 권한 상태를 한 화면에서 확인한다.
- **혼합 운영 권장 구성**: Windows 노트북을 Wi-Fi + BLE 주력 센서로, macOS는 BLE 센서 또는 서버로 둔다(§7.2의 Wi-Fi 이슈 때문).

## 8. 법적 고려

- **전파법**: 송신하지 않는다. 재밍이나 능동 교란 기능은 넣지 않는다. 예외: macOS의 BLE 능동 스캔 요청은 일반 블루투스 기기의 정상 동작으로 허용한다(§7.2). Windows는 수동 스캔을 쓴다.
- **통신비밀보호법**: 패킷 내용, 셀룰러 제어 정보·식별자를 디코딩하지 않는다. 에너지, 비콘 헤더, 광고 메타데이터만 사용한다.
- **개인정보**: MAC과 광고 데이터는 세션 종료 후 폐기를 기본으로 하고, 보존이 필요하면 해시 처리한다.
- 실제 시험장에 적용하려면 주관 기관의 승인과 법률 검토가 필요하다.

## 9. 로드맵

| Phase | 범위 | 완료 기준 |
|---|---|---|
| 1 | **Tauri 앱 골격(단독 모드)**, 권한 온보딩, BLE + Wi-Fi 스캔(Windows 우선), 기준선, R1–R6 | Windows 노트북 한 대로 숨긴 핫스팟과 폰 BLE를 탐지 / macOS Wi-Fi 동작 여부 실측(§7.2) |
| 2 | (뺌) 블루투스 연결 탐지 — 다루지 않는다(사용자 결정 2026-11-16, §4.6) | — |
| 2.5 | **센서/서버/감독관 뷰 모드, 웹 대시보드, 다교실 위치 판정(§4.7)** | 인접 3개 교실 시범(Windows·macOS 혼합)에서 옆 교실 기기를 올바른 교실로 배정 |
| 2.9 | **배포**: 코드 서명, macOS 공증, 설치 파일, 점검 모드 | 서명된 설치 파일로 새 PC에 설치해 권한 온보딩부터 탐지까지 완료 |
| 3 | SDR 업링크 에너지 감지(R7), LTE FDD 업링크 대역 | 폰 사진 업로드 버스트를 탐지 |
| 4 | 6GHz 대응 검증, 갤럭시 글래스 실기 테스트 | 6GHz 전용 연결을 탐지 |

## 10. 미결정 사항

- [ ] 프로젝트명
- [ ] 라이선스
- [x] AI 안경 제조사별 BLE 광고 특성 수집 → R1a에 반영. **메타 레이밴 2세대는 폰과 연결된 뒤 광고를 멈춘다는 테스트 결과**가 있어 [S4], 안경 직접 탐지는 보조 수단으로 두고 폰 탐지(R1)를 주력으로 한다.
  - [ ] 삼성 인텔리전트 아이웨어의 광고 특성 (12월 출시 후 실측)
  - [ ] 메타 1세대, 오클리 메타, 레이밴 디스플레이가 연결 후에도 광고하는지 실측
- [ ] RSSI 경계 실측 → 방법은 §4.4로 확정, 수치는 현장 측정 필요
- [ ] 수거한 폰의 "전원 꺼짐"을 보장하는 운영 절차와의 연계
- [x] 5G n78 업링크 감지 방안 → §4.5. NSA 단말은 LTE FDD 업링크로 커버. SA 단말용 (A)/(B)는 실측으로 검증 필요
- [ ] 의료기기 화이트리스트 등록 절차
- [x] 블루투스 연결 탐지(R8·R9) → 다루지 않는다(사용자 결정 2026-11-16, §4.6)
- [ ] 앱 프레임워크 확정: Tauri 2 vs egui / iced
- [ ] macOS 버전별(14 / 15 / 26) CoreWLAN BSSID 수신 실측 — 안 되면 macOS는 Wi-Fi 센서 제외
- [ ] Windows 10 지원 여부 (24H2 위치 권한 정책은 Windows 11 기준)
- [ ] 코드 서명 인증서(Windows), Apple Developer Program 계정 준비
- [ ] 학교·기관 PC의 위치 서비스 그룹 정책 확인 절차
- [ ] 위치 판정 신뢰도 기준(1·2위 RSSI 차이) 실측 보정
- [ ] 센서 ↔ 서버 망 구성 (유선 LAN 확보 가능 여부, 시험장별 차이)
- [ ] 휴대용 추적용 지향성 안테나 선정

## 11. 참고 자료

**BLE 식별자**

- [S1] Bluetooth SIG Company Identifiers (미러): Apple `0x004C`, Samsung `0x0075` — https://gist.github.com/angorb/f92f76108b98bb0d81c74f60671e9c67
- [S2] Android `BluetoothAssignedNumbers.GOOGLE` = 224 (`0x00E0`) — https://developer.android.com/reference/kotlin/android/bluetooth/BluetoothAssignedNumbers
- [S3] Nearby Glasses (안경 company ID 목록, 한계) — https://www.helpnetsecurity.com/2026/02/27/nearby-glasses-android-app-detect-smart-glasses/ · 소스: https://github.com/yjeanrenaud/yj_nearbyglasses
- [S4] 레이밴 메타 2세대가 연결 후 광고를 중단한다는 테스트 — https://virtual.reality.news/news/app-to-detect-meta-glasses-nearby-has-a-critical-blind-spot/
- [S5] Google Fast Pair: 광고 주체는 Provider(액세서리), UUID `0xFE2C` — https://developers.google.com/nearby/fast-pair/specifications/service/provider
- [S5a] Apple Continuity BLE 광고 분석 (Martin et al., PETS 2019) — https://arxiv.org/abs/1904.10600
- [S5b] ZuckOff (`0x0D53`, `0x058E`, `0x03C2`, `0xFD5F` 매핑) — https://pinggy.io/blog/zuckoff_meta_glasses_bluetooth_detector/

**RSSI / 전파 감쇠**

- [S6] 인체 차폐: 뒷주머니가 앞주머니 대비 약 15dB 낮음 (Leith & Farrell, 2020) — https://arxiv.org/pdf/2006.06822
- [S7] 벽 감쇠 약 6dB, 8.5m에서 패킷 손실 약 50% (ESP32 BLE 연구 요약) — https://www.beaconzone.co.uk/blog/wp-json/wp/v2/posts/10027
- [S8] RSSI 10dB 변동 시 평균 위치 오차 1.32m — https://aminer.org/pub/5e09aa8ddf1a9c0c416c28a9/experimental-analysis-of-received-signals-strength-in-bluetooth-low-energy-ble-and
- [S9] 2.4GHz 인체 차폐 분석 (Sensors, 2018) — https://www.ncbi.nlm.nih.gov/pmc/articles/PMC6211019/

**5G / 셀룰러**

- [S10] ETRI 보고서: 국내 5G는 NSA 기술 중심 — https://ksp.etri.re.kr/ksp/plan-report/file/1303.pdf
- [S11] NSA(EN-DC)에서 LTE 앵커와 LTE·5G 동시 업링크 — https://patents.google.com/patent/US10405368
- [S12] 국내 5G TDD 다운:업 4:1 채택 (2019) — https://v.daum.net/v/Np5GHamZr4
- [S13] n77/n78 대역의 DDDSU 프레임 — https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/12010072
- [S14] GSMA 3.5GHz TDD 동기화 가이드 — https://www.gsma.com/spectrum/wp-content/uploads/2020/04/3.5-GHz-5G-TDD-Synchronisation.pdf

**안경 ↔ 폰 경로**

- [S21] visor wiki: 메타 안경 ↔ 폰 프로토콜 (BLE L2CAP / 클래식 RFCOMM / Wi-Fi Direct) — https://github.com/macedonga/visor/wiki

**Windows / macOS 앱 배포**

- [S31] Apple Developer Forums: macOS 14.4에서 airport 도구 중단, CoreWLAN SSID에 위치 권한 필요 — https://developer.apple.com/forums/thread/748505
- [S32] Microsoft Learn: Wi-Fi access and location changes (BSSID API는 정밀 위치 허용 앱만) — https://learn.microsoft.com/windows/win32/nativewifi/wi-fi-access-location-changes
- [S34] r/hackrf: HackRF는 공식 CI 빌드로 Windows 네이티브 DLL·도구 제공 — https://redlib.hackliberty.org/r/hackrf/comments/1k5vqd4/release_hackrfoneforwindows_comprehensive_windows
- [S35] Cisco Secure Client 사례: 24H2에서 위치 서비스·데스크톱 앱 위치 접근을 켜야 동작 — https://learn.microsoft.com/en-us/answers/a/1884468
- [S36] Apple Developer Forums: macOS 14+에서 위치 권한 없이는 scanForNetworks의 ssid/bssid가 nil — https://developer.apple.com/forums/thread/840513
- [S37] macwifi: 실제 BSSID는 위치 권한 + 안정적 Developer ID 서명 앱만, launchd 데몬 불가 — https://jaisonerick.github.io/macwifi/how-it-works · https://dev.to/jaisonerick/reading-wi-fi-data-from-go-on-macos-after-apple-removed-airport-19g
- [S38] Apple Developer Forums: Sequoia 15.1에서 권한을 줘도 SSID·BSSID가 비어 나옴 — https://developer.apple.com/forums/thread/769950
- [S39] SimpleBLE platform notes: macOS/iOS는 MAC 대신 기기별 UUID — https://docs.simpleble.org/simpleble/platform_notes
- [S40] SimpleBLE platform notes: 이 UUID는 호스트마다 다름 — https://docs.simpleble.org/docs/simpleble/platform_notes
- [S41] bleak CoreBluetooth backend: macOS는 수동 스캔 미지원 — https://bleak.readthedocs.io/en/latest/_modules/bleak/backends/corebluetooth/scanner.html
