use super::*;
use crate::testkit::{single_room, three_rooms};

fn samples(readings: &[(&str, &[i16])]) -> BTreeMap<SensorId, Vec<i16>> {
    readings
        .iter()
        .map(|(id, v)| (SensorId::new(*id), v.to_vec()))
        .collect()
}

#[test]
fn given_empty_and_odd_and_even_samples_when_median_taken_then_lower_middle() {
    assert_eq!(
        (
            lower_median::<i16>(&[]),
            lower_median(&[3, 1, 2]),
            lower_median(&[4, 1, 3, 2])
        ),
        (None, Some(2), Some(2))
    );
}

#[test]
fn given_device_strongest_at_room_3_2_back_when_located_then_room_3_2_back_high() {
    let s = samples(&[
        ("3-2-뒤", &[-58, -57, -60]),
        ("3-2-앞", &[-66]),
        ("3-3-앞", &[-71]),
    ]);
    let loc = locate(&s, &three_rooms(), false, &DetectorConfig::default()).expect("표본이 있다");
    assert_eq!(
        (loc.room, loc.position, loc.confidence),
        (
            Some(RoomId::new("3-2")),
            Some(SensorPos::Back),
            Confidence::High
        )
    );
}

#[test]
fn given_room_difference_of_ten_db_when_located_then_high_confidence() {
    let s = samples(&[("3-1-앞", &[-55]), ("3-2-앞", &[-65])]);
    assert_eq!(
        locate(&s, &three_rooms(), false, &DetectorConfig::default()).map(|l| l.confidence),
        Some(Confidence::High)
    );
}

#[test]
fn given_room_difference_under_five_db_when_located_then_low_confidence() {
    let s = samples(&[("3-1-뒤", &[-60]), ("3-2-앞", &[-64])]);
    assert_eq!(
        locate(&s, &three_rooms(), false, &DetectorConfig::default()).map(|l| l.confidence),
        Some(Confidence::Low)
    );
}

#[test]
fn given_host_local_device_when_located_then_confidence_lowered_one_step() {
    let s = samples(&[("3-1-앞", &[-55]), ("3-2-앞", &[-65])]);
    assert_eq!(
        locate(&s, &three_rooms(), true, &DetectorConfig::default()).map(|l| l.confidence),
        Some(Confidence::Medium)
    );
}

#[test]
fn given_all_sensors_below_boundary_by_more_than_shield_margin_when_located_then_likely_outside_without_room()
 {
    let s = samples(&[("3-1-앞", &[-95]), ("3-2-앞", &[-100])]);
    let loc = locate(&s, &three_rooms(), false, &DetectorConfig::default()).expect("표본이 있다");
    assert_eq!((loc.room, loc.likely_outside), (None, true));
}

#[test]
fn given_body_shadowed_signal_within_shield_margin_when_located_then_room_kept() {
    let s = samples(&[("3-1-앞", &[-85]), ("3-2-앞", &[-90])]);
    let loc = locate(&s, &three_rooms(), false, &DetectorConfig::default()).expect("표본이 있다");
    assert_eq!(
        (loc.room, loc.likely_outside),
        (Some(RoomId::new("3-1")), false)
    );
}

#[test]
fn given_front_and_back_within_five_db_when_located_then_position_omitted() {
    let s = samples(&[("3-2-뒤", &[-60]), ("3-2-앞", &[-62]), ("3-3-앞", &[-80])]);
    let loc = locate(&s, &three_rooms(), false, &DetectorConfig::default()).expect("표본이 있다");
    assert_eq!((loc.room, loc.position), (Some(RoomId::new("3-2")), None));
}

#[test]
fn given_extreme_rssi_values_when_located_then_no_overflow() {
    let s = samples(&[("3-1-앞", &[i16::MAX]), ("3-2-앞", &[i16::MIN])]);
    assert_eq!(
        locate(&s, &three_rooms(), false, &DetectorConfig::default()).map(|l| l.confidence),
        Some(Confidence::High)
    );
}

#[test]
fn given_single_uncalibrated_room_when_located_then_room_with_low_confidence_and_calibration_flag()
{
    let s = samples(&[("solo", &[-70])]);
    let loc =
        locate(&s, &single_room(None), false, &DetectorConfig::default()).expect("표본이 있다");
    assert_eq!(
        (
            loc.room,
            loc.position,
            loc.confidence,
            loc.needs_calibration
        ),
        (Some(RoomId::new("3-2")), None, Confidence::Low, true)
    );
}

#[test]
fn given_single_calibrated_room_when_located_then_high_confidence() {
    let s = samples(&[("solo", &[-70])]);
    assert_eq!(
        locate(
            &s,
            &single_room(Some(-75)),
            false,
            &DetectorConfig::default()
        )
        .map(|l| l.confidence),
        Some(Confidence::High)
    );
}

#[test]
fn given_only_one_room_seen_in_three_room_layout_when_located_then_low_confidence() {
    let s = samples(&[("3-3-뒤", &[-60])]);
    assert_eq!(
        locate(&s, &three_rooms(), false, &DetectorConfig::default()).map(|l| l.confidence),
        Some(Confidence::Low)
    );
}

#[test]
fn given_unknown_sensor_only_when_located_then_none() {
    let s = samples(&[("9-9-앞", &[-50])]);
    assert_eq!(
        locate(&s, &three_rooms(), false, &DetectorConfig::default()),
        None
    );
}

#[test]
fn given_equal_medians_when_located_then_evidence_ordered_by_sensor_id() {
    let s = samples(&[("3-2-앞", &[-60]), ("3-1-앞", &[-60])]);
    let loc = locate(&s, &three_rooms(), false, &DetectorConfig::default()).expect("표본이 있다");
    assert_eq!(
        loc.evidence,
        vec![
            (SensorId::new("3-1-앞"), -60),
            (SensorId::new("3-2-앞"), -60)
        ]
    );
}

proptest::proptest! {
    #[test]
    fn given_any_samples_when_one_sensor_raised_then_its_room_does_not_drop_out_of_first(
        a in -95i16..-30, b in -95i16..-30, bump in 0i16..20,
    ) {
        let cfg = DetectorConfig::default();
        let before = locate(&samples(&[("3-1-앞", &[a]), ("3-2-앞", &[b])]), &three_rooms(), false, &cfg).and_then(|l| l.evidence.first().cloned());
        let after = locate(&samples(&[("3-1-앞", &[a.saturating_add(bump)]), ("3-2-앞", &[b])]), &three_rooms(), false, &cfg).and_then(|l| l.evidence.first().cloned());
        let was_first = before.as_ref().is_some_and(|(id, _)| id == &SensorId::new("3-1-앞"));
        let still_first = after.as_ref().is_some_and(|(id, _)| id == &SensorId::new("3-1-앞"));
        proptest::prop_assert!(!was_first || still_first);
    }
}

#[test]
fn given_separate_wifi_boundary_when_located_by_radio_then_each_radio_uses_its_own_boundary() {
    let mut layout = single_room(Some(-80));
    layout.rooms[0].sensors[0].wifi_inside_boundary_dbm = Some(-50);
    let s = samples(&[("solo", &[-70])]);
    let cfg = DetectorConfig::default();
    let outside = |radio| {
        locate_radio(&s, &layout, false, &cfg, radio)
            .map(|l| (l.likely_outside, l.needs_calibration))
    };
    assert_eq!(
        (
            outside(crate::config::Radio::Wifi),
            outside(crate::config::Radio::Bluetooth)
        ),
        (Some((true, false)), Some((false, false)))
    );
}

#[test]
fn given_no_wifi_boundary_when_located_for_wifi_then_needs_calibration() {
    let mut layout = single_room(Some(-80));
    layout.rooms[0].sensors[0].wifi_inside_boundary_dbm = None;
    let s = samples(&[("solo", &[-70])]);
    let loc = locate_radio(
        &s,
        &layout,
        false,
        &DetectorConfig::default(),
        crate::config::Radio::Wifi,
    );
    assert_eq!(
        loc.map(|l| (l.needs_calibration, l.likely_outside)),
        Some((true, false))
    );
}
