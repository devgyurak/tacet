//! 위협 모델(설계 §3 T1–T7)과 오탐 요인(설계 §4.3, §4.7)의 합성 시나리오.
//!
//! 기준선은 10초(`quick_config`), 기준선 이후 20초부터 위협 기기가 나타난다. 식별자는 모두 합성 값이다.

use tacet_core::config::{DetectorConfig, Whitelist, WhitelistEntry};
use tacet_core::engine::{AlertCandidate, DeviceLabel, Level};
use tacet_core::ids::{RoomId, SensorPos};
use tacet_core::locate::Confidence;
use tacet_core::obs::{Band, CellBand};
use tacet_core::rules::RuleId;
use tacet_core::testkit::{
    ble, doc_mac, doc_random_mac, hidden_wifi, multi_sensor, quick_config, repeat, report, rf,
    run_scenario, single_room, snapshot_after, three_rooms, wifi, with_storage,
};

fn rules_seen(alerts: &[AlertCandidate]) -> Vec<RuleId> {
    let mut rules: Vec<RuleId> = alerts.iter().flat_map(|a| a.rules.clone()).collect();
    rules.sort();
    rules.dedup();
    rules
}

fn top_level(alerts: &[AlertCandidate]) -> Option<Level> {
    alerts.iter().map(|a| a.level).max()
}

fn calibrated() -> tacet_core::config::RoomLayout {
    single_room(Some(-80))
}

#[test]
fn given_t1_glasses_with_hidden_phone_hotspot_and_phone_ble_when_watched_then_high_alert() {
    let mut reports = repeat("solo", 5, 20, |s| {
        ble(1)
            .vendor(0x004C, &[0x10, 5])
            .rssi(-65)
            .at_secs(s)
            .build()
    });
    reports.extend(repeat("solo", 5, 20, |s| {
        wifi(doc_random_mac(2))
            .ssid_bytes(&[])
            .rssi(-60)
            .at_secs(s)
            .build()
    }));
    let alerts = run_scenario(quick_config(), calibrated(), Whitelist::default(), &reports);
    assert_eq!(top_level(&alerts), Some(Level::High));
}

#[test]
fn given_t2_phone_ble_only_when_watched_then_medium_alert_with_r1_and_r2() {
    let reports = repeat("solo", 3, 20, |s| {
        ble(1).vendor(0x0075, &[1]).rssi(-65).at_secs(s).build()
    });
    let alerts = run_scenario(quick_config(), calibrated(), Whitelist::default(), &reports);
    assert_eq!(
        (top_level(&alerts), rules_seen(&alerts)),
        (Some(Level::Medium), vec![RuleId::R1, RuleId::R2])
    );
}

#[test]
fn given_t3_hidden_ssid_hotspot_when_watched_then_r3_r4_r5_alert() {
    let reports = repeat("solo", 3, 20, |s| {
        wifi(doc_random_mac(2))
            .ssid_bytes(&[0; 6])
            .rssi(-60)
            .at_secs(s)
            .build()
    });
    let alerts = run_scenario(quick_config(), calibrated(), Whitelist::default(), &reports);
    assert_eq!(
        rules_seen(&alerts),
        vec![RuleId::R3, RuleId::R4, RuleId::R5]
    );
}

#[test]
fn given_t4_6ghz_hotspot_when_watched_then_same_rules_as_2_4ghz() {
    let reports = repeat("solo", 3, 20, |s| {
        wifi(doc_random_mac(2))
            .ssid_bytes(&[])
            .channel(Band::B6, 37)
            .rssi(-60)
            .at_secs(s)
            .build()
    });
    let alerts = run_scenario(quick_config(), calibrated(), Whitelist::default(), &reports);
    assert_eq!(
        rules_seen(&alerts),
        vec![RuleId::R3, RuleId::R4, RuleId::R5]
    );
}

#[test]
fn given_t5_uplink_burst_when_watched_then_r7_alert() {
    let mut reports = repeat("solo", 5, 0, |s| rf(CellBand::B3, -72, s));
    reports.push(report("solo", 51, rf(CellBand::B3, -58, 21)));
    let alerts = run_scenario(quick_config(), calibrated(), Whitelist::default(), &reports);
    assert_eq!(rules_seen(&alerts), vec![RuleId::R7]);
}

#[test]
fn given_glasses_power_on_advertising_when_watched_then_r1a_and_r2_alert() {
    let reports = vec![report(
        "solo",
        1,
        ble(5).vendor(0x01AB, &[2]).rssi(-70).at_secs(20).build(),
    )];
    let alerts = run_scenario(quick_config(), calibrated(), Whitelist::default(), &reports);
    assert_eq!(rules_seen(&alerts), vec![RuleId::R1a, RuleId::R2]);
}

#[test]
fn given_photo_transfer_over_wifi_direct_when_watched_then_new_random_direct_wifi_alerts() {
    let reports = repeat("solo", 2, 20, |s| {
        wifi(doc_random_mac(7))
            .ssid("DIRECT-gl-Phone")
            .rssi(-70)
            .at_secs(s)
            .build()
    });
    let alerts = run_scenario(quick_config(), calibrated(), Whitelist::default(), &reports);
    assert_eq!(
        rules_seen(&alerts),
        vec![RuleId::R3, RuleId::R4, RuleId::R6]
    );
}

#[test]
fn given_device_without_bluetooth_or_radio_when_watched_then_nothing_is_detected() {
    let alerts = run_scenario(quick_config(), calibrated(), Whitelist::default(), &[]);
    assert_eq!(alerts, Vec::<AlertCandidate>::new());
}

#[test]
fn given_collected_phone_left_on_at_storage_spot_when_watched_then_no_alert_is_raised() {
    let layout = with_storage(calibrated(), "solo", -50, -35);
    let reports = repeat("solo", 5, 20, |s| {
        ble(1).vendor(0x004C, &[1]).rssi(-42).at_secs(s).build()
    });
    assert_eq!(
        run_scenario(quick_config(), layout, Whitelist::default(), &reports),
        Vec::<AlertCandidate>::new()
    );
}

#[test]
fn given_school_ap_seen_during_baseline_when_watched_then_no_alert_is_raised() {
    let reports = repeat("solo", 30, 0, |s| {
        wifi(doc_random_mac(30))
            .ssid_bytes(&[])
            .rssi(-55)
            .at_secs(s)
            .build()
    });
    assert_eq!(
        run_scenario(quick_config(), calibrated(), Whitelist::default(), &reports),
        Vec::<AlertCandidate>::new()
    );
}

#[test]
fn given_registered_hearing_aid_by_address_when_watched_then_no_alert_is_raised() {
    let whitelist = Whitelist {
        entries: vec![WhitelistEntry::Mac(doc_random_mac(8))],
    };
    let reports = repeat("solo", 3, 20, |s| {
        ble(8)
            .vendor(0x004C, &[0x10, 1])
            .rssi(-50)
            .at_secs(s)
            .build()
    });
    assert_eq!(
        run_scenario(quick_config(), calibrated(), whitelist, &reports),
        Vec::<AlertCandidate>::new()
    );
}

#[test]
fn given_registered_cgm_by_vendor_data_when_watched_then_no_alert_is_raised() {
    let whitelist = Whitelist {
        entries: vec![WhitelistEntry::Manufacturer {
            company: 0x00E0,
            prefix: vec![0xC6],
        }],
    };
    let reports = repeat("solo", 3, 20, |s| {
        ble(9)
            .vendor(0x00E0, &[0xC6, 1])
            .rssi(-50)
            .at_secs(s)
            .build()
    });
    assert_eq!(
        run_scenario(quick_config(), calibrated(), whitelist, &reports),
        Vec::<AlertCandidate>::new()
    );
}

#[test]
fn given_rotating_phone_address_when_watched_then_counted_as_one_device() {
    let reports = vec![
        report(
            "solo",
            1,
            ble(1).vendor(0x004C, &[7, 7]).rssi(-65).at_secs(20).build(),
        ),
        report(
            "solo",
            2,
            ble(2).vendor(0x004C, &[7, 7]).rssi(-66).at_secs(25).build(),
        ),
        report(
            "solo",
            3,
            ble(3).vendor(0x004C, &[7, 7]).rssi(-64).at_secs(30).build(),
        ),
    ];
    let alerts = run_scenario(quick_config(), calibrated(), Whitelist::default(), &reports);
    assert_eq!(alerts.len(), 1);
}

#[test]
fn given_uncalibrated_room_when_phone_ble_seen_then_alert_without_boundary_rule_and_low_confidence()
{
    let reports = repeat("solo", 2, 20, |s| {
        ble(1).vendor(0x004C, &[1]).rssi(-40).at_secs(s).build()
    });
    let alerts = run_scenario(
        quick_config(),
        single_room(None),
        Whitelist::default(),
        &reports,
    );
    let summary: Vec<(Vec<RuleId>, Option<Confidence>)> = alerts
        .iter()
        .map(|a| {
            (
                a.rules.clone(),
                a.localization.as_ref().map(|l| l.confidence),
            )
        })
        .collect();
    assert_eq!(summary, vec![(vec![RuleId::R1], Some(Confidence::Low))]);
}

#[test]
fn given_neighbor_room_device_when_seen_by_three_rooms_then_assigned_to_neighbor_room() {
    let reports = multi_sensor(
        &[
            ("3-3-뒤", -52),
            ("3-3-앞", -60),
            ("3-2-뒤", -66),
            ("3-1-앞", -80),
        ],
        20,
        5,
        |rssi, s| hidden_wifi(0x2A, rssi, s),
    );
    let alerts = snapshot_after(
        quick_config(),
        three_rooms(),
        Whitelist::default(),
        &reports,
    );
    let last = alerts
        .last()
        .and_then(|a| a.localization.clone())
        .map(|l| (l.room, l.position, l.confidence));
    assert_eq!(
        last,
        Some((
            Some(RoomId::new("3-3")),
            Some(SensorPos::Back),
            Confidence::High
        ))
    );
}

#[test]
fn given_body_shadowed_phone_in_our_room_weaker_than_wall_attenuated_neighbor_when_located_then_our_room_still_wins()
 {
    let reports = multi_sensor(
        &[("3-2-뒤", -71), ("3-2-앞", -74), ("3-1-앞", -79)],
        20,
        5,
        |rssi, s| hidden_wifi(0x2B, rssi, s),
    );
    let alerts = run_scenario(
        quick_config(),
        three_rooms(),
        Whitelist::default(),
        &reports,
    );
    let room = alerts
        .last()
        .and_then(|a| a.localization.as_ref())
        .and_then(|l| l.room.clone());
    assert_eq!(room, Some(RoomId::new("3-2")));
}

#[test]
fn given_device_outside_building_when_watched_then_outside_penalty_suppresses_alert() {
    let reports = multi_sensor(&[("3-1-앞", -95), ("3-2-앞", -100)], 20, 5, |rssi, s| {
        ble(4).vendor(0x004C, &[1]).rssi(rssi).at_secs(s).build()
    });
    assert_eq!(
        run_scenario(
            quick_config(),
            three_rooms(),
            Whitelist::default(),
            &reports
        ),
        Vec::<AlertCandidate>::new()
    );
}

#[test]
fn given_body_shadowed_phone_weaker_than_boundary_but_within_shield_margin_when_watched_then_still_alerted()
 {
    let reports = multi_sensor(&[("3-1-앞", -88), ("3-2-앞", -90)], 20, 5, |rssi, s| {
        ble(4).vendor(0x004C, &[1]).rssi(rssi).at_secs(s).build()
    });
    let alerts = run_scenario(
        quick_config(),
        three_rooms(),
        Whitelist::default(),
        &reports,
    );
    assert_eq!(
        alerts
            .first()
            .map(|a| (a.rules.clone(), a.penalties.is_empty())),
        Some((vec![RuleId::R1], true))
    );
}

#[test]
fn given_neighbor_device_first_seen_only_by_our_sensor_when_alerted_then_low_confidence_and_adjacent_rooms_told()
 {
    let alerts = run_scenario(
        quick_config(),
        three_rooms(),
        Whitelist::default(),
        &[report("3-2-앞", 1, hidden_wifi(0x30, -70, 20))],
    );
    let first = alerts.first().map(|a| {
        (
            a.localization.as_ref().map(|l| l.confidence),
            a.notify_rooms.clone(),
        )
    });
    assert_eq!(
        first,
        Some((
            Some(Confidence::Low),
            vec![RoomId::new("3-1"), RoomId::new("3-2"), RoomId::new("3-3")]
        ))
    );
}

#[test]
fn given_macos_sensors_seeing_same_glasses_when_located_then_confidence_lowered() {
    let reports = multi_sensor(&[("3-1-앞", -50), ("3-2-앞", -62)], 20, 3, |rssi, s| {
        ble(0)
            .host_local(u128::from(rssi.unsigned_abs()))
            .vendor(0x01AB, &[4])
            .rssi(rssi)
            .at_secs(s)
            .build()
    });
    let alerts = snapshot_after(
        quick_config(),
        three_rooms(),
        Whitelist::default(),
        &reports,
    );
    let conf = alerts
        .last()
        .and_then(|a| a.localization.as_ref())
        .map(|l| l.confidence);
    assert_eq!(conf, Some(Confidence::Medium));
}

#[test]
fn given_same_scenario_twice_when_run_then_identical_results() {
    let make = || {
        let mut r = multi_sensor(&[("3-1-앞", -55), ("3-2-뒤", -61)], 20, 4, |rssi, s| {
            hidden_wifi(1, rssi, s)
        });
        r.extend(repeat("3-2-앞", 4, 20, |s| {
            wifi(doc_random_mac(3))
                .ssid_bytes(&[])
                .rssi(-60)
                .at_secs(s)
                .build()
        }));
        run_scenario(
            DetectorConfig {
                baseline_ms: 10_000,
                ..DetectorConfig::default()
            },
            three_rooms(),
            Whitelist::default(),
            &r,
        )
    };
    assert_eq!(make(), make());
}

#[test]
fn given_alert_label_for_hidden_wifi_when_alerted_then_bssid_and_hidden_kept() {
    let alerts = run_scenario(
        quick_config(),
        calibrated(),
        Whitelist::default(),
        &[report("solo", 1, hidden_wifi(0x2A, -60, 20))],
    );
    assert_eq!(
        alerts.first().map(|a| a.label.clone()),
        Some(DeviceLabel::Wifi {
            bssid: doc_mac(0x2A),
            hidden: true,
            direct: false
        })
    );
}
