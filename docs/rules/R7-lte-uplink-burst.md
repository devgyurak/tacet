# R7 — LTE 업링크 대역 버스트

- 상태: 구현됨(수집기는 `sdr` 기능)
- 단계: Phase 3
- 막는 위협: T1, T2, T5 (설계 §3)
- 수집기·플랫폼: SDR 수집기(SoapySDR, HackRF)

## 흔적

폰이 외부로 데이터를 보내면 업링크로 송신한다. 국내 5G NSA 단말도 LTE FDD 업링크를 남긴다(설계 §4.5).

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| `power_cdbm` | `≥ 그 센서·대역의 기준선 중앙값 + rf_burst_cdb` | 기준선 대비 상대 변화. 기준선 관측이 없으면 평가 안 함 |

## 쓰는 필드

대역 평균 전력(dBFS 기준 상대값)만 쓴다. 복조·동기화·디코딩 없음(AGENTS.md §3.2).

## 가중치

기본 4(높음). 버스트 기본 +10dB(`rf_burst_cdb = 1000`).

## 근거

3GPP TS 36.101 표 5.5-1(B1·B3·B5·B7 업링크), 설계 §4.5.

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 옆 교실 폰의 업로드 | 적중할 수 있다 | 위치 판정(센서별 전력 비교) |
| 원거리 기지국 다운링크 | FDD라 업링크 대역과 겹치지 않는다 | 대역 분리 |

## 경계

기준선 중앙값은 짝수 개면 아래쪽. 기준선은 센서마다 따로.

## 테스트

`given_rf_power_at_reference_plus_burst_when_scored_then_r7_hits`, `given_rf_burst_over_sensor_baseline_when_watched_then_r7_alert`, `given_tone_inside_band_when_measured_then_louder_than_same_tone_outside_band`

## 미검증

버스트 문턱, 수신 이득(`TODO(field-unverified)`, `crates/collector-sdr/src/lib.rs`). n78(TDD) 직접 감지는 범위 밖 — 시간 게이팅은 법률 검토 전 착수하지 않는다.
