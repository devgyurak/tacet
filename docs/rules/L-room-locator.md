# L — 다교실 위치 판정

- 상태: 구현됨
- 단계: Phase 2.5
- 막는 위협: 전 위협(옆 교실 오탐 처리) (설계 §3)
- 수집기·플랫폼: 서버 모드(여러 센서), 단독 모드(한 교실)

## 흔적

옆 교실 기기를 잡았다면 오탐이 아니라 건물 어딘가의 정탐이다. 틀리기 쉬운 것은 교실 판정이다(설계 §4.7).

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| 센서별 세기 | 최근 `locate_window_ms` 동안의 중앙값(짝수 개면 아래쪽) | `locate::lower_median` |
| 교실 | 그 교실 센서들의 중앙값 중 최댓값이 가장 큰 교실 | 동점은 센서 ID 순 |
| 신뢰도 | 1·2위 교실 차이 `≥ high_confidence_db` 높음, `≥ medium_confidence_db` 중간, 그 밖 낮음 | 여러 교실 배치에서 한 교실만 봤으면 **낮음**(다른 교실 센서가 아직 못 받았을 수 있다 — 인접 교실에도 알린다), 교실이 하나뿐인 배치면 높음 |
| macOS 호스트별 UUID | 신뢰도 한 단계 낮춤 | 설계 §7.2 |
| 가장 센 센서에 보정값 없음 | 신뢰도 낮음 + "보정 필요" | |
| 모든 센서가 경계보다 `shield_margin_db`(기본 15dB) 넘게 약함 | 교실 미정, 건물 밖 가능성(감점) | 설계 §4.3. 경계 바로 아래는 몸에 가려진 교실 안 폰일 수 있어 감점하지 않는다(설계 §4.4, [S6] 뒷주머니 약 15dB) |
| 앞·뒤 위치 | 그 교실 다른 센서가 못 봤거나 두 센서 차이가 `medium_confidence_db` 이상일 때만 표시 | 1–2dB 차이로 "뒤쪽"이라 하지 않는다 |
| 보관 위치 감점 | BLE 기기이고, 보관 위치 센서의 최근 **중앙값**이 그 범위일 때. 폰 광고 규칙(R1·R1b·R2) 점수까지만 뺀다 | 한 번 튄 표본이나 보관 위치 근처의 핫스팟 점수는 지우지 않는다 |
| 재알림 | 강도가 오르거나, 알린 적 없는 교실이 알릴 교실에 들어오거나, 신뢰도 **중 이상**으로 교실이 바뀔 때 | 낮은 신뢰도로 두 교실을 오가는 흔들림은 다시 알리지 않는다. 알린 교실은 누적한다. `score_window_ms`보다 오래 조용했던 기기가 돌아오면 새 알림 |
| 감점 표시 | 알림 문구에 "감점: …" 줄 | 감독관이 점수가 낮은 이유를 본다 |

## 쓰는 필드

센서별 BLE·Wi-Fi RSSI만 쓴다. SDR 대역 전력은 dBFS 기준 상대값이라 교실 경계(dBm)와 눈금이 달라 위치·감점에 쓰지 않는다.

## 가중치

건물 밖 감점 기본 4, 인체 차폐 여유 15dB, 신뢰도 기준 10dB·5dB(설계 §4.7 예시).

## 근거

설계 §4.4, §4.7, [S6][S7][S8].

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 인체 차폐(우리 교실 폰이 약하게) | 벽 감쇠(약 6dB)보다 차폐가 클 수 있다 | 상대 비교, 앞뒤 센서 |
| 낮은 신뢰도 | 인접 교실에도 알림 | `notify_rooms` |

## 경계

신뢰도 경계와 같은 차이는 위쪽 등급. 표본이 짝수 개면 아래쪽 중앙값. 설계 §4.7의 알림 예시(-58/-66/-71 → "신뢰도 중")는 같은 절의 기준(차이 13dB → 높음)과 맞지 않는다 — 기준을 따랐고, 예시는 실측 보정 때 다시 본다.

## 테스트

`given_device_strongest_at_room_3_2_back_when_located_then_room_3_2_back_high`, `given_neighbor_room_device_when_seen_by_three_rooms_then_assigned_to_neighbor_room`, `given_body_shadowed_phone_in_our_room_weaker_than_wall_attenuated_neighbor_when_located_then_our_room_still_wins`, `given_macos_sensors_seeing_same_glasses_when_located_then_confidence_lowered`, `given_body_shadowed_phone_weaker_than_boundary_but_within_shield_margin_when_watched_then_still_alerted`, `given_neighbor_device_first_seen_only_by_our_sensor_when_alerted_then_low_confidence_and_adjacent_rooms_told`, `given_front_and_back_within_five_db_when_located_then_position_omitted`, `given_two_rooms_alternating_by_small_margin_when_watched_then_no_realert_on_room_flip`, `given_storage_zone_when_hidden_wifi_seen_there_then_no_storage_penalty`

## 미검증

신뢰도 기준(1·2위 차이), 한 교실만 본 기기의 신뢰도(`TODO(field-unverified)`, `crates/core/src/locate/mod.rs`), 중앙값 창 길이, 인체 차폐 여유 15dB(문헌 한 건 기준), 센서마다 다른 경계를 그대로 비교하는 순위(경계로 정규화할지는 열린 결정).
