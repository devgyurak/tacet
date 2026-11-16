# BLE — Windows

- 상태: 구현됨(교차 컴파일 검사만, 실기 미확인)
- 단계: Phase 1
- API·크레이트: WinRT `Windows.Devices.Bluetooth.Advertisement.BluetoothLEAdvertisementWatcher`(`windows` 0.62)

## 송신 여부

`SetScanningMode(Passive)`로 **수동 스캔**한다 — 스캔 요청을 보내지 않는다(AGENTS.md §3.1). 연결·GATT 조회는 하지 않는다.
btleplug의 Windows 백엔드는 능동 스캔을 쓰므로 Windows에서는 btleplug을 쓰지 않는다.

## 라이선스

`windows` 크레이트(MIT OR Apache-2.0). C 라이브러리 링크 없음.

## 얻는 필드

| API 필드 | `Observation` 필드 | 허용 목록 |
|---|---|---|
| `BluetoothAddress` | `BleId::Addr` | 사용 |
| `RawSignalStrengthInDBm` | `rssi` | 사용 |
| `DataSections`(0x02·0x03·0x06·0x07·0x16·0xFF) | `services`, `service_data`, `manufacturer` | 사용(`adv::apply_section`) |
| 그 밖의 구간(이름 등) | — | 받지만 버림 |

## 권한

블루투스가 꺼져 있거나 사용자가 막으면 감시기가 `Stopped`(오류 `DisabledByUser`·`ConsentRequired`·`RadioNotAvailable`)로
멈춘다. 각각 권한 없음·장비 없음으로 보고한다.

## 제약

수동 스캔은 스캔 응답(scan response)을 받지 못한다 — 제조사 데이터가 스캔 응답에만 있는 기기는 R1을 놓칠 수 있다.

## 변환

`collector-ble::adv::{addr_from_u64, apply_section, to_obs}`. 합성 AD 바이트로 테스트한다.

## 미검증

- 실제 Windows 11 노트북에서 수동 스캔으로 폰 제조사 데이터가 충분히 들어오는지(`field-test`).
- 감시기 콜백 스레드에서 `try_send`가 밀릴 때 버려지는 비율.
