# R3 — 새 BSSID가 교실 경계 안

- 상태: 구현됨
- 단계: Phase 1
- 막는 위협: T1, T3, T4 (설계 §3)
- 수집기·플랫폼: Wi-Fi 수집기(Windows WLAN API, macOS CoreWLAN)

## 흔적

기준선에 없던 Wi-Fi 네트워크가 교실 경계 안의 세기로 보인다.

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| `rssi` | `≥ 그 센서의 wifi_inside_boundary_dbm`(없으면 판정을 미루고 "와이파이 기준값 없음" 주의 — 블루투스 경계를 빌리지 않는다) | 와이파이 경계는 핫스팟으로 따로 잰다(`L-room-calibration.md`) |

## 쓰는 필드

RSSI만 쓴다.

## 가중치

기본 2(중간).

## 근거

설계 §4.2.

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 옆 교실·복도 AP | 기준선에 있으면 미적중 | 기준선 |

## 경계

R2와 같음.

## 테스트

`given_hidden_ssid_random_bssid_inside_boundary_when_scored_then_r3_r4_r5_hit`, `given_t3_hidden_ssid_hotspot_when_watched_then_r3_r4_r5_alert`, `given_separate_wifi_boundary_when_hotspot_weaker_than_it_then_no_inside_room_hit_for_wifi`, `given_only_bluetooth_boundary_when_wifi_seen_then_wifi_calibration_missing_and_no_inside_room_hit`

## 미검증

Wi-Fi RSSI와 BLE RSSI의 경계가 같아도 되는지 실측 필요.
