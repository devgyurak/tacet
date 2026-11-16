# R2 — 새 BLE 광고자가 교실 경계 안

- 상태: 구현됨
- 단계: Phase 1
- 막는 위협: T1, T2 (설계 §3)
- 수집기·플랫폼: BLE 수집기

## 흔적

기준선에 없던 광고자가 교실 경계 안의 세기로 보인다.

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| `rssi` | `≥ 그 센서의 inside_boundary_dbm` (같으면 적중) | 고정 dBm 금지. 보정값이 없으면 평가하지 않고 "보정 필요" 경고(설계 §4.4) |

## 쓰는 필드

RSSI만 쓴다.

## 가중치

기본 2(중간).

## 근거

설계 §4.2, §4.4.

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 옆 교실 기기 | 경계 아래면 미적중 | 보정 경계 |
| 몸에 가려진 폰(뒷주머니 약 15dB) | 경계 아래로 떨어질 수 있다 | 다른 규칙(R1·R1a)과 합산, 위치 판정은 상대 비교 |

## 경계

경계와 같은 RSSI는 적중(`≥`). 보정값 없음은 평가 안 함.

## 테스트

`given_rssi_equal_to_boundary_when_scored_then_r2_hits`, `given_rssi_one_below_boundary_when_scored_then_r2_misses`, `given_uncalibrated_room_when_phone_ble_seen_then_alert_without_boundary_rule_and_low_confidence`

## 미검증

교실별 경계값은 `field-test`의 교실 보정 절차로 정한다.
