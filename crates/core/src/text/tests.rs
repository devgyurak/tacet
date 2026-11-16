use super::*;
use crate::device::DeviceKey;
use crate::ids::{RoomId, SensorId};
use crate::obs::CollectorKind;
use crate::testkit::{doc_mac, doc_random_mac, three_rooms};
use crate::time::Timestamp;

fn candidate() -> AlertCandidate {
    AlertCandidate {
        device: DeviceKey::Bssid(doc_mac(0x2A)),
        label: DeviceLabel::Wifi {
            bssid: doc_mac(0x2A),
            hidden: true,
            direct: false,
        },
        level: Level::Medium,
        score: 6,
        rules: vec![RuleId::R3, RuleId::R5],
        localization: Some(Localization {
            room: Some(RoomId::new("3-2")),
            position: Some(SensorPos::Back),
            confidence: Confidence::Medium,
            likely_outside: false,
            needs_calibration: false,
            evidence: vec![
                (SensorId::new("3-2-뒤"), -58),
                (SensorId::new("3-2-앞"), -66),
                (SensorId::new("3-3-앞"), -71),
            ],
        }),
        notify_rooms: vec![RoomId::new("3-2")],
        penalties: vec![],
        at: Timestamp(0),
    }
}

#[test]
fn given_hidden_wifi_alert_when_rendered_then_design_alert_form() {
    let text = alert(&candidate(), &three_rooms());
    let lines: Vec<&str> = text.lines().take(4).collect();
    assert_eq!(
        lines,
        vec![
            "[확인 필요] 숨긴 Wi-Fi 감지 (BSSID 00:00:5E:00:53:2A)",
            "의심 강도: 중간 (점수 6)",
            "추정 위치: 3-2 교실 뒤쪽 · 신뢰도 중",
            "근거: 3-2 뒤 센서 -58dBm / 3-2 앞 -66 / 3-3 앞 -71 / 3-1 미탐지",
        ]
    );
}

#[test]
fn given_any_alert_when_rendered_then_starts_with_check_needed_and_ends_with_human_check() {
    let text = alert(&candidate(), &three_rooms());
    assert_eq!(
        (
            text.starts_with("[확인 필요]"),
            text.ends_with("최종 확인은 감독관이 한다.")
        ),
        (true, true)
    );
}

#[test]
fn given_outside_and_undecided_locations_when_rendered_then_no_room_claimed() {
    let mut loc = candidate().localization.expect("위치가 있다");
    loc.likely_outside = true;
    loc.room = None;
    let outside = location(&loc);
    loc.likely_outside = false;
    loc.needs_calibration = true;
    let undecided = location(&loc);
    assert_eq!(
        (outside, undecided),
        (
            "추정 위치: 건물 밖 가능성(교실 경계 밖) · 신뢰도 중".to_owned(),
            "추정 위치: 건물 안, 교실 미정 · 신뢰도 중 · 보정 필요".to_owned()
        )
    );
}

#[test]
fn given_device_labels_when_rendered_then_korean_descriptions() {
    let labels = [
        device(&DeviceLabel::Ble {
            company: Some(0x01AB),
        }),
        device(&DeviceLabel::Ble {
            company: Some(0x1234),
        }),
        device(&DeviceLabel::Wifi {
            bssid: doc_random_mac(1),
            hidden: true,
            direct: false,
        }),
        device(&DeviceLabel::Wifi {
            bssid: doc_random_mac(1),
            hidden: false,
            direct: true,
        }),
        device(&DeviceLabel::Wifi {
            bssid: doc_random_mac(1),
            hidden: false,
            direct: false,
        }),
        device(&DeviceLabel::Cell { band: CellBand::B3 }),
    ];
    assert_eq!(
        labels,
        [
            "BLE 광고 감지 (Meta)",
            "BLE 광고 감지",
            "숨긴 Wi-Fi 감지 (BSSID 02:00:5E:00:53:01)",
            "Wi-Fi Direct 감지 (BSSID 02:00:5E:00:53:01)",
            "새 Wi-Fi 감지 (BSSID 02:00:5E:00:53:01)",
            "LTE 업링크 증가 감지 (B3)",
        ]
    );
}

#[test]
fn given_alert_without_localization_when_rendered_then_building_level_line() {
    let mut c = candidate();
    c.localization = None;
    assert!(alert(&c, &three_rooms()).contains("추정 위치: 세기 정보 없음 · 건물 안 탐지"));
}

#[test]
fn given_collector_statuses_when_rendered_then_reason_explained() {
    let lines = [
        collector_status(
            CollectorKind::Wifi,
            &CollectorStatus::Off(OffReason::PermissionDenied(Permission::PreciseLocation)),
        ),
        collector_status(CollectorKind::Ble, &CollectorStatus::Running),
        collector_status(
            CollectorKind::Sdr,
            &CollectorStatus::Off(OffReason::DeviceMissing),
        ),
    ];
    assert_eq!(
        lines,
        [
            "Wi-Fi 수집: 꺼짐 — 정밀 위치 권한 없음 — 설정 > 개인 정보 > 위치에서 켠다",
            "BLE 수집: 수집 중",
            "LTE 업링크 수집(SDR): 꺼짐 — 장비 없음",
        ]
    );
}

#[test]
fn given_remaining_off_reasons_when_rendered_then_each_has_text() {
    let reasons = [
        OffReason::PermissionDenied(Permission::Location),
        OffReason::PermissionDenied(Permission::Bluetooth),
        OffReason::DriverMissing,
        OffReason::Unsupported("리눅스".to_owned()),
        OffReason::Error("시간 초과".to_owned()),
    ];
    let rendered: Vec<String> = reasons
        .into_iter()
        .map(|r| collector_status(CollectorKind::Sdr, &CollectorStatus::Off(r)))
        .collect();
    assert_eq!(
        rendered
            .iter()
            .filter(|line| line.contains("꺼짐 — "))
            .count(),
        5
    );
}

#[test]
fn given_starting_collector_when_rendered_then_starting_text() {
    assert_eq!(
        collector_status(CollectorKind::Ble, &CollectorStatus::Starting),
        "BLE 수집: 시작 중"
    );
}

#[test]
fn given_engine_warnings_and_phases_when_rendered_then_korean_lines() {
    let lines = [
        warning(&EngineWarning::CalibrationMissing(SensorId::new("solo"))),
        warning(&EngineWarning::UnknownSensor(SensorId::new("x"))),
        phase(Phase::Baseline, 1_500),
        phase(Phase::Watching, 0),
    ];
    assert_eq!(
        lines,
        [
            "solo 센서: 교실 보정값 없음 — 교실 경계 판정(R2·R3, 교실 안팎)을 하지 않는다",
            "x 센서: 배치에 없는 센서라 보고서를 버렸다",
            "기준선 수집 중 — 2초 남음",
            "감시 중",
        ]
    );
}

#[test]
fn given_all_rules_when_described_then_each_starts_with_its_id() {
    let ids = [
        RuleId::R1,
        RuleId::R1a,
        RuleId::R1b,
        RuleId::R2,
        RuleId::R3,
        RuleId::R4,
        RuleId::R5,
        RuleId::R6,
        RuleId::R7,
    ];
    let names: Vec<&str> = ids.iter().map(|r| rule(*r)).collect();
    assert_eq!(names.iter().filter(|n| n.starts_with('R')).count(), 9);
}

#[test]
fn given_levels_and_confidences_when_named_then_korean() {
    assert_eq!(
        (
            [level(Level::Low), level(Level::High)],
            [confidence(Confidence::Low), confidence(Confidence::High)]
        ),
        (["낮음", "높음"], ["낮음", "높음"])
    );
}

#[test]
fn given_known_and_unknown_company_ids_when_named_then_only_known_resolve() {
    assert_eq!(
        [
            company_name(0x004C),
            company_name(0x0075),
            company_name(0x00E0),
            company_name(0x058E),
            company_name(0x0D53),
            company_name(0x03C2),
            company_name(1)
        ],
        [
            Some("Apple"),
            Some("Samsung"),
            Some("Google"),
            Some("Meta Platforms Technologies"),
            Some("Luxottica"),
            Some("Snap"),
            None
        ]
    );
}

#[test]
fn given_front_position_and_unlayouted_sensor_when_rendered_then_front_and_raw_sensor_name() {
    let mut loc = candidate().localization.expect("위치가 있다");
    loc.position = Some(SensorPos::Front);
    loc.evidence = vec![
        (SensorId::new("3-1-앞"), -55),
        (SensorId::new("배치밖"), -70),
    ];
    assert_eq!(
        (location(&loc), evidence(&loc, &three_rooms())),
        (
            "추정 위치: 3-2 교실 앞쪽 · 신뢰도 중".to_owned(),
            "근거: 3-1 앞 센서 -55dBm / 배치밖 -70 / 3-2 미탐지 / 3-3 미탐지".to_owned()
        )
    );
}

#[test]
fn given_room_without_position_when_rendered_then_room_only() {
    let mut loc = candidate().localization.expect("위치가 있다");
    loc.position = None;
    assert_eq!(location(&loc), "추정 위치: 3-2 교실 · 신뢰도 중");
}

#[test]
fn given_each_cell_band_when_described_then_band_name_shown() {
    let names: Vec<String> = [CellBand::B1, CellBand::B5, CellBand::B7]
        .iter()
        .map(|b| device(&DeviceLabel::Cell { band: *b }))
        .collect();
    assert_eq!(
        names,
        [
            "LTE 업링크 증가 감지 (B1)",
            "LTE 업링크 증가 감지 (B5)",
            "LTE 업링크 증가 감지 (B7)"
        ]
    );
}

#[test]
fn given_alert_with_penalties_when_rendered_then_penalty_line_listed() {
    let mut c = candidate();
    c.penalties = vec![
        crate::engine::Penalty::StorageZone,
        crate::engine::Penalty::FarOutside,
    ];
    assert!(alert(&c, &three_rooms()).contains(
        "감점: 수거폰 보관 위치의 신호, 모든 센서에서 교실 경계보다 훨씬 약함(건물 밖 가능성)"
    ));
}

#[test]
fn given_mac_whitelist_warning_when_rendered_then_tells_to_use_manufacturer_entry() {
    assert!(
        warning(&EngineWarning::MacWhitelistNotApplicable(SensorId::new(
            "맥"
        )))
        .ends_with("의료기기는 제조사 데이터로 등록한다")
    );
}
