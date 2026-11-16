use super::*;
use crate::config::{RoomLayout, Whitelist, WhitelistEntry};
use crate::engine::Penalty;
use crate::obs::CellBand;
use crate::testkit::{
    ble, doc_mac, doc_random_mac, hidden_wifi, quick_config, report, rf, run, secs, single_room,
    three_rooms, wifi, with_storage,
};

fn engine(layout: RoomLayout) -> Engine {
    Engine::new(quick_config(), layout, Whitelist::default())
}

fn whitelisted(entry: WhitelistEntry) -> Engine {
    Engine::new(
        quick_config(),
        single_room(Some(-80)),
        Whitelist {
            entries: vec![entry],
        },
    )
}

#[test]
fn given_new_engine_when_queried_then_baseline_phase_with_full_remaining_time() {
    let e = engine(single_room(Some(-80)));
    assert_eq!(
        (e.phase(), e.baseline_remaining_ms(), e.baseline_devices()),
        (Phase::Baseline, 10_000, 0)
    );
}

#[test]
fn given_tick_past_baseline_when_queried_then_watching_and_time_never_goes_back() {
    let mut e = engine(single_room(Some(-80)));
    e.tick(secs(11));
    e.tick(secs(2));
    assert_eq!((e.phase(), e.baseline_remaining_ms()), (Phase::Watching, 0));
}

#[test]
fn given_device_seen_during_baseline_when_seen_again_later_then_no_alert() {
    let mut e = engine(single_room(Some(-80)));
    let early = report(
        "solo",
        1,
        wifi(doc_random_mac(1)).ssid_bytes(&[]).at_secs(1).build(),
    );
    let late = report(
        "solo",
        2,
        wifi(doc_random_mac(1)).ssid_bytes(&[]).at_secs(20).build(),
    );
    assert_eq!(
        (run(&mut e, &[early, late]), e.baseline_devices()),
        (vec![], 1)
    );
}

#[test]
fn given_report_from_unknown_sensor_when_ingested_then_dropped_with_warning() {
    let mut e = engine(single_room(Some(-80)));
    let alerts = e.ingest(&report("x", 1, hidden_wifi(1, -50, 20)));
    assert_eq!(
        (alerts, e.warnings()),
        (
            vec![],
            vec![EngineWarning::UnknownSensor(SensorId::new("x"))]
        )
    );
}

#[test]
fn given_uncalibrated_sensor_when_ingested_then_calibration_warning() {
    let mut e = engine(single_room(None));
    e.ingest(&report("solo", 1, ble(1).at_secs(1).build()));
    assert_eq!(
        e.warnings(),
        vec![EngineWarning::CalibrationMissing(SensorId::new("solo"))]
    );
}

#[test]
fn given_whitelisted_bssid_and_vendor_when_watched_then_no_alerts() {
    let alerts = [
        run(
            &mut whitelisted(WhitelistEntry::Mac(doc_random_mac(1))),
            &[report(
                "solo",
                1,
                wifi(doc_random_mac(1)).ssid_bytes(&[]).at_secs(20).build(),
            )],
        ),
        run(
            &mut whitelisted(WhitelistEntry::Manufacturer {
                company: 0x004C,
                prefix: vec![0x12],
            }),
            &[report(
                "solo",
                1,
                ble(3)
                    .vendor(0x004C, &[0x12, 0x34])
                    .rssi(-50)
                    .at_secs(20)
                    .build(),
            )],
        ),
        run(
            &mut whitelisted(WhitelistEntry::Mac(doc_random_mac(4))),
            &[report(
                "solo",
                1,
                ble(4).vendor(0x01AB, &[1]).at_secs(20).build(),
            )],
        ),
    ];
    assert_eq!(alerts.iter().map(Vec::len).sum::<usize>(), 0);
}

#[test]
fn given_whitelist_for_other_device_when_watched_then_alert_still_raised() {
    let mut e = whitelisted(WhitelistEntry::Mac(doc_mac(2)));
    assert_eq!(
        run(&mut e, &[report("solo", 1, hidden_wifi(1, -50, 20))]).len(),
        1
    );
}

#[test]
fn given_new_hidden_wifi_after_baseline_when_watched_then_medium_alert_for_single_room() {
    let mut e = engine(single_room(Some(-80)));
    let alerts = run(&mut e, &[report("solo", 1, hidden_wifi(0x2A, -60, 20))]);
    let summary: Vec<(Level, Vec<RuleId>, Option<RoomId>)> = alerts
        .iter()
        .map(|a| {
            (
                a.level,
                a.rules.clone(),
                a.localization.as_ref().and_then(|l| l.room.clone()),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![(
            Level::Medium,
            vec![RuleId::R3, RuleId::R5],
            Some(RoomId::new("3-2"))
        )]
    );
}

#[test]
fn given_same_level_repeated_when_watched_then_alert_emitted_once() {
    let mut e = engine(single_room(Some(-80)));
    let reports: Vec<_> = (20..25)
        .map(|s| report("solo", s, hidden_wifi(1, -60, s)))
        .collect();
    assert_eq!(run(&mut e, &reports).len(), 1);
}

#[test]
fn given_phone_then_hotspot_rules_accumulate_when_watched_then_level_rises() {
    let mut e = engine(single_room(Some(-80)));
    let bssid = doc_mac(9);
    let reports = [
        report(
            "solo",
            1,
            wifi(bssid).ssid("DIRECT-ab").rssi(-70).at_secs(20).build(),
        ),
        report(
            "solo",
            2,
            wifi(bssid).ssid_bytes(&[]).rssi(-60).at_secs(21).build(),
        ),
    ];
    let levels: Vec<Level> = run(&mut e, &reports).iter().map(|a| a.level).collect();
    assert_eq!(levels, vec![Level::Medium, Level::High]);
}

#[test]
fn given_device_quiet_longer_than_score_window_when_seen_again_then_fresh_alert_with_recent_rules_only()
 {
    let mut cfg = quick_config();
    cfg.score_window_ms = 5_000;
    let mut e = Engine::new(cfg, single_room(Some(-80)), Whitelist::default());
    let bssid = doc_random_mac(9);
    let first = run(
        &mut e,
        &[report(
            "solo",
            1,
            wifi(bssid).ssid("DIRECT-a").rssi(-90).at_secs(20).build(),
        )],
    );
    let later = e.ingest(&report(
        "solo",
        2,
        wifi(bssid).ssid("일반").rssi(-90).at_secs(60).build(),
    ));
    let later_rules: Vec<Vec<RuleId>> = later.iter().map(|a| a.rules.clone()).collect();
    assert_eq!((first.len(), later_rules), (1, vec![vec![RuleId::R4]]));
}

#[test]
fn given_signal_in_storage_zone_when_watched_then_penalty_suppresses_alert() {
    let mut e = engine(with_storage(single_room(Some(-80)), "solo", -50, -40));
    let alerts = run(
        &mut e,
        &[report(
            "solo",
            1,
            ble(1).vendor(0x004C, &[1]).rssi(-45).at_secs(20).build(),
        )],
    );
    assert_eq!(alerts, vec![]);
}

#[test]
fn given_signal_outside_storage_band_when_watched_then_alert_raised() {
    let mut e = engine(with_storage(single_room(Some(-80)), "solo", -50, -40));
    let alerts = run(
        &mut e,
        &[report(
            "solo",
            1,
            ble(1).vendor(0x004C, &[1]).rssi(-70).at_secs(20).build(),
        )],
    );
    assert_eq!(
        alerts.iter().map(|a| a.rules.clone()).collect::<Vec<_>>(),
        vec![vec![RuleId::R1, RuleId::R2]]
    );
}

#[test]
fn given_rf_burst_over_sensor_baseline_when_watched_then_r7_alert() {
    let mut e = engine(single_room(Some(-80)));
    let reports = [
        report("solo", 1, rf(CellBand::B3, -70, 1)),
        report("solo", 2, rf(CellBand::B3, -70, 2)),
        report("solo", 3, rf(CellBand::B3, -55, 20)),
    ];
    let alerts = run(&mut e, &reports);
    assert_eq!(
        alerts.iter().map(|a| a.rules.clone()).collect::<Vec<_>>(),
        vec![vec![RuleId::R7]]
    );
}

#[test]
fn given_low_confidence_room_when_alerted_then_adjacent_rooms_notified() {
    let mut e = engine(three_rooms());
    let reports = [
        report("3-2-앞", 1, hidden_wifi(1, -60, 20)),
        report("3-1-앞", 2, hidden_wifi(1, -62, 20)),
    ];
    let alerts = run(&mut e, &reports);
    let last = alerts.last().map(|a| {
        (
            a.localization.as_ref().map(|l| l.confidence),
            a.notify_rooms.clone(),
        )
    });
    assert_eq!(
        last,
        Some((
            Some(Confidence::Low),
            vec![RoomId::new("3-1"), RoomId::new("3-2"), RoomId::new("3-3")]
        ))
    );
}

#[test]
fn given_device_moves_to_other_room_when_watched_then_new_alert_for_new_room() {
    let mut e = engine(three_rooms());
    let first = run(&mut e, &[report("3-1-앞", 1, hidden_wifi(1, -50, 20))]);
    let moved = run(&mut e, &[report("3-3-뒤", 2, hidden_wifi(1, -40, 21))]);
    let rooms: Vec<Option<RoomId>> = first
        .iter()
        .chain(&moved)
        .map(|a| a.localization.as_ref().and_then(|l| l.room.clone()))
        .collect();
    assert_eq!(
        rooms,
        vec![Some(RoomId::new("3-1")), Some(RoomId::new("3-3"))]
    );
}

#[test]
fn given_rf_burst_without_dbm_strength_when_watched_then_alert_without_localization_notifies_everyone()
 {
    let mut e = engine(three_rooms());
    let alerts = run(
        &mut e,
        &[
            report("3-1-앞", 1, rf(CellBand::B3, -70, 1)),
            report("3-1-앞", 2, rf(CellBand::B3, -70, 2)),
            report("3-1-앞", 3, rf(CellBand::B3, -55, 20)),
        ],
    );
    assert_eq!(
        alerts
            .iter()
            .map(|a| (a.localization.is_none(), a.notify_rooms.is_empty()))
            .collect::<Vec<_>>(),
        vec![(true, true)]
    );
}

#[test]
fn given_engine_when_config_and_layout_read_then_same_as_given() {
    let e = engine(single_room(Some(-80)));
    assert_eq!(
        (e.config().baseline_ms, e.layout().rooms.len()),
        (10_000, 1)
    );
}

#[test]
fn given_wifi_device_label_when_alerted_then_bssid_kept_for_supervisor() {
    let mut e = engine(single_room(Some(-80)));
    let alerts = run(
        &mut e,
        &[report(
            "solo",
            1,
            wifi(doc_mac(5))
                .ssid_bytes(&[])
                .rssi(-60)
                .at_secs(20)
                .build(),
        )],
    );
    assert_eq!(
        alerts.first().map(|a| a.label.clone()),
        Some(DeviceLabel::Wifi {
            bssid: doc_mac(5),
            hidden: true,
            direct: false
        })
    );
}

#[test]
fn given_storage_zone_when_hidden_wifi_seen_there_then_no_storage_penalty() {
    let mut e = engine(with_storage(single_room(Some(-80)), "solo", -50, -40));
    let alerts = run(&mut e, &[report("solo", 1, hidden_wifi(1, -45, 20))]);
    assert_eq!(
        alerts
            .iter()
            .map(|a| (a.score, a.penalties.clone()))
            .collect::<Vec<_>>(),
        vec![(6, vec![])]
    );
}

#[test]
fn given_storage_zone_when_phone_and_hotspot_rules_mix_then_only_phone_points_are_reduced() {
    let mut e = engine(with_storage(single_room(Some(-80)), "solo", -50, -40));
    let alerts = run(
        &mut e,
        &[report(
            "solo",
            1,
            ble(1)
                .vendor(0x004C, &[1])
                .vendor(0x01AB, &[2])
                .rssi(-45)
                .at_secs(20)
                .build(),
        )],
    );
    assert_eq!(
        alerts
            .iter()
            .map(|a| (a.score, a.penalties.clone()))
            .collect::<Vec<_>>(),
        vec![(8, vec![Penalty::StorageZone])]
    );
}

#[test]
fn given_late_report_from_baseline_period_when_ingested_after_baseline_then_still_baseline() {
    let mut e = engine(single_room(Some(-80)));
    let after = e.ingest(&report("solo", 1, hidden_wifi(9, -60, 30)));
    let late_baseline = e.ingest(&report("solo", 2, hidden_wifi(1, -60, 9)));
    let again = e.ingest(&report("solo", 3, hidden_wifi(1, -60, 31)));
    assert_eq!((after.len(), late_baseline.len(), again.len()), (1, 0, 0));
}

#[test]
fn given_report_exactly_at_baseline_end_when_ingested_then_watched() {
    let mut e = engine(single_room(Some(-80)));
    assert_eq!(
        e.ingest(&report("solo", 1, hidden_wifi(1, -60, 10))).len(),
        1
    );
}

#[test]
fn given_two_rooms_alternating_by_small_margin_when_watched_then_no_realert_on_room_flip() {
    let mut e = engine(three_rooms());
    let reports = [
        report("3-1-앞", 1, hidden_wifi(1, -60, 20)),
        report("3-2-앞", 2, hidden_wifi(1, -62, 20)),
        report("3-2-앞", 3, hidden_wifi(1, -58, 21)),
        report("3-1-앞", 4, hidden_wifi(1, -57, 22)),
    ];
    let alerts = run(&mut e, &reports);
    assert_eq!(alerts.len(), 1);
}

#[test]
fn given_macos_ble_and_mac_whitelist_when_ingested_then_whitelist_limit_warning() {
    let mut e = Engine::new(
        quick_config(),
        single_room(Some(-80)),
        Whitelist {
            entries: vec![WhitelistEntry::Mac(doc_mac(1))],
        },
    );
    e.ingest(&report("solo", 1, ble(0).host_local(3).at_secs(1).build()));
    assert_eq!(
        e.warnings(),
        vec![EngineWarning::MacWhitelistNotApplicable(SensorId::new(
            "solo"
        ))]
    );
}

#[test]
fn given_uncalibrated_sensor_when_only_rf_seen_then_no_calibration_warning() {
    let mut e = engine(single_room(None));
    e.ingest(&report("solo", 1, rf(CellBand::B3, -70, 1)));
    assert_eq!(e.warnings(), vec![]);
}

#[test]
fn given_late_report_older_than_last_hit_when_ingested_then_hit_time_not_moved_back() {
    let mut e = engine(single_room(Some(-80)));
    run(&mut e, &[report("solo", 1, hidden_wifi(1, -60, 100))]);
    e.ingest(&report("solo", 2, hidden_wifi(1, -60, 60)));
    // 점수 창은 마지막 적중 t=100s 기준이어야 한다 — 늦게 온 t=60s가 덮어쓰면 창에서 일찍 빠진다.
    e.tick(Timestamp(100_000 + quick_config().score_window_ms - 1_000));
    let alerted: Vec<RuleId> = e.snapshot().iter().flat_map(|a| a.rules.clone()).collect();
    assert_eq!(alerted, vec![RuleId::R3, RuleId::R5]);
}

/// 블루투스 경계 `ble`, 와이파이 경계 `wifi`인 한 교실 배치.
fn room_with_boundaries(ble: Option<i16>, wifi: Option<i16>) -> RoomLayout {
    let mut layout = single_room(ble);
    layout.rooms[0].sensors[0].wifi_inside_boundary_dbm = wifi;
    layout
}

#[test]
fn given_separate_wifi_boundary_when_hotspot_weaker_than_it_then_no_inside_room_hit_for_wifi() {
    let mut e = engine(room_with_boundaries(Some(-80), Some(-60)));
    let alerts = run(
        &mut e,
        &[report(
            "solo",
            1,
            wifi(doc_mac(5)).ssid("일반").rssi(-70).at_secs(20).build(),
        )],
    );
    assert_eq!(
        alerts
            .iter()
            .flat_map(|a| a.rules.clone())
            .collect::<Vec<_>>(),
        Vec::<RuleId>::new()
    );
}

#[test]
fn given_separate_wifi_boundary_when_bluetooth_device_seen_then_bluetooth_boundary_still_used() {
    let mut e = engine(room_with_boundaries(Some(-80), Some(-60)));
    let alerts = run(
        &mut e,
        &[report(
            "solo",
            1,
            ble(7).vendor(0x004C, &[1]).rssi(-70).at_secs(20).build(),
        )],
    );
    assert_eq!(
        alerts
            .iter()
            .flat_map(|a| a.rules.clone())
            .collect::<Vec<_>>(),
        vec![RuleId::R1, RuleId::R2]
    );
}

#[test]
fn given_only_bluetooth_boundary_when_wifi_seen_then_wifi_calibration_missing_and_no_inside_room_hit()
 {
    let mut e = engine(room_with_boundaries(Some(-80), None));
    let alerts = run(
        &mut e,
        &[report(
            "solo",
            1,
            wifi(doc_mac(5)).ssid("일반").rssi(-70).at_secs(20).build(),
        )],
    );
    assert_eq!(
        (
            e.warnings(),
            alerts
                .iter()
                .flat_map(|a| a.rules.clone())
                .collect::<Vec<_>>()
        ),
        (
            vec![EngineWarning::WifiCalibrationMissing(SensorId::new("solo"))],
            Vec::<RuleId>::new()
        )
    );
}

#[test]
fn given_both_boundaries_when_wifi_and_bluetooth_seen_then_no_calibration_warning() {
    let mut e = engine(room_with_boundaries(Some(-80), Some(-60)));
    e.ingest(&report(
        "solo",
        1,
        wifi(doc_mac(5)).ssid("일반").rssi(-70).at_secs(1).build(),
    ));
    e.ingest(&report("solo", 2, ble(7).rssi(-70).at_secs(1).build()));
    assert_eq!(e.warnings(), vec![]);
}
