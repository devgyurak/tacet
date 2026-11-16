# R4 — locally administered BSSID

- 상태: 구현됨
- 단계: Phase 1
- 막는 위협: T1, T3, T4 (설계 §3)
- 수집기·플랫폼: Wi-Fi 수집기

## 흔적

폰 핫스팟은 랜덤 MAC(locally administered 비트 = 1)을 BSSID로 쓰는 경우가 많다.

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| `bssid[0] & 0x02` | 1 | multicast 비트(`0x01`)가 켜진 주소는 제외 |

## 쓰는 필드

BSSID만 쓴다.

## 가중치

기본 4(높음).

## 근거

IEEE 802 MAC 주소 비트 정의.

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 학교 AP가 LA 주소를 쓰는 경우 | 기준선에 있으면 미적중 | 기준선 |

## 경계

multicast + LA는 미적중.

## 테스트

`given_multicast_locally_administered_bssid_when_scored_then_r4_misses`

## 미검증

없음.
