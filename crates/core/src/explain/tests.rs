use super::*;
use crate::device::DeviceKey;
use crate::engine::Penalty;
use crate::ids::{RoomId, SensorId, SensorPos};
use crate::testkit::{doc_mac, three_rooms};
use crate::time::Timestamp;

fn localization(
    room: Option<&str>,
    position: Option<SensorPos>,
    confidence: Confidence,
) -> Localization {
    Localization {
        room: room.map(RoomId::new),
        position,
        confidence,
        likely_outside: false,
        needs_calibration: false,
        evidence: vec![(SensorId::new("3-2-뒤"), -58)],
    }
}

fn candidate(label: DeviceLabel, rules: Vec<RuleId>, loc: Option<Localization>) -> AlertCandidate {
    AlertCandidate {
        device: DeviceKey::Bssid(doc_mac(0x2A)),
        label,
        level: Level::Medium,
        score: 6,
        rules,
        localization: loc,
        notify_rooms: vec![RoomId::new("3-2")],
        penalties: vec![],
        at: Timestamp(0),
    }
}

/// 제목에 남은 기술 식별자(BSSID·주소 표기).
fn titles_with_identifiers(labels: &[DeviceLabel]) -> Vec<String> {
    labels
        .iter()
        .map(title)
        .filter(|t| t.contains("BSSID") || t.contains("00:") || t.contains("0x"))
        .collect()
}

#[test]
fn given_device_labels_when_titled_then_plain_words_without_identifiers() {
    let labels = [
        DeviceLabel::Wifi {
            bssid: doc_mac(1),
            hidden: true,
            direct: false,
        },
        DeviceLabel::Wifi {
            bssid: doc_mac(2),
            hidden: false,
            direct: true,
        },
        DeviceLabel::Wifi {
            bssid: doc_mac(3),
            hidden: false,
            direct: false,
        },
    ];
    assert_eq!(titles_with_identifiers(&labels), Vec::<String>::new());
}

#[test]
fn given_ble_vendors_when_titled_then_glasses_phone_and_unknown_are_told_apart() {
    assert_eq!(
        [
            title(&DeviceLabel::Ble {
                company: Some(0x01AB)
            }),
            title(&DeviceLabel::Ble {
                company: Some(0x004C)
            }),
            title(&DeviceLabel::Ble { company: None }),
        ],
        [
            "스마트 안경으로 보이는 블루투스 신호 (Meta)".to_owned(),
            "휴대폰일 수 있는 블루투스 신호 (Apple)".to_owned(),
            "새 블루투스 기기 신호".to_owned()
        ]
    );
}

#[test]
fn given_localizations_when_placed_then_room_side_outside_and_unknown_in_plain_words() {
    let outside = Localization {
        likely_outside: true,
        ..localization(None, None, Confidence::Low)
    };
    assert_eq!(
        [
            place(Some(&localization(
                Some("3-2"),
                Some(SensorPos::Back),
                Confidence::High
            ))),
            place(Some(&localization(Some("3-2"), None, Confidence::High))),
            place(Some(&outside)),
            place(Some(&localization(None, None, Confidence::Low))),
            place(None),
        ],
        [
            "3-2 교실 뒤쪽".to_owned(),
            "3-2 교실".to_owned(),
            "건물 밖일 수 있음 (모든 교실에서 신호가 아주 약함)".to_owned(),
            "건물 안 (어느 교실인지 정하지 못함)".to_owned(),
            "건물 안 (신호 세기 정보가 없어 교실을 알 수 없음)".to_owned()
        ]
    );
}

#[test]
fn given_low_confidence_when_explained_then_says_neighbor_rooms_were_told_too() {
    assert!(
        certainty(Some(&localization(Some("3-2"), None, Confidence::Low)))
            .contains("옆 교실에도 함께 알렸습니다")
    );
}

#[test]
fn given_alert_with_penalty_and_no_calibration_when_explained_then_caveats_reasons_and_guide_in_plain_words()
 {
    let loc = Localization {
        needs_calibration: true,
        ..localization(Some("3-2"), Some(SensorPos::Back), Confidence::Low)
    };
    let mut c = candidate(
        DeviceLabel::Ble {
            company: Some(0x004C),
        },
        vec![RuleId::R1, RuleId::R2],
        Some(loc),
    );
    c.penalties = vec![Penalty::StorageZone];
    let e = alert(&c, &three_rooms());
    assert_eq!(
        (
            e.level.as_str(),
            e.reasons.len(),
            e.caveats.len(),
            e.guide.starts_with("3-2 교실 뒤쪽을 중심으로"),
            e.technical.len()
        ),
        ("의심 보통", 2, 2, true, 4)
    );
}

#[test]
fn given_alert_without_location_when_explained_then_guide_asks_every_room_to_look() {
    let e = alert(
        &candidate(
            DeviceLabel::Cell {
                band: crate::obs::CellBand::B3,
            },
            vec![RuleId::R7],
            None,
        ),
        &three_rooms(),
    );
    assert_eq!(
        (
            e.title.as_str(),
            e.guide.starts_with("교실을 정하지 못했습니다"),
            e.technical.len()
        ),
        ("휴대폰 데이터 전송으로 보이는 전파 증가", true, 2)
    );
}

/// 쉬운 말 문구에 남은 단정 표현.
fn verdict_words(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|t| ["부정행위", "적발", "확정"].iter().any(|w| t.contains(w)))
        .cloned()
        .collect()
}

#[test]
fn given_every_rule_reason_when_checked_then_no_verdict_words() {
    let all = [
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
    let texts: Vec<String> = all.iter().map(|r| reason(*r).to_owned()).collect();
    assert_eq!(verdict_words(&texts), Vec::<String>::new());
}

#[test]
fn given_collector_states_when_weighed_then_basic_collectors_are_problems_and_missing_extras_are_info()
 {
    let off = |r: OffReason| CollectorStatus::Off(r);
    assert_eq!(
        [
            collector_state(
                CollectorKind::Wifi,
                &off(OffReason::PermissionDenied(Permission::Location))
            )
            .0,
            collector_state(CollectorKind::Ble, &off(OffReason::DeviceMissing)).0,
            collector_state(CollectorKind::Sdr, &off(OffReason::DeviceMissing)).0,
            collector_state(CollectorKind::Sdr, &off(OffReason::DriverMissing)).0,
            collector_state(CollectorKind::Ble, &CollectorStatus::Running).0,
        ],
        [
            Severity::Problem,
            Severity::Problem,
            Severity::Info,
            Severity::Warning,
            Severity::Ok
        ]
    );
}

#[test]
fn given_off_reasons_when_explained_then_each_has_problem_and_action() {
    let reasons = [
        OffReason::PermissionDenied(Permission::PreciseLocation),
        OffReason::PermissionDenied(Permission::Bluetooth),
        OffReason::DeviceMissing,
        OffReason::DriverMissing,
        OffReason::Unsupported("시험".to_owned()),
        OffReason::Error("시험".to_owned()),
    ];
    let empty: Vec<&OffReason> = reasons
        .iter()
        .filter(|r| {
            collector_notice(
                &three_rooms(),
                &SensorId::new("3-1-앞"),
                CollectorKind::Wifi,
                &CollectorStatus::Off((*r).clone()),
            )
            .is_none_or(|n| {
                n.problem.is_empty() || n.action.is_empty() || n.place != "3-1 교실 앞쪽 노트북"
            })
        })
        .collect();
    assert_eq!(empty, Vec::<&OffReason>::new());
}

#[test]
fn given_engine_warnings_when_explained_then_calibration_is_warning_and_mac_whitelist_is_info() {
    let layout = three_rooms();
    let s = SensorId::new("3-2-앞");
    assert_eq!(
        [
            warning(&layout, &EngineWarning::CalibrationMissing(s.clone())).severity,
            warning(
                &layout,
                &EngineWarning::UnknownSensor(SensorId::new("모름"))
            )
            .severity,
            warning(&layout, &EngineWarning::MacWhitelistNotApplicable(s)).severity,
        ],
        [Severity::Warning, Severity::Warning, Severity::Info]
    );
}

#[test]
fn given_unknown_sensor_when_placed_then_says_not_in_layout() {
    assert_eq!(
        sensor_place(&three_rooms(), &SensorId::new("모름")),
        "모름 노트북(교실 배치에 없음)"
    );
}

#[test]
fn given_sensor_with_place_description_when_placed_then_description_replaces_front_back() {
    let mut layout = crate::testkit::single_room(None);
    layout.rooms[0].sensors[0].place = Some("칠판에서 셋째 줄 가운데".to_owned());
    assert_eq!(
        sensor_place(&layout, &SensorId::new("solo")),
        format!("{} 교실 칠판에서 셋째 줄 가운데 노트북", layout.rooms[0].id)
    );
}

#[test]
fn given_rooms_when_audience_named_then_listed_or_everyone() {
    assert_eq!(
        [
            audience(&[RoomId::new("3-1"), RoomId::new("3-2")]),
            audience(&[])
        ],
        ["3-1 교실, 3-2 교실".to_owned(), "모든 교실".to_owned()]
    );
}

#[test]
fn given_location_permission_missing_when_noticed_then_settings_shortcut_attached() {
    let layout = three_rooms();
    let s = SensorId::new("3-1-앞");
    let notice = |r: OffReason| {
        collector_notice(&layout, &s, CollectorKind::Wifi, &CollectorStatus::Off(r))
            .and_then(|n| n.fix)
    };
    assert_eq!(
        [
            notice(OffReason::PermissionDenied(Permission::Location)),
            notice(OffReason::PermissionDenied(Permission::PreciseLocation)),
            notice(OffReason::DeviceMissing),
        ],
        [
            Some(Fix::OpenLocationSettings),
            Some(Fix::OpenLocationSettings),
            None
        ]
    );
}

#[test]
fn given_calibration_missing_when_explained_then_calibrate_shortcut_attached() {
    let notice = warning(
        &three_rooms(),
        &EngineWarning::CalibrationMissing(SensorId::new("3-2-앞")),
    );
    assert_eq!(
        (notice.fix, notice.action.contains("교실 보정하기")),
        (Some(Fix::Calibrate), true)
    );
}
