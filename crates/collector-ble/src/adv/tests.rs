use tacet_core::ids::{BleId, Mac, ServiceUuid};

use super::*;

#[test]
fn given_windows_address_value_when_converted_then_big_endian_low_48_bits() {
    assert_eq!(
        addr_from_u64(0x0000_0200_5E00_532A),
        [0x02, 0x00, 0x5E, 0x00, 0x53, 0x2A]
    );
}

#[test]
fn given_uuid_texts_when_parsed_then_hyphens_ignored_and_bad_length_rejected() {
    assert_eq!(
        (
            uuid_from_text("00000000-0000-4000-8000-000000000007"),
            uuid_from_text("1234")
        ),
        (Some(0x0000_0000_0000_4000_8000_0000_0000_0007), None)
    );
}

#[test]
fn given_spec_shaped_payload_when_parsed_then_sections_split_by_length() {
    // 플래그(0x01) 1바이트, 제조사 데이터(0xFF) 회사 0x004C + 2바이트.
    let payload = [0x02, 0x01, 0x06, 0x05, 0xFF, 0x4C, 0x00, 0x12, 0x34];
    assert_eq!(
        parse_ad_structures(&payload),
        vec![(0x01, &[0x06][..]), (0xFF, &[0x4C, 0x00, 0x12, 0x34][..])]
    );
}

#[test]
fn given_length_beyond_payload_when_parsed_then_stops_without_panic() {
    assert_eq!(
        parse_ad_structures(&[0x02, 0x01, 0x06, 0x09, 0xFF, 0x4C]),
        vec![(0x01, &[0x06][..])]
    );
}

#[test]
fn given_zero_length_section_when_parsed_then_stops() {
    assert_eq!(
        parse_ad_structures(&[0x00, 0x02, 0x01, 0x06]),
        Vec::<(u8, &[u8])>::new()
    );
}

#[test]
fn given_sections_when_applied_then_uuids_service_data_and_vendor_collected() {
    let mut raw = RawAdvertisement::default();
    apply_section(&mut raw, AD_UUID16_COMPLETE, &[0x5F, 0xFD, 0x2C]);
    apply_section(&mut raw, AD_UUID128_COMPLETE, &[7; 16]);
    apply_section(&mut raw, AD_SERVICE_DATA16, &[0x2C, 0xFE, 1, 2]);
    apply_section(&mut raw, AD_MANUFACTURER, &[0xAB, 0x01, 9]);
    apply_section(&mut raw, AD_SERVICE_DATA16, &[0x2C]);
    apply_section(&mut raw, 0x09, b"name");
    assert_eq!(
        (raw.services.len(), raw.service_data, raw.manufacturer),
        (
            2,
            vec![(ServiceUuid::from_u16(0xFE2C).0, vec![1, 2])],
            vec![(0x01AB, vec![9])]
        )
    );
}

#[test]
fn given_windows_advertisement_when_converted_then_addr_id_and_sorted_fields() {
    let raw = RawAdvertisement {
        addr: Some([0x02, 0x00, 0x5E, 0x00, 0x53, 0x01]),
        host_uuid: None,
        rssi: Some(-61),
        manufacturer: vec![(0x0075, vec![2]), (0x004C, vec![1])],
        services: vec![
            ServiceUuid::from_u16(0xFE2C).0,
            ServiceUuid::from_u16(0xFD5F).0,
            ServiceUuid::from_u16(0xFD5F).0,
        ],
        service_data: vec![],
        name: None,
    };
    let obs = to_obs(raw, Timestamp(5)).expect("관측값");
    assert_eq!(
        (obs.id, obs.manufacturer[0].0, obs.services.len()),
        (
            BleId::Addr(Mac([0x02, 0x00, 0x5E, 0x00, 0x53, 0x01])),
            0x004C,
            2
        )
    );
}

#[test]
fn given_macos_advertisement_with_zero_address_when_converted_then_host_local_id() {
    let raw = RawAdvertisement {
        addr: Some([0; 6]),
        host_uuid: Some(7),
        rssi: Some(-70),
        ..RawAdvertisement::default()
    };
    assert_eq!(
        to_obs(raw, Timestamp(0)).map(|o| o.id),
        Some(BleId::HostLocal(7))
    );
}

#[test]
fn given_missing_rssi_or_identity_when_converted_then_none() {
    let no_rssi = RawAdvertisement {
        addr: Some([1; 6]),
        ..RawAdvertisement::default()
    };
    let no_id = RawAdvertisement {
        rssi: Some(-50),
        ..RawAdvertisement::default()
    };
    assert_eq!(
        (to_obs(no_rssi, Timestamp(0)), to_obs(no_id, Timestamp(0))),
        (None, None)
    );
}

/// 구간으로 나눠 모두 반영하고, 구간이 차지한 바이트 수를 돌려준다.
fn parse_and_apply(bytes: &[u8]) -> usize {
    let mut raw = RawAdvertisement::default();
    let sections = parse_ad_structures(bytes);
    for (ad_type, data) in &sections {
        apply_section(&mut raw, *ad_type, data);
    }
    sections.iter().map(|(_, d)| d.len() + 2).sum()
}

proptest::proptest! {
    #[test]
    fn given_any_payload_when_parsed_and_applied_then_sections_fit_inside_payload(bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..64)) {
        proptest::prop_assert!(parse_and_apply(&bytes) <= bytes.len());
    }
}

#[test]
fn given_complete_and_short_names_when_applied_then_complete_name_kept_and_cleaned() {
    let mut raw = RawAdvertisement {
        rssi: Some(-50),
        addr: Some([0x02, 0x00, 0x5E, 0x00, 0x53, 0x01]),
        ..RawAdvertisement::default()
    };
    apply_section(&mut raw, AD_NAME_SHORT, "짧은".as_bytes());
    apply_section(&mut raw, AD_NAME_COMPLETE, " 시험 폰\t".as_bytes());
    apply_section(&mut raw, AD_NAME_SHORT, "다시짧은".as_bytes());
    assert_eq!(
        to_obs(raw, Timestamp(0)).and_then(|o| o.name).map(|n| n.0),
        Some("시험 폰".to_owned())
    );
}

#[test]
fn given_only_short_name_when_applied_then_short_name_used() {
    let mut raw = RawAdvertisement {
        rssi: Some(-50),
        addr: Some([0x02, 0x00, 0x5E, 0x00, 0x53, 0x01]),
        ..RawAdvertisement::default()
    };
    apply_section(&mut raw, AD_NAME_SHORT, "시험".as_bytes());
    assert_eq!(
        to_obs(raw, Timestamp(0)).and_then(|o| o.name).map(|n| n.0),
        Some("시험".to_owned())
    );
}

#[test]
fn given_named_advertisement_when_names_dropped_for_detection_then_name_removed_and_kept_for_calibration()
 {
    let mut raw = RawAdvertisement {
        rssi: Some(-50),
        addr: Some([0x02, 0x00, 0x5E, 0x00, 0x53, 0x01]),
        ..RawAdvertisement::default()
    };
    apply_section(&mut raw, AD_NAME_COMPLETE, "시험".as_bytes());
    let obs = to_obs(raw, Timestamp(0));
    assert_eq!(
        (
            obs.clone().map(|o| apply_names(o, crate::Names::Drop).name),
            obs.map(|o| apply_names(o, crate::Names::Keep).name.map(|n| n.0))
        ),
        (Some(None), Some(Some("시험".to_owned())))
    );
}
