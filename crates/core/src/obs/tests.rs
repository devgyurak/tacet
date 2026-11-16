use super::*;
use crate::testkit::{ble, doc_mac, rf, wifi};

#[test]
fn given_zero_length_ssid_when_read_then_hidden() {
    assert_eq!(Ssid::from_bytes(&[]), Ssid::Hidden);
}

#[test]
fn given_nul_filled_ssid_when_read_then_hidden() {
    assert_eq!(Ssid::from_bytes(&[0, 0, 0, 0]), Ssid::Hidden);
}

#[test]
fn given_named_ssid_bytes_when_read_then_named() {
    assert_eq!(
        Ssid::from_bytes(b"DIRECT-ab"),
        Ssid::Named("DIRECT-ab".to_owned())
    );
}

#[test]
fn given_center_frequencies_when_mapped_then_band_and_channel_follow_802_11() {
    let mapped = [2412, 2484, 5180, 5825, 5935, 5955, 6115, 900].map(band_channel_from_mhz);
    assert_eq!(
        mapped,
        [
            Some((Band::B2_4, 1)),
            Some((Band::B2_4, 14)),
            Some((Band::B5, 36)),
            Some((Band::B5, 165)),
            Some((Band::B6, 2)),
            Some((Band::B6, 1)),
            Some((Band::B6, 33)),
            None,
        ]
    );
}

#[test]
fn given_channel_1_on_2_4_and_6_ghz_when_mapped_then_bands_differ() {
    assert_ne!(band_channel_from_mhz(2412), band_channel_from_mhz(5955));
}

#[test]
fn given_cell_bands_when_uplink_ranges_read_then_3gpp_values() {
    assert_eq!(
        CellBand::ALL.map(CellBand::uplink_khz)[0],
        (1_920_000, 1_980_000)
    );
}

#[test]
fn given_observations_when_strength_read_then_rf_has_no_dbm_strength() {
    let strengths = [
        ble(1).rssi(-61).build().strength_dbm(),
        wifi(doc_mac(1)).rssi(-70).build().strength_dbm(),
        rf(CellBand::B3, -55, 0).strength_dbm(),
    ];
    assert_eq!(strengths, [Some(-61), Some(-70), None]);
}

#[test]
fn given_observations_when_time_read_then_each_reports_its_stamp() {
    let times = [
        ble(1).at_secs(3).build().at(),
        rf(CellBand::B1, -50, 4).at(),
        wifi(doc_mac(1)).at_secs(5).build().at(),
    ];
    assert_eq!(times, [Timestamp(3000), Timestamp(4000), Timestamp(5000)]);
}

#[test]
fn given_advertised_name_bytes_when_cleaned_then_control_chars_dropped_trimmed_and_capped() {
    let long = "가".repeat(50);
    assert_eq!(
        [
            ble_name(" 시험 폰\n".as_bytes()).map(|n| n.0),
            ble_name(&[0x00, 0x01]).map(|n| n.0),
            ble_name(&[0xFF, b'A']).map(|n| n.0),
            ble_name(long.as_bytes()).map(|n| n.0.chars().count().to_string()),
            ble_name(b"").map(|n| n.0),
        ],
        [
            Some("시험 폰".to_owned()),
            None,
            Some("\u{FFFD}A".to_owned()),
            Some("40".to_owned()),
            None
        ]
    );
}

#[test]
fn given_ble_observation_with_name_when_serialized_or_debug_printed_then_name_absent() {
    let mut obs = crate::testkit::ble(1).build_ble();
    obs.name = ble_name("시험 폰".as_bytes());
    let json = serde_json::to_string(&obs).unwrap_or_default();
    let back: Option<BleObs> = serde_json::from_str(&json).ok();
    assert_eq!(
        (
            json.contains("시험 폰"),
            format!("{obs:?}").contains("시험 폰"),
            back.map(|b| b.name)
        ),
        (false, false, Some(None))
    );
}
