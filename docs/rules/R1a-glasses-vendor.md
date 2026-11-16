# R1a — 스마트 안경 BLE 광고

- 상태: 구현됨
- 단계: Phase 1
- 막는 위협: T1, T2 (설계 §3). T7(연결 뒤 광고 중단)은 전원·페어링 구간에 걸릴 때만 잡는다
- 수집기·플랫폼: BLE 수집기

## 흔적

스마트 안경은 전원을 켤 때와 페어링 구간에서 광고한다. 레이밴 메타 2세대는 폰과 연결된 뒤 광고를 멈춘다([S4]) — 그래서 이 규칙은 보조 수단이다. 같은 폰의 광고(R1)와 Wi-Fi Direct(R6)는 폰이 광고하거나 전송하는 구간에만 잡히므로, 연결 뒤 폰이 조용하면 공백이다. 광고 없이 연결만 남은 안경은 범위 밖이다(설계 §3, §4.6).

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| `manufacturer[].company` | `0x01AB`(Meta), `0x058E`(Meta Platforms Technologies), `0x0D53`(Luxottica), `0x03C2`(Snap) 중 하나 | `rules::GLASSES_VENDOR_IDS` |
| `services` 또는 `service_data` | 16비트 `0xFD5F`(Oculus) 포함 | `rules::OCULUS_SERVICE` |

## 쓰는 필드

회사 ID와 서비스 UUID만 쓴다.

## 가중치

기본 6(매우 높음). `config::Weights::r1a`.

## 근거

설계 §4.2, [S3] Nearby Glasses, [S5b] ZuckOff 매핑.

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 시험 전 연결이 끝난 안경 | 광고 없음 → 미적중 | 잡지 못한다(범위 밖). 폰이 광고·전송하는 구간에만 R1·R6가 잡는다 |
| 감독관의 스마트 안경 | 적중 | 화이트리스트 |

## 경계

서비스 데이터로만 `0xFD5F`가 오는 경우도 적중.

## 테스트

`given_each_glasses_vendor_id_when_scored_then_no_vendor_misses_r1a`, `given_oculus_service_uuid_when_scored_then_r1a_hits`, `given_glasses_power_on_advertising_when_watched_then_r1a_and_r2_alert`

## 미검증

삼성 인텔리전트 아이웨어의 회사 ID·서비스(12월 출시 후 실측), 메타 1세대·오클리 메타·레이밴 디스플레이가 연결 후에도 광고하는지(설계 §10).
