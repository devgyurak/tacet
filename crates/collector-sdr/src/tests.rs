use std::sync::Arc;

use tacet_core::Timestamp;

use super::*;

#[tokio::test]
async fn given_build_when_run_then_starting_status_is_sent_first() {
    let (tx, mut rx) = mpsc::channel(8);
    let settings = SdrSettings {
        device_args: "driver=없는장치".to_owned(),
        ..SdrSettings::default()
    };
    run(settings, Arc::new(|| Timestamp(0)), tx).await;
    assert_eq!(
        rx.recv().await,
        Some(CollectorEvent::Status(
            CollectorKind::Sdr,
            CollectorStatus::Starting
        ))
    );
}

#[tokio::test]
async fn given_missing_device_or_feature_when_run_then_collector_reports_off() {
    let (tx, mut rx) = mpsc::channel(8);
    let settings = SdrSettings {
        device_args: "driver=없는장치".to_owned(),
        ..SdrSettings::default()
    };
    run(settings, Arc::new(|| Timestamp(0)), tx).await;
    rx.recv().await;
    assert!(matches!(
        rx.recv().await,
        Some(CollectorEvent::Status(
            CollectorKind::Sdr,
            CollectorStatus::Off(_)
        ))
    ));
}

#[test]
fn given_default_settings_when_read_then_hackrf_driver() {
    assert_eq!(SdrSettings::default().device_args, "driver=hackrf");
}
