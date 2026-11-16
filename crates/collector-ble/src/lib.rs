//! BLE 광고 수집기(설계 §5.2, R1·R1a·R1b·R2).
//!
//! - Windows: WinRT `BluetoothLEAdvertisementWatcher`를 **수동(passive) 스캔**으로 쓴다 — 스캔 요청을 보내지
//!   않는다(AGENTS.md §3.1). 실제 주소를 준다.
//! - macOS: CoreBluetooth(btleplug). 수동 스캔이 없어 능동 스캔만 되며, 이는 허용한 예외다(설계 §7.2).
//!   주소 대신 호스트별 UUID를 준다.
//! - 리눅스는 배포 대상이 아니라 지원하지 않는다고 보고한다.
//!
//! 연결·GATT 조회·구독은 하지 않는다. 광고 메타데이터만 쓴다(AGENTS.md §3.2).

pub mod adv;
#[cfg(target_os = "macos")]
mod sys_macos;
#[cfg(windows)]
mod sys_windows;

use tacet_core::obs::{CollectorEvent, CollectorKind, CollectorStatus};
use tacet_core::time::ClockFn;
use tokio::sync::mpsc;

/// 광고 이름을 넘길지. 이름은 교실 보정 목록에만 쓴다(AGENTS.md §3.2) — 감지 중에는 버린다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Names {
    /// 버린다(감지).
    Drop,
    /// 넘긴다(교실 보정).
    Keep,
}

/// 감지용 수집을 돌린다. 광고 이름은 버린다. 어댑터가 없거나 권한이 없으면 그 상태를 보내고 끝난다.
pub async fn run(clock: ClockFn, tx: mpsc::Sender<CollectorEvent>) {
    run_with(clock, tx, Names::Drop).await;
}

/// 교실 보정용 수집을 돌린다. 감독관이 자기 휴대폰을 고를 수 있게 광고 이름을 넘긴다.
pub async fn run_for_calibration(clock: ClockFn, tx: mpsc::Sender<CollectorEvent>) {
    run_with(clock, tx, Names::Keep).await;
}

async fn run_with(clock: ClockFn, tx: mpsc::Sender<CollectorEvent>, names: Names) {
    let _ = tx
        .send(CollectorEvent::Status(
            CollectorKind::Ble,
            CollectorStatus::Starting,
        ))
        .await;
    #[cfg(target_os = "macos")]
    sys_macos::run(clock, tx, names).await;
    #[cfg(windows)]
    sys_windows::run(clock, tx, names).await;
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (clock, names);
        let reason = tacet_core::obs::OffReason::Unsupported("리눅스는 개발·검증용".to_owned());
        let _ = tx
            .send(CollectorEvent::Status(
                CollectorKind::Ble,
                CollectorStatus::Off(reason),
            ))
            .await;
    }
}
