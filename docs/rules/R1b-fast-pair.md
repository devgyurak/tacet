# R1b — Fast Pair 액세서리 광고

- 상태: 구현됨
- 단계: Phase 1
- 막는 위협: T1, T2(보조) (설계 §3)
- 수집기·플랫폼: BLE 수집기

## 흔적

Fast Pair 서비스(`0xFE2C`)는 폰이 아니라 이어폰 같은 액세서리가 광고한다([S5]). 액세서리는 폰과 함께 다니는 경우가 많아 약한 신호로만 쓴다.

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| `services` 또는 `service_data` | 16비트 `0xFE2C` 포함 | `rules::FAST_PAIR_SERVICE` |

## 쓰는 필드

서비스 UUID만 쓴다.

## 가중치

기본 1(낮음~중간). 이 규칙 하나로는 알림 문턱(4)을 넘지 않는다.

## 근거

Google Fast Pair Provider 규격([S5]).

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 수험생의 무선 이어폰(전원 켜짐) | 적중 | 단독으로는 알림 안 됨 |

## 경계

R1과 함께 오면 합산된다.

## 테스트

`given_fast_pair_service_data_when_scored_then_only_r1b_hits`

## 미검증

가중치(`TODO(field-unverified)`).
