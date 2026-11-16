use tacet_core::Timestamp;
use tacet_core::config::Whitelist;
use tacet_core::ids::SensorId;
use tacet_core::obs::{CollectorKind, CollectorStatus, OffReason};
use tacet_core::report::SessionSalt;
use tacet_core::testkit::{hidden_wifi, quick_config, report, single_room};

use super::*;

fn hub() -> Hub {
    Hub::new(
        quick_config(),
        single_room(Some(-80)),
        Whitelist::default(),
        SessionSalt([1; 32]),
    )
}

#[test]
fn given_local_hidden_wifi_after_baseline_when_ingested_then_alert_logged_and_reported_hashed() {
    let mut h = hub();
    h.tick(Timestamp::from_secs(11));
    let alerts = h.ingest_local(&SensorId::new("solo"), hidden_wifi(0x2A, -60, 20));
    let entry_hidden = h.report().entries.first().map(|e| !e.device.contains("53"));
    assert_eq!(
        (alerts.len(), h.log().to_vec(), entry_hidden),
        (
            1,
            vec!["[확인 필요] 숨긴 Wi-Fi 감지 (BSSID 00:00:5E:00:53:2A)".to_owned()],
            Some(true)
        )
    );
}

#[test]
fn given_statuses_and_connection_when_viewed_then_sensor_lines_present() {
    let mut h = hub();
    let s = SensorId::new("solo");
    h.set_connected(&s, false);
    h.set_status(
        &s,
        CollectorKind::Wifi,
        CollectorStatus::Off(OffReason::DeviceMissing),
    );
    let view = h.view();
    assert_eq!(
        (
            view.sensors[0].connected,
            view.sensors[0]
                .collectors
                .iter()
                .map(|c| c.technical.clone())
                .collect::<Vec<_>>()
        ),
        (true, vec!["Wi-Fi 수집: 꺼짐 — 장비 없음".to_owned()])
    );
}

#[test]
fn given_disconnected_sensor_when_viewed_then_marked_disconnected() {
    let mut h = hub();
    h.set_connected(&SensorId::new("x"), false);
    assert!(!h.view().sensors[0].connected);
}

fn ingest_all(h: &mut Hub, reports: &[tacet_core::obs::SensorReport]) {
    for r in reports {
        h.ingest(r);
    }
}

#[test]
fn given_many_alerts_when_logged_then_log_capped() {
    let mut h = hub();
    h.tick(Timestamp::from_secs(11));
    let reports: Vec<_> = (0..250u32)
        .map(|i| report("solo", u64::from(i), hidden_wifi((i % 250) as u8, -60, 20)))
        .collect();
    ingest_all(&mut h, &reports);
    assert_eq!(h.log().len(), 200);
}

#[test]
fn given_reports_received_out_of_order_when_ticked_past_delay_then_judged_in_time_order() {
    let mut h = hub();
    h.receive(report("solo", 2, hidden_wifi(1, -60, 21)));
    h.receive(report("solo", 1, hidden_wifi(1, -60, 20)));
    let early = h.tick(Timestamp::from_secs(21));
    let later = h.tick(Timestamp::from_secs(30));
    assert_eq!(
        (early.len(), later.iter().map(|a| a.at).collect::<Vec<_>>()),
        (0, vec![Timestamp::from_secs(20)])
    );
}
