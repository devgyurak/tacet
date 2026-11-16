# Wi-Fi — macOS

- 상태: 구현됨(컴파일 확인, 실기 미확인)
- 단계: Phase 1
- API·크레이트: CoreWLAN `scanForNetworksWithName:error:`(`objc2-core-wlan` 0.3), CoreLocation

## 송신 여부

OS 스캔이라 probe request를 보낸다 — 허용한 예외다. 이름을 넘기지 않는다(방향성 스캔 안 함).

## 라이선스

`objc2-*`(Zlib OR Apache-2.0 OR MIT).

## 얻는 필드

| API 필드 | `Observation` 필드 | 허용 목록 |
|---|---|---|
| `bssid` | `bssid` | 사용 — 비어 있으면 관측을 버린다 |
| `ssidData` | `ssid` | 사용 |
| `rssiValue` | `rssi` | 사용 |
| `wlanChannel` | `channel`, `band` | 사용 |

## 권한

위치 권한 + 안정적인 Developer ID 서명 + 사용자 세션이 있어야 BSSID가 온다([S36][S37]). 네트워크가 보이는데 모두 BSSID가
비어 있으면 위치 권한 없음으로 보고한다(`bss::corewlan_status`). 앱이 메인 스레드에서 `requestWhenInUseAuthorization`을 부른다.

## 제약

macOS 15.1에서 권한을 줘도 비어 나온다는 보고가 있다([S38]). 안 되면 macOS는 BLE 센서로만 쓴다(설계 §7.2).

## 변환

`collector-wifi::bss::{from_corewlan, band_from_corewlan, corewlan_status}`.

## 미검증

macOS 14 / 15 / 26의 BSSID 수신(설계 §10 — `field-test`).
