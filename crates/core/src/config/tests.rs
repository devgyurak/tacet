use super::*;
use crate::testkit::three_rooms;

#[test]
fn given_default_config_when_read_then_baseline_is_three_minutes() {
    assert_eq!(DetectorConfig::default().baseline_ms, 180_000);
}

#[test]
fn given_three_room_layout_when_validated_then_no_layout_error() {
    assert_eq!(three_rooms().validate(), Ok(()));
}

#[test]
fn given_duplicate_room_when_validated_then_duplicate_room_error() {
    let mut layout = three_rooms();
    layout.rooms[1].id = RoomId::new("3-1");
    assert_eq!(
        layout.validate(),
        Err(LayoutError::DuplicateRoom(RoomId::new("3-1")))
    );
}

#[test]
fn given_duplicate_sensor_when_validated_then_duplicate_sensor_error() {
    let mut layout = three_rooms();
    layout.rooms[1].sensors[0].id = SensorId::new("3-1-앞");
    assert_eq!(
        layout.validate(),
        Err(LayoutError::DuplicateSensor(SensorId::new("3-1-앞")))
    );
}

#[test]
fn given_unknown_adjacent_room_when_validated_then_unknown_adjacent_error() {
    let mut layout = three_rooms();
    layout.rooms[0].adjacent.push(RoomId::new("9-9"));
    assert_eq!(
        layout.validate(),
        Err(LayoutError::UnknownAdjacentRoom(RoomId::new("9-9")))
    );
}

#[test]
fn given_sensor_id_when_looked_up_then_room_and_position_found() {
    let layout = three_rooms();
    let found = layout
        .sensor(&SensorId::new("3-2-뒤"))
        .map(|(room, spec)| (room.id.clone(), spec.position));
    assert_eq!(found, Some((RoomId::new("3-2"), SensorPos::Back)));
}

#[test]
fn given_partial_toml_like_json_when_deserialized_then_defaults_fill_rest() {
    let cfg: DetectorConfig =
        serde_json::from_str(r#"{"alert_score": 7}"#).expect("설정 JSON은 유효하다");
    assert_eq!((cfg.alert_score, cfg.baseline_ms), (7, 180_000));
}

#[test]
fn given_default_config_when_validated_then_no_setting_error() {
    assert_eq!(DetectorConfig::default().validate(), Ok(()));
}

#[test]
fn given_bad_orders_when_validated_then_errors() {
    let scores = DetectorConfig {
        medium_score: 20,
        ..DetectorConfig::default()
    }
    .validate();
    let zero = DetectorConfig {
        alert_score: 0,
        ..DetectorConfig::default()
    }
    .validate();
    let confidence = DetectorConfig {
        medium_confidence_db: 12,
        ..DetectorConfig::default()
    }
    .validate();
    assert_eq!(
        (scores, zero, confidence),
        (
            Err(SettingError::ScoreOrder),
            Err(SettingError::ScoreOrder),
            Err(SettingError::ConfidenceOrder)
        )
    );
}

#[test]
fn given_manufacturer_entry_with_empty_prefix_when_validated_then_error() {
    let list = Whitelist {
        entries: vec![WhitelistEntry::Manufacturer {
            company: 0x004C,
            prefix: vec![],
        }],
    };
    let ok = Whitelist {
        entries: vec![WhitelistEntry::Manufacturer {
            company: 0x004C,
            prefix: vec![1],
        }],
    };
    assert_eq!(
        (list.validate(), ok.validate()),
        (Err(SettingError::EmptyManufacturerPrefix(0x004C)), Ok(()))
    );
}
