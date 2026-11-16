use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tacet_core::obs::{OffReason, Permission};
use tacet_core::testkit::{ble, doc_mac, wifi};
use tacet_core::time::Timestamp;

use super::*;

/// 시험이 정하는 시계(ms).
fn clock() -> (Arc<AtomicU64>, ClockFn) {
    let now = Arc::new(AtomicU64::new(0));
    let reader = now.clone();
    (
        now,
        Arc::new(move || Timestamp(reader.load(Ordering::SeqCst))),
    )
}

/// 출력 줄 중 보정 진행 상황만 (단계, 블루투스 경계, 블루투스 문제 유무)로.
fn stages(out: &[u8]) -> Vec<(String, Option<i64>, bool)> {
    String::from_utf8_lossy(out)
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["type"] == "calibration")
        .map(|v| {
            (
                v["progress"]["stage"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                v["progress"]["bluetooth_dbm"].as_i64(),
                !v["bluetooth_problem"].is_null(),
            )
        })
        .collect()
}

/// 기기 `addr`의 광고를 `from_secs`부터 1초 간격으로 보낸다.
async fn send_ble(tx: &mpsc::Sender<CollectorEvent>, addr: u8, from_secs: u64, rssi: &[i16]) {
    for (i, value) in rssi.iter().enumerate() {
        let obs = ble(addr)
            .vendor(0x004C, &[1])
            .rssi(*value)
            .at_secs(from_secs + i as u64)
            .build();
        let _ = tx.send(CollectorEvent::Obs(obs)).await;
    }
}

/// 와이파이 `addr`의 비콘을 `from_secs`부터 8초 간격으로 보낸다.
async fn send_wifi(tx: &mpsc::Sender<CollectorEvent>, addr: u8, from_secs: u64, rssi: &[i16]) {
    for (i, value) in rssi.iter().enumerate() {
        let obs = wifi(doc_mac(addr))
            .ssid("시험 핫스팟")
            .rssi(*value)
            .at_secs(from_secs + 8 * i as u64)
            .build();
        let _ = tx.send(CollectorEvent::Obs(obs)).await;
    }
}

async fn settle() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}

#[tokio::test(start_paused = true)]
async fn given_phone_and_hotspot_selected_then_measured_at_edge_when_run_then_done_with_both_boundaries()
 {
    let (now, clock) = clock();
    let (event_tx, event_rx) = mpsc::channel(64);
    let (command_tx, command_rx) = mpsc::channel(8);
    let task = tokio::spawn(async move {
        let mut out = Vec::new();
        let _ = run(event_rx, command_rx, clock, &mut out).await;
        out
    });
    // 첫 블루투스 기기는 b1, 그다음 처음 보는 와이파이는 w2가 된다.
    send_ble(&event_tx, 1, 0, &[-41, -40, -39]).await;
    send_wifi(&event_tx, 9, 0, &[-35, -35]).await;
    settle().await;
    now.store(10_000, Ordering::SeqCst);
    let select = Command::CalibrateSelect {
        bluetooth: "b1".to_owned(),
        wifi: "w2".to_owned(),
    };
    let _ = command_tx.send(select).await;
    settle().await;
    now.store(12_000, Ordering::SeqCst);
    let _ = command_tx.send(Command::CalibrateFar).await;
    settle().await;
    send_ble(&event_tx, 1, 12, &[-79, -77, -78, -76]).await;
    send_wifi(&event_tx, 9, 12, &[-60, -61, -62]).await;
    settle().await;
    now.store(40_000, Ordering::SeqCst);
    settle().await;
    let _ = command_tx.send(Command::Shutdown).await;
    let out = task.await.unwrap_or_default();
    let seen = stages(&out);
    let last_wifi = String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v["progress"]["wifi_dbm"].as_i64())
        .next_back();
    assert_eq!(
        (
            seen.iter().any(|s| s.0 == "ready_for_far"),
            seen.last().cloned(),
            last_wifi
        ),
        (true, Some(("done".to_owned(), Some(-78), false)), Some(-61))
    );
}

#[tokio::test(start_paused = true)]
async fn given_bluetooth_permission_denied_when_run_then_progress_carries_plain_bluetooth_problem()
{
    let (_now, clock) = clock();
    let (event_tx, event_rx) = mpsc::channel(8);
    let (command_tx, command_rx) = mpsc::channel(8);
    let task = tokio::spawn(async move {
        let mut out = Vec::new();
        let _ = run(event_rx, command_rx, clock, &mut out).await;
        out
    });
    let off = CollectorStatus::Off(OffReason::PermissionDenied(Permission::Bluetooth));
    let _ = event_tx
        .send(CollectorEvent::Status(CollectorKind::Ble, off))
        .await;
    settle().await;
    drop(command_tx);
    let out = task.await.unwrap_or_default();
    assert_eq!(stages(&out).last().map(|s| s.2), Some(true));
}

#[tokio::test(start_paused = true)]
async fn given_commands_channel_closed_when_run_then_returns() {
    let (_now, clock) = clock();
    let (_event_tx, event_rx) = mpsc::channel::<CollectorEvent>(1);
    let (command_tx, command_rx) = mpsc::channel(1);
    drop(command_tx);
    let mut out = Vec::new();
    assert!(run(event_rx, command_rx, clock, &mut out).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn given_bluetooth_collector_ends_when_run_then_progress_reports_bluetooth_stopped() {
    let (_now, clock) = clock();
    let (event_tx, event_rx) = mpsc::channel::<CollectorEvent>(1);
    let (command_tx, command_rx) = mpsc::channel(8);
    let task = tokio::spawn(async move {
        let mut out = Vec::new();
        let _ = run(event_rx, command_rx, clock, &mut out).await;
        out
    });
    drop(event_tx);
    settle().await;
    let _ = command_tx.send(Command::Shutdown).await;
    let out = task.await.unwrap_or_default();
    assert_eq!(stages(&out).last().map(|s| s.2), Some(true));
}
