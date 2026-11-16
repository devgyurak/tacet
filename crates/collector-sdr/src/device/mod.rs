//! SoapySDR 장비 어댑터. 수신 스트림만 연다 — 송신 스트림은 만들지 않는다(AGENTS.md §3.1).

use rustfft::num_complex::Complex;
use soapysdr::{Device, Direction};
use tacet_core::obs::{
    CellBand, CollectorEvent, CollectorKind, CollectorStatus, Observation, OffReason, RfObs,
};
use tacet_core::time::ClockFn;
use tokio::sync::mpsc;

use crate::SdrSettings;
use crate::power::{FFT_SIZE, band_power_cdbfs, combine_cdbfs, hop_centers};

/// 표본률. HackRF가 무리 없이 내는 값.
const SAMPLE_RATE: f64 = 20e6;
/// 홉마다 읽는 표본 수(프레임 16개).
const READ_LEN: usize = FFT_SIZE * 16;
/// 읽기 시간 제한(µs).
const READ_TIMEOUT_US: i64 = 1_000_000;
/// 모든 대역에서 전력을 하나도 못 잰 순회가 이만큼 이어지면 장비가 빠졌거나 멈춘 것으로 보고 꺼짐을 알린다.
const MAX_EMPTY_SWEEPS: u32 = 3;

fn off(tx: &mpsc::Sender<CollectorEvent>, reason: OffReason) {
    let _ = tx.blocking_send(CollectorEvent::Status(
        CollectorKind::Sdr,
        CollectorStatus::Off(reason),
    ));
}

/// 대역을 차례로 돌며 전력을 보낸다. 받는 쪽이 닫히면 끝난다.
pub(crate) fn run_blocking(
    settings: &SdrSettings,
    clock: &ClockFn,
    tx: &mpsc::Sender<CollectorEvent>,
) {
    let device = match Device::new(settings.device_args.as_str()) {
        Ok(device) => device,
        // 장치 목록에 없으면 장비 없음, 있는데 못 열었으면 그 오류(드라이버·권한·다른 프로그램이 점유).
        Err(e) => {
            let found = soapysdr::enumerate(settings.device_args.as_str())
                .is_ok_and(|list| !list.is_empty());
            let reason = if found {
                OffReason::Error(e.to_string())
            } else {
                OffReason::DeviceMissing
            };
            return off(tx, reason);
        }
    };
    let setup = device
        .set_sample_rate(Direction::Rx, 0, SAMPLE_RATE)
        .and_then(|()| device.set_gain(Direction::Rx, 0, settings.gain_db))
        .and_then(|()| device.rx_stream::<Complex<f32>>(&[0]));
    let mut stream = match setup {
        Ok(stream) => stream,
        Err(e) => return off(tx, OffReason::Error(e.to_string())),
    };
    if let Err(e) = stream.activate(None) {
        return off(tx, OffReason::Error(e.to_string()));
    }
    if tx
        .blocking_send(CollectorEvent::Status(
            CollectorKind::Sdr,
            CollectorStatus::Running,
        ))
        .is_err()
    {
        return;
    }
    let mut buf = vec![Complex::new(0.0f32, 0.0); READ_LEN];
    let mut empty_sweeps = 0u32;
    loop {
        let mut measured = false;
        for band in CellBand::ALL {
            let (lo_khz, hi_khz) = band.uplink_khz();
            let (lo, hi) = (f64::from(lo_khz) * 1e3, f64::from(hi_khz) * 1e3);
            let mut powers = Vec::new();
            for center in hop_centers(lo, hi, SAMPLE_RATE) {
                if device.set_frequency(Direction::Rx, 0, center, ()).is_err() {
                    continue;
                }
                // 재튜닝 직후 표본은 버린다.
                let _ = stream.read(&mut [&mut buf[..]], READ_TIMEOUT_US);
                let Ok(n) = stream.read(&mut [&mut buf[..]], READ_TIMEOUT_US) else {
                    continue;
                };
                if let Some(p) = band_power_cdbfs(&buf[..n], SAMPLE_RATE, center, lo, hi) {
                    powers.push(p);
                }
            }
            if let Some(power_cdbm) = combine_cdbfs(&powers) {
                measured = true;
                let obs = Observation::Rf(RfObs {
                    band,
                    power_cdbm,
                    at: clock(),
                });
                if tx.blocking_send(CollectorEvent::Obs(obs)).is_err() {
                    let _ = stream.deactivate(None);
                    return;
                }
            }
        }
        empty_sweeps = if measured { 0 } else { empty_sweeps + 1 };
        if empty_sweeps >= MAX_EMPTY_SWEEPS {
            let _ = stream.deactivate(None);
            return off(
                tx,
                OffReason::Error("장비에서 표본을 읽지 못한다 — 연결을 확인한다".to_owned()),
            );
        }
    }
}
