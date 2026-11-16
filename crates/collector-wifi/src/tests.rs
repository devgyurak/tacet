use std::sync::Arc;

use tacet_core::Timestamp;

use super::*;

#[test]
fn given_scan_interval_when_read_then_several_seconds() {
    assert!(SCAN_INTERVAL >= Duration::from_secs(4));
}

#[cfg(not(any(target_os = "macos", windows)))]
#[tokio::test]
async fn given_linux_when_run_then_unsupported_status() {
    let (tx, mut rx) = mpsc::channel(4);
    run(Arc::new(|| Timestamp(0)), tx).await;
    rx.recv().await;
    assert!(matches!(
        rx.recv().await,
        Some(CollectorEvent::Status(
            CollectorKind::Wifi,
            CollectorStatus::Off(_)
        ))
    ));
}

#[cfg(any(target_os = "macos", windows))]
#[tokio::test]
#[ignore = "실제 장비 필요: Wi-Fi 어댑터와 위치 권한"]
async fn given_real_adapter_when_run_then_first_status_is_starting() {
    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(run(Arc::new(|| Timestamp(0)), tx));
    let first = rx.recv().await;
    task.abort();
    assert_eq!(
        first,
        Some(CollectorEvent::Status(
            CollectorKind::Wifi,
            CollectorStatus::Starting
        ))
    );
}
