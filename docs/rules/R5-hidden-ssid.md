# R5 — 숨긴 SSID

- 상태: 구현됨
- 단계: Phase 1
- 막는 위협: T3, T4 (설계 §3)
- 수집기·플랫폼: Wi-Fi 수집기

## 흔적

SSID를 숨긴 핫스팟도 비콘을 송신한다. 숨긴 네트워크는 길이 0 SSID나 NUL로 채운 SSID로 나타난다.

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| `ssid` | 길이 0이거나 모든 바이트가 0 | `Ssid::from_bytes` |

## 쓰는 필드

SSID 바이트만 쓴다. 숨긴 SSID의 이름은 알아내지 않는다(모니터 모드 없음, 설계 §5.2).

## 가중치

기본 4(높음).

## 근거

IEEE 802.11 비콘 SSID 요소.

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 학교의 숨긴 AP | 기준선에 있으면 미적중 | 기준선 |

## 경계

두 표현 모두 적중. macOS는 SSID 데이터가 없을 때도 숨김으로 보되, BSSID가 없으면(위치 권한 없음) 관측 자체를 버린다.

## 테스트

`given_zero_length_ssid_when_read_then_hidden`, `given_nul_filled_ssid_when_scored_then_r5_hits`, `given_6ghz_hidden_network_when_scored_then_r5_hits`

## 미검증

macOS 버전별로 SSID 데이터가 권한과 함께 오는지(설계 §7.2).
