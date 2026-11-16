use tacet_core::obs::{CollectorStatus, OffReason, Permission};

use super::*;

const DOC_BSSID: [u8; 6] = [0x02, 0x00, 0x5E, 0x00, 0x53, 0x01];

#[test]
fn given_windows_entry_on_2_4ghz_when_converted_then_channel_and_named_ssid() {
    let obs =
        from_windows(DOC_BSSID, b"DIRECT-ab\0\0", 9, -61, 2_437_000, Timestamp(1)).expect("관측값");
    assert_eq!(
        (obs.band, obs.channel, obs.ssid, obs.rssi),
        (Band::B2_4, 6, Ssid::Named("DIRECT-ab".to_owned()), -61)
    );
}

#[test]
fn given_windows_hidden_entry_on_6ghz_when_converted_then_hidden_ssid_and_6ghz() {
    let obs = from_windows(DOC_BSSID, &[0; 32], 0, -55, 6_135_000, Timestamp(1)).expect("관측값");
    assert_eq!(
        (obs.band, obs.channel, obs.ssid),
        (Band::B6, 37, Ssid::Hidden)
    );
}

#[test]
fn given_windows_entry_with_overlong_length_when_converted_then_clamped_to_buffer() {
    let obs = from_windows(DOC_BSSID, b"abc", 99, -50, 5_180_000, Timestamp(0)).expect("관측값");
    assert_eq!(obs.ssid, Ssid::Named("abc".to_owned()));
}

#[test]
fn given_unknown_frequency_or_huge_rssi_when_converted_then_none_or_clamped() {
    let none = from_windows(DOC_BSSID, b"a", 1, -50, 900_000, Timestamp(0));
    let clamped =
        from_windows(DOC_BSSID, b"a", 1, i32::MIN, 2_412_000, Timestamp(0)).map(|o| o.rssi);
    assert_eq!((none, clamped), (None, Some(i16::MIN)));
}

#[test]
fn given_corewlan_network_when_converted_then_parsed_bssid_and_band() {
    let obs = from_corewlan(
        Some("02:00:5e:00:53:01"),
        Some(b"\0\0\0"),
        -70,
        149,
        2,
        Timestamp(3),
    )
    .expect("관측값");
    assert_eq!(
        (obs.bssid, obs.band, obs.channel, obs.ssid),
        (Mac(DOC_BSSID), Band::B5, 149, Ssid::Hidden)
    );
}

#[test]
fn given_corewlan_without_bssid_or_band_when_converted_then_none() {
    assert_eq!(
        (
            from_corewlan(None, None, -50, 1, 1, Timestamp(0)),
            from_corewlan(Some("02:00:5e:00:53:01"), None, -50, 1, 0, Timestamp(0))
        ),
        (None, None)
    );
}

#[test]
fn given_corewlan_without_ssid_data_when_converted_then_hidden() {
    assert_eq!(
        from_corewlan(Some("02:00:5e:00:53:01"), None, -50, 1, 1, Timestamp(0)).map(|o| o.ssid),
        Some(Ssid::Hidden)
    );
}

#[test]
fn given_corewlan_band_codes_when_mapped_then_three_bands() {
    assert_eq!(
        [0, 1, 2, 3].map(band_from_corewlan),
        [None, Some(Band::B2_4), Some(Band::B5), Some(Band::B6)]
    );
}

#[test]
fn given_scan_counts_when_status_decided_then_all_missing_bssids_mean_no_location_permission() {
    assert_eq!(
        [
            corewlan_status(3, 3),
            corewlan_status(3, 1),
            corewlan_status(0, 0)
        ],
        [
            CollectorStatus::Off(OffReason::PermissionDenied(Permission::Location)),
            CollectorStatus::Running,
            CollectorStatus::Running,
        ]
    );
}
