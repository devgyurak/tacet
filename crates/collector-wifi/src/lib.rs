//! Wi-Fi 관리 모드 스캔 수집기(설계 §5.2, R3–R6).
//!
//! - Windows: WLAN API(`WlanScan`, `WlanGetNetworkBssList`). 24H2부터 **정밀 위치 권한**이 있어야 BSSID를
//!   준다(설계 §7.1). 거부되면 그 상태를 보고한다.
//! - macOS: CoreWLAN. **위치 권한과 서명된 앱**이어야 BSSID를 준다(설계 §7.2). BSSID가 비어 오면 권한이 없는
//!   것으로 보고한다.
//! - OS 스캔이 내보내는 probe request는 허용한 예외다(AGENTS.md §3.1). 우리 코드는 프레임을 만들지 않는다.
//!
//! 비콘 헤더(BSSID, SSID, 채널, RSSI)만 쓴다(AGENTS.md §3.2).

pub mod bss;
#[cfg(target_os = "macos")]
mod sys_macos;
#[cfg(windows)]
mod sys_windows;

use std::time::Duration;

use tacet_core::obs::{CollectorEvent, CollectorKind, CollectorStatus};
use tacet_core::time::ClockFn;
use tokio::sync::mpsc;

/// 스캔 간격. OS가 수 초 단위로 스캔을 제한한다(설계 §7).
pub const SCAN_INTERVAL: Duration = Duration::from_secs(8);

/// 수집을 돌린다.
pub async fn run(clock: ClockFn, tx: mpsc::Sender<CollectorEvent>) {
    let _ = tx
        .send(CollectorEvent::Status(
            CollectorKind::Wifi,
            CollectorStatus::Starting,
        ))
        .await;
    #[cfg(any(target_os = "macos", windows))]
    {
        #[cfg(target_os = "macos")]
        let scan = sys_macos::scan_loop;
        #[cfg(windows)]
        let scan = sys_windows::scan_loop;
        let blocking_tx = tx.clone();
        let result = tokio::task::spawn_blocking(move || scan(&clock, &blocking_tx)).await;
        if let Err(e) = result {
            let reason = tacet_core::obs::OffReason::Error(format!("수집 작업이 멈췄다: {e}"));
            let _ = tx
                .send(CollectorEvent::Status(
                    CollectorKind::Wifi,
                    CollectorStatus::Off(reason),
                ))
                .await;
        }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = clock;
        let reason = tacet_core::obs::OffReason::Unsupported("리눅스는 개발·검증용".to_owned());
        let _ = tx
            .send(CollectorEvent::Status(
                CollectorKind::Wifi,
                CollectorStatus::Off(reason),
            ))
            .await;
    }
}

/// macOS 위치 권한을 요청한다. 앱의 메인 스레드에서 부른다(권한 온보딩). 다른 OS에서는 아무 일도 하지 않는다.
pub fn request_location_permission() {
    #[cfg(target_os = "macos")]
    sys_macos::request_location_permission();
}

#[cfg(test)]
mod tests;
