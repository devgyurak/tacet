# BLE — macOS

- 상태: 구현됨(컴파일·실행 확인, 실기 탐지 미확인)
- 단계: Phase 1
- API·크레이트: CoreBluetooth(`btleplug` 0.11)

## 송신 여부

CoreBluetooth는 수동 스캔을 지원하지 않아 **능동 스캔만** 된다([S41]). 스캔 요청을 보내는데, 일반 블루투스 기기의 정상 동작으로
보고 허용한 예외다(설계 §7.2, §8, AGENTS.md §3.1 — 법률 검토 대상). 연결·GATT 조회는 하지 않는다.

## 라이선스

`btleplug`(MIT/Apache-2.0/BSD-3-Clause), `uuid`(MIT OR Apache-2.0).

## 얻는 필드

| API 필드 | `Observation` 필드 | 허용 목록 |
|---|---|---|
| `PeripheralId`(호스트별 UUID) | `BleId::HostLocal` | 사용 — 다른 센서와 비교하지 않는다 |
| `rssi`, `manufacturer_data`, `services`, `service_data` | 같은 이름 | 사용 |
| `local_name` | — | 받지만 버림 |

## 권한

앱 번들의 `NSBluetoothAlwaysUsageDescription`과 사용자 허용이 필요하다(`desktop/package.json`의 `build.mac.extendInfo`). 거부되면 권한 없음으로 보고한다.

## 제약

주소가 없어 다교실 상관은 같은 시간 창 + 같은 제조사 데이터로만 하고 위치 신뢰도를 한 단계 낮춘다(`docs/rules/L-room-locator.md`).

## 변환

`collector-ble::adv::{uuid_from_text, to_obs}`.

## 미검증

서명된 앱 번들에서 블루투스 권한 요청이 뜨는지, 터미널 실행 시 동작(`field-test`).
