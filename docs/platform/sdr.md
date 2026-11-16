# SDR — HackRF(SoapySDR)

- 상태: 구현됨(`sdr` 기능, 장비 없이 계산만 확인)
- 단계: Phase 3
- API·크레이트·장비: SoapySDR 0.9(`soapysdr` 0.4), HackRF One, `rustfft` 6

## 송신 여부

수신 스트림(`rx_stream`)만 연다. 송신 스트림·`hackrf_start_tx`는 쓰지 않는다(`check_passive.py`가 막는다).

## 라이선스

`soapysdr`(BSL-1.0 OR Apache-2.0), SoapySDR(BSL-1.0), `rustfft`(MIT OR Apache-2.0). HackRF 지원은 SoapyHackRF 모듈을 런타임에 쓴다.

## 얻는 필드

| 장비 출력 | `Observation` 필드 | 허용 목록 |
|---|---|---|
| IQ 표본 → FFT 대역 평균 전력 | `RfObs::power_cdbm`(0.01dBFS, 상대값) | 사용 |
| 심볼·프레임 | — | 만들지 않음 |

## 권한

없음. 장비가 없으면 장비 없음으로 보고한다.

## 제약

20MHz 표본률, 대역폭의 80%씩 홉한다(B1 4홉, B3 5홉, B5 2홉, B7 5홉). dBFS는 센서마다 다르므로 R7은 센서별 기준선과만 비교한다.

## 변환

`collector-sdr::power::{hop_centers, band_power_cdbfs, combine_cdbfs}` — 합성 톤으로 테스트한다.

## 미검증

교실 거리에서 포화하지 않는 이득, 업로드 버스트 상승폭(`field-test`).
