use std::collections::BTreeMap;

use super::*;
use crate::config::Whitelist;
use crate::obs::{CellBand, OffReason, Permission};
use crate::testkit::{
    doc_random_mac, hidden_wifi, quick_config, report, rf, run, single_room, three_rooms, wifi,
};

fn table(kinds: &[(CollectorKind, CollectorStatus)]) -> CollectorTable {
    let mut table = BTreeMap::new();
    table.insert(
        SensorId::new("solo"),
        (true, kinds.iter().cloned().collect()),
    );
    table
}

fn running() -> CollectorTable {
    table(&[
        (CollectorKind::Ble, CollectorStatus::Running),
        (CollectorKind::Wifi, CollectorStatus::Running),
    ])
}

fn engine() -> Engine {
    Engine::new(quick_config(), single_room(Some(-80)), Whitelist::default())
}

#[test]
fn given_engine_in_baseline_when_viewed_then_preparing_phase_with_remaining_seconds() {
    let view = building_view(&engine(), &running());
    assert_eq!(
        (
            view.phase.watching,
            view.phase.title.starts_with("준비 중"),
            view.phase.remaining_secs,
            view.rooms
        ),
        (false, true, 10, vec![RoomId::new("3-2")])
    );
}

#[test]
fn given_healthy_sensor_when_viewed_then_ok_summary_with_plain_collector_names_and_no_notices() {
    let view = building_view(&engine(), &running());
    let names: Vec<(String, String)> = view.sensors[0]
        .collectors
        .iter()
        .map(|c| (c.name.clone(), c.state.clone()))
        .collect();
    assert_eq!(
        (
            view.sensors[0].place.as_str(),
            view.sensors[0].severity,
            names,
            view.notices.len()
        ),
        (
            "3-2 교실 앞쪽 노트북",
            Severity::Ok,
            vec![
                ("블루투스 신호 감지".to_owned(), "정상 작동 중".to_owned()),
                ("와이파이 신호 감지".to_owned(), "정상 작동 중".to_owned())
            ],
            0
        )
    );
}

#[test]
fn given_bluetooth_permission_denied_when_viewed_then_problem_notice_says_what_where_and_how_to_fix()
 {
    let t = table(&[(
        CollectorKind::Ble,
        CollectorStatus::Off(OffReason::PermissionDenied(Permission::Bluetooth)),
    )]);
    let view = building_view(&engine(), &t);
    let notice = &view.notices[0];
    assert_eq!(
        (
            view.sensors[0].severity,
            notice.severity,
            notice.place.as_str(),
            notice.problem.as_str(),
            notice.action.contains("허용")
        ),
        (
            Severity::Problem,
            Severity::Problem,
            "3-2 교실 앞쪽 노트북",
            "블루투스 신호 감지: 블루투스 사용 권한이 없습니다.",
            true
        )
    );
}

#[test]
fn given_optional_equipment_missing_when_viewed_then_info_only_and_sensor_still_ok() {
    let t = table(&[
        (CollectorKind::Ble, CollectorStatus::Running),
        (
            CollectorKind::Sdr,
            CollectorStatus::Off(OffReason::DeviceMissing),
        ),
    ]);
    let view = building_view(&engine(), &t);
    let severities: Vec<Severity> = view.notices.iter().map(|n| n.severity).collect();
    assert_eq!(
        (view.sensors[0].severity, severities),
        (Severity::Info, vec![Severity::Info])
    );
}

#[test]
fn given_two_alerts_when_viewed_then_stronger_first() {
    let mut e = engine();
    run(
        &mut e,
        &[
            report("solo", 1, hidden_wifi(1, -60, 20)),
            report(
                "solo",
                2,
                wifi(doc_random_mac(2))
                    .ssid_bytes(&[])
                    .rssi(-60)
                    .at_secs(21)
                    .build(),
            ),
        ],
    );
    let levels: Vec<Level> = building_view(&e, &running())
        .alerts
        .iter()
        .map(|a| a.level)
        .collect();
    assert_eq!(levels, vec![Level::High, Level::Medium]);
}

#[test]
fn given_alert_view_when_built_then_plain_explanation_and_technical_text_both_present() {
    let mut e = engine();
    run(&mut e, &[report("solo", 1, hidden_wifi(1, -60, 20))]);
    let view = building_view(&e, &running());
    let alert = &view.alerts[0];
    assert_eq!(
        (
            view.phase.title.as_str(),
            alert.explain.title.as_str(),
            alert.explain.place.as_str(),
            alert.audience.as_str(),
            alert.text.starts_with("[확인 필요]"),
            alert.explain.technical.iter().any(|t| t.contains("R5"))
        ),
        (
            "감시 중",
            "이름을 숨긴 와이파이 (핫스팟일 수 있음)",
            "3-2 교실",
            "3-2 교실",
            true,
            true
        )
    );
}

#[test]
fn given_layout_sensor_never_connected_when_viewed_then_problem_sensor_and_notice() {
    let e = Engine::new(quick_config(), three_rooms(), Whitelist::default());
    let view = building_view(&e, &BTreeMap::new());
    let problems = view
        .notices
        .iter()
        .filter(|n| n.severity == Severity::Problem)
        .count();
    assert_eq!(
        (
            view.sensors.len(),
            view.sensors
                .iter()
                .all(|s| !s.connected && s.summary == "연결된 적 없음"),
            problems,
            view.notices[0].problem.as_str()
        ),
        (6, true, 6, "노트북이 아직 연결되지 않았습니다.")
    );
}

#[test]
fn given_disconnected_sensor_when_viewed_then_connection_lost_notice() {
    let mut t = running();
    t.entry(SensorId::new("solo"))
        .and_modify(|entry| entry.0 = false);
    let view = building_view(&engine(), &t);
    assert_eq!(
        (
            view.sensors[0].summary.as_str(),
            view.notices[0].problem.as_str()
        ),
        ("연결 끊김", "노트북 연결이 끊겼습니다.")
    );
}

#[test]
fn given_alert_without_room_when_viewed_then_marked_for_everyone() {
    let mut e = engine();
    run(
        &mut e,
        &[
            report("solo", 1, rf(CellBand::B3, -70, 1)),
            report("solo", 2, rf(CellBand::B3, -70, 2)),
            report("solo", 3, rf(CellBand::B3, -55, 20)),
        ],
    );
    let view = building_view(&e, &running());
    assert_eq!(
        view.alerts
            .first()
            .map(|a| (a.everyone, a.audience.clone())),
        Some((true, "모든 교실".to_owned()))
    );
}

#[test]
fn given_sensor_mode_with_wifi_permission_missing_when_viewed_then_own_place_and_problem_notice() {
    let info = crate::testkit::sensor_info("3-2-뒤", "3-2", crate::ids::SensorPos::Back);
    let kinds: BTreeMap<CollectorKind, CollectorStatus> = [
        (CollectorKind::Ble, CollectorStatus::Running),
        (
            CollectorKind::Wifi,
            CollectorStatus::Off(OffReason::PermissionDenied(Permission::Location)),
        ),
    ]
    .into_iter()
    .collect();
    let view = sensor_only_view(&info, true, &kinds);
    assert_eq!(
        (
            view.sensors[0].place.as_str(),
            view.sensors[0].severity,
            view.notices.len(),
            view.alerts.len(),
            view.phase.title.starts_with("센서")
        ),
        ("3-2 교실 뒤쪽 노트북", Severity::Problem, 1, 0, true)
    );
}

#[test]
fn given_alert_with_identifier_when_technical_removed_then_no_identifier_or_rule_text_remains() {
    let mut e = engine();
    run(
        &mut e,
        &[report(
            "solo",
            1,
            wifi(doc_random_mac(2))
                .ssid_bytes(&[])
                .rssi(-60)
                .at_secs(20)
                .build(),
        )],
    );
    let t = table(&[(
        CollectorKind::Wifi,
        CollectorStatus::Off(OffReason::PermissionDenied(Permission::Location)),
    )]);
    let full = building_view(&e, &t);
    let public = serde_json::to_string(&full.without_technical()).unwrap_or_default();
    assert_eq!(
        (
            serde_json::to_string(&full)
                .unwrap_or_default()
                .contains("02:00:5E:00:53:02"),
            public.contains("02:00:5E:00:53:02"),
            public.contains("R5"),
            public.contains("숨긴 와이파이")
        ),
        (true, false, false, true)
    );
}
