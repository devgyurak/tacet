//! SDR 업링크 에너지 수집기(R7, 설계 §4.5).
//!
//! LTE FDD 업링크 대역(B1·B3·B5·B7)의 평균 전력만 잰다. 수신 스트림만 열고 복조·동기화·디코딩을 하지
//! 않는다(AGENTS.md §3.1, §3.2). 전력은 dBFS 기준 상대값이라 같은 센서의 기준선과만 비교한다(R7은 기준선
//! 대비 상승폭이다). 장비 수집은 `sdr` 기능 뒤에 있다.

#[cfg(feature = "sdr")]
mod device;
pub mod power;

use tacet_core::obs::{CollectorEvent, CollectorKind, CollectorStatus, OffReason};
use tacet_core::time::ClockFn;
use tokio::sync::mpsc;

/// 수집기 설정.
#[derive(Debug, Clone)]
pub struct SdrSettings {
    /// SoapySDR 장치 인자(예: `driver=hackrf`).
    pub device_args: String,
    /// 수신 이득(dB).
    // TODO(field-unverified): 교실 거리에서 포화하지 않는 이득은 실측으로 정한다.
    pub gain_db: f64,
}

impl Default for SdrSettings {
    fn default() -> Self {
        Self {
            device_args: "driver=hackrf".to_owned(),
            gain_db: 20.0,
        }
    }
}

/// 수집을 돌린다. 장비가 없거나 기능 없이 빌드됐으면 그 상태를 보내고 끝난다.
pub async fn run(settings: SdrSettings, clock: ClockFn, tx: mpsc::Sender<CollectorEvent>) {
    let _ = tx
        .send(CollectorEvent::Status(
            CollectorKind::Sdr,
            CollectorStatus::Starting,
        ))
        .await;
    #[cfg(feature = "sdr")]
    {
        let blocking_tx = tx.clone();
        let result = tokio::task::spawn_blocking(move || {
            device::run_blocking(&settings, &clock, &blocking_tx)
        })
        .await;
        if let Err(e) = result {
            let reason = OffReason::Error(format!("수집 작업이 멈췄다: {e}"));
            let _ = tx
                .send(CollectorEvent::Status(
                    CollectorKind::Sdr,
                    CollectorStatus::Off(reason),
                ))
                .await;
        }
    }
    #[cfg(not(feature = "sdr"))]
    {
        let _ = (settings, clock);
        let reason = OffReason::Unsupported("sdr 기능 없이 빌드됨".to_owned());
        let _ = tx
            .send(CollectorEvent::Status(
                CollectorKind::Sdr,
                CollectorStatus::Off(reason),
            ))
            .await;
    }
}

#[cfg(test)]
mod tests;
