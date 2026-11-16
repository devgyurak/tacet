# R1 — 폰 제조사 BLE 광고

- 상태: 구현됨
- 단계: Phase 1
- 막는 위협: T1, T2 (설계 §3)
- 수집기·플랫폼: BLE 수집기(Windows 수동 스캔, macOS CoreBluetooth)

## 흔적

블루투스가 켜진 폰은 연동·기기 찾기 기능 때문에 광고를 송신한다(설계 §1, [S5a]). 수거·전원 차단이 끝난 교실에서는 보이지 않아야 한다.

## 조건

| 필드 | 조건 | 비고 |
|---|---|---|
| `manufacturer[].company` | `0x004C`(Apple), `0x0075`(Samsung), `0x00E0`(Google) 중 하나 | `rules::PHONE_VENDOR_IDS` |

## 쓰는 필드

제조사 데이터의 회사 ID만 쓴다. 데이터 내용은 기기 상관(지문)에만 쓰고 해석하지 않는다.

## 가중치

기본 4(높음). `config::Weights::r1`.

## 근거

Bluetooth SIG Company Identifiers([S1]), Android `BluetoothAssignedNumbers.GOOGLE`([S2]).

## 오탐 요인

| 요인 | 반응 | 걸러내는 것 |
|---|---|---|
| 수거폰이 꺼지지 않음 | 적중 | 보관 위치 감점 |
| 옆 교실 폰 | 적중 | 위치 판정으로 재배정, 건물 밖이면 감점 |
| 감독관 폰 | 적중 | 화이트리스트(주소) — macOS 센서는 주소가 없어 제조사 데이터 화이트리스트를 쓴다 |
| 랜덤 MAC 교체 | 같은 기기로 묶임 | 기기 상관(같은 제조사 데이터 + RSSI 허용 오차) |

## 경계

회사 ID가 여러 개인 광고는 하나만 맞아도 적중. 기준선 기기는 적중하지 않는다.

## 테스트

`given_each_phone_vendor_id_when_scored_then_no_vendor_misses_r1`, `given_t2_phone_ble_only_when_watched_then_medium_alert_with_r1_and_r2`, `given_collected_phone_left_on_at_storage_spot_when_watched_then_no_alert_is_raised`

## 미검증

가중치 4(`TODO(field-unverified)`, `crates/core/src/config/mod.rs`). 기종별로 화면이 꺼진 상태에서도 광고하는지 실측이 필요하다(`field-test`).
