# Wi-Fi — Windows

- 상태: 구현됨(교차 컴파일 검사만, 실기 미확인)
- 단계: Phase 1
- API·크레이트: Native Wifi `WlanScan`, `WlanGetNetworkBssList`(`windows` 0.62)

## 송신 여부

`WlanScan`은 OS 스캔이라 probe request를 보낸다 — 허용한 예외다(AGENTS.md §3.1). SSID·IE를 넘기지 않으므로 우리가 프레임 내용을 정하지 않는다.

## 라이선스

`windows`(MIT OR Apache-2.0).

## 얻는 필드

| API 필드 | `Observation` 필드 | 허용 목록 |
|---|---|---|
| `dot11Bssid` | `bssid` | 사용 |
| `dot11Ssid` | `ssid` | 사용(길이 32로 자름) |
| `lRssi` | `rssi` | 사용 |
| `ulChCenterFrequency` | `band`, `channel` | 사용 |
| IE 블롭 | — | 읽지 않음 |

## 권한

Windows 11 24H2부터 정밀 위치 권한이 없으면 `ERROR_ACCESS_DENIED` — 권한 없음(정밀 위치)으로 보고하고, 앱의 권한 점검 패널에서
`ms-settings:privacy-location`을 연다(설계 §7.1). 기관 PC의 그룹 정책도 확인 대상이다.

## 제약

드라이버가 4초 안에 스캔을 끝내므로 스캔 뒤 4초 기다렸다가 목록을 읽는다. 6GHz는 6E 칩이 필요하다.

## 변환

`collector-wifi::bss::from_windows`.

## 미검증

Windows 10 지원 여부(설계 §10), 24H2 이전 버전의 동작.
