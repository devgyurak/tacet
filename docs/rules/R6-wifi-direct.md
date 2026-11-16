# R6 — Wi-Fi Direct SSID

- 상태: 구현됨
- 단계: Phase 1
- 막는 위협: T1, T2(촬영 → 전송 단계) (설계 §3)
- 수집기·플랫폼: Wi-Fi 수집기

## 흔적

안경이 사진·영상을 폰으로 보낼 때 폰이 그룹 소유자인 Wi-Fi Direct를 쓴다([S21]). 그룹 소유자는 `DIRECT-`로 시작하는 SSID 비콘을 낸다.

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| `ssid` | `DIRECT-`로 시작(대문자, Wi-Fi P2P 규격) | `rules::WIFI_DIRECT_PREFIX` |

## 쓰는 필드

SSID 접두사만 쓴다.

## 가중치

기본 4(높음).

## 근거

Wi-Fi P2P 규격, 안경 ↔ 폰 경로([S21], 설계 §11).

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 교실 프린터의 Wi-Fi Direct | 기준선에 있으면 미적중 | 기준선 |

## 경계

소문자 `direct-`는 미적중.

## 테스트

`given_direct_prefix_ssid_when_scored_then_r6_hits`, `given_lowercase_direct_prefix_when_scored_then_r6_misses`, `given_photo_transfer_over_wifi_direct_when_watched_then_new_random_direct_wifi_alerts`

## 미검증

사진 전송 중 `DIRECT-` 비콘이 실제로 나타나는지(`field-test` 스킬로 실측).
